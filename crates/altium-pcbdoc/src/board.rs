//! Roadmap step 2: the board outline, its cutouts and the board thickness.
//!
//! Everything here was worked out from our own CC0 test boards in `tests/boards/` and the numbers
//! in their README.txt (see LEGAL.md):
//!
//! - `/Board6/Data` holds one record: a little-endian `u32` length, then text of the form
//!   `|KEY=VALUE|KEY=VALUE…` ending in a NUL byte.
//! - The board shape is a numbered vertex list in that record: `KINDn`, `VXn`, `VYn`, `CXn`,
//!   `CYn`, `SAn`, `EAn`, `Rn`. The last vertex repeats the first. Lengths are text such as
//!   `1181.1024mil`, angles text such as ` 1.80000000000000E+0002` (degrees).
//! - Vertex n starts the edge to vertex n + 1. `KINDn=0` is a straight edge (its centre, radius
//!   and angles are 0). `KINDn=1` is an arc with centre (`CXn`, `CYn`), radius `Rn`, running
//!   counter-clockwise from `SAn` to `EAn`: on `outline-with-arcs` (a 50 x 30 mm outline with
//!   3 mm corner arcs) Altium lists the vertices clockwise, so each arc's vertex sits at `EAn`
//!   and the next one at `SAn`. The reader hands such a contour over reversed, counter-clockwise,
//!   as `Segment::Arc` has no other direction.
//! - The layer stack is in `V9_STACK_LAYERn_…` keys: each solder mask and dielectric has a
//!   `DIELHEIGHT`, each copper layer a `COPTHICK`. Their sum is the 1.600 mm we typed in.
//! - `/Regions6/Data` is a run of records: a type byte (`0x0b`), a `u32` length, then 18 bytes we
//!   do not read yet, a `u32` text length, the text (with `ISBOARDCUTOUT=TRUE` for a board
//!   cutout), a `u32` vertex count, and that many pairs of little-endian `f64` (x, y) in
//!   1/10000 mil. The polygon does not repeat its first vertex.
//! - A cutout with arcs (the round one on `outline-with-arcs`) is only a polygon there (35
//!   corners); its arcs are in `/ShapeBasedRegions6/Data` (see `shape_based.rs`), which is read
//!   for it instead.

use std::collections::BTreeMap;

use board_model::{Contour, Note, Outline, Point, Segment, Severity};

/// Millimetres per mil.
const MM_PER_MIL: f64 = 0.0254;
/// Millimetres per internal unit (1/10000 mil), the unit of binary coordinates.
const MM_PER_UNIT: f64 = MM_PER_MIL / 10_000.0;
/// The most vertices one outline or region may have before the file is treated as hostile.
const MAX_VERTICES: usize = 100_000;
/// The most stack layers read before the file is treated as hostile.
const MAX_STACK_LAYERS: usize = 10_000;
/// The most region records read from one stream.
const MAX_REGIONS: usize = 1_000_000;
/// Lengths larger than this (in mm, about 25 m) are refused as not a real board.
const MAX_LENGTH_MM: f64 = 25_000.0;
/// The record type byte of a region, as seen in `/Regions6/Data`.
const REGION_RECORD: u8 = 0x0b;
/// Bytes between a region record's length and its text length that are not read yet.
const REGION_SKIPPED: usize = 18;

/// The `KEY=VALUE` pairs of one text record. Keys keep their case; the first of a repeated key wins.
#[derive(Debug, Default)]
pub(crate) struct Properties(BTreeMap<String, String>);

impl Properties {
    /// Parses `|KEY=VALUE|KEY=VALUE…`, stopping at the first NUL byte. Bytes are read as Latin-1.
    pub(crate) fn parse(bytes: &[u8]) -> Self {
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        let text: String = bytes.iter().take(end).map(|&b| char::from(b)).collect();
        let mut map = BTreeMap::new();
        for field in text.split('|') {
            if let Some((key, value)) = field.split_once('=') {
                map.entry(key.to_string())
                    .or_insert_with(|| value.to_string());
            }
        }
        Self(map)
    }

    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// A length such as `1181.1024mil`, in millimetres.
    pub(crate) fn length_mm(&self, key: &str) -> Result<Option<f64>, String> {
        self.get(key).map(|v| parse_length_mm(key, v)).transpose()
    }
}

/// Parses a length written as a number followed by `mil` into millimetres.
fn parse_length_mm(key: &str, value: &str) -> Result<f64, String> {
    let number = value
        .trim()
        .strip_suffix("mil")
        .ok_or_else(|| format!("{key}={value}: expected a length in mil"))?;
    let mil: f64 = number
        .trim()
        .parse()
        .map_err(|_| format!("{key}={value}: not a number"))?;
    let mm = mil * MM_PER_MIL;
    if mm.is_finite() && mm.abs() <= MAX_LENGTH_MM {
        Ok(mm)
    } else {
        Err(format!("{key}={value}: out of range"))
    }
}

/// Reads the one record in `/Board6/Data`.
pub(crate) fn board_record(data: &[u8]) -> Result<Properties, String> {
    let len_bytes: [u8; 4] = data
        .get(0..4)
        .and_then(|b| b.try_into().ok())
        .ok_or("/Board6/Data is shorter than its length field")?;
    let len = usize::try_from(u32::from_le_bytes(len_bytes)).map_err(|e| e.to_string())?;
    let text = 4usize
        .checked_add(len)
        .and_then(|end| data.get(4..end))
        .ok_or("/Board6/Data is shorter than its record says")?;
    Ok(Properties::parse(text))
}

/// One vertex of an outline that may have arcs: the edge from it to the next vertex is straight
/// (`arc` is `None`) or the arc given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Vertex {
    pub at: Point,
    pub arc: Option<Arc>,
}

/// A circle piece running counter-clockwise from `start_deg` to `end_deg`, as Altium stores it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Arc {
    pub center: Point,
    pub radius: f64,
    pub start_deg: f64,
    pub end_deg: f64,
}

/// How far (mm) a vertex may be from its arc's end and still count as on it.
const ARC_END_TOLERANCE: f64 = 0.001;

/// The board shape from the `KINDn`/`VXn`/`VYn` vertex list, with `CXn`/`CYn`/`Rn`/`SAn`/`EAn`
/// for the arcs.
pub(crate) fn outline(board: &Properties, notes: &mut Vec<Note>) -> Result<Contour, String> {
    let mut vertices = Vec::new();
    let mut unknown = 0usize;
    for i in 0..=MAX_VERTICES {
        let Some(kind) = board.get(&format!("KIND{i}")) else {
            break;
        };
        if i == MAX_VERTICES {
            return Err(format!(
                "the board outline has more than {MAX_VERTICES} vertices"
            ));
        }
        let at = match (
            board.length_mm(&format!("VX{i}"))?,
            board.length_mm(&format!("VY{i}"))?,
        ) {
            (Some(x), Some(y)) => Point { x, y },
            _ => return Err(format!("outline vertex {i} has no VX{i}/VY{i}")),
        };
        let arc = match kind.trim() {
            "0" => None,
            "1" => Some(outline_arc(board, i)?),
            _ => {
                unknown += 1;
                None
            }
        };
        vertices.push(Vertex { at, arc });
    }
    if unknown > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{unknown} board outline edge(s) have an unknown KIND (not 0 or 1); read as \
                 straight lines"
            ),
        });
    }
    let has_arcs = vertices.iter().any(|v| v.arc.is_some());
    let corners = distinct(&vertices).len();
    if corners < 3 && !(has_arcs && corners >= 1) {
        return Err(format!(
            "the board outline has {corners} vertices; a board needs at least 3"
        ));
    }
    Ok(contour("the board outline", &vertices, notes))
}

/// The arc of outline vertex `i`.
fn outline_arc(board: &Properties, i: usize) -> Result<Arc, String> {
    let length = |key: String| -> Result<f64, String> {
        board
            .length_mm(&key)?
            .ok_or_else(|| format!("outline arc {i} has no {key}"))
    };
    let angle = |key: String| -> Result<f64, String> {
        board
            .get(&key)
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|a| a.is_finite() && a.abs() <= 3600.0)
            .ok_or_else(|| format!("outline arc {i} has no usable {key}"))
    };
    Ok(Arc {
        center: Point {
            x: length(format!("CX{i}"))?,
            y: length(format!("CY{i}"))?,
        },
        radius: length(format!("R{i}"))?,
        start_deg: angle(format!("SA{i}"))?,
        end_deg: angle(format!("EA{i}"))?,
    })
}

/// The vertices without a last one that repeats the first.
fn distinct(vertices: &[Vertex]) -> &[Vertex] {
    match vertices {
        [first, .., last] if near(first.at, last.at) => vertices.split_last().map_or(&[], |s| s.1),
        _ => vertices,
    }
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).hypot(a.y - b.y) <= ARC_END_TOLERANCE
}

fn on_circle(arc: &Arc, deg: f64) -> Point {
    let (sin, cos) = deg.to_radians().sin_cos();
    Point {
        x: arc.center.x + arc.radius * cos,
        y: arc.center.y + arc.radius * sin,
    }
}

/// Joins the vertices into a closed contour, each to the next by a line or by its arc. Altium
/// may list them clockwise (each arc then runs from its end angle to its start angle); since
/// [`Segment::Arc`] only runs counter-clockwise, such a contour is handed over reversed. An arc
/// whose ends are not its two vertices, or that runs against the others, becomes a straight
/// line with a warning that names `what`.
pub(crate) fn contour(what: &str, vertices: &[Vertex], notes: &mut Vec<Note>) -> Contour {
    /// One edge before the direction is settled: a line, or an arc and whether it runs
    /// counter-clockwise from `from` to `to`.
    enum Edge {
        Line,
        Arc(Arc, bool),
    }
    let vertices = distinct(vertices);
    let mut edges = Vec::with_capacity(vertices.len());
    let mut unmatched = 0usize;
    for (i, v) in vertices.iter().enumerate() {
        let to = vertices
            .get(i + 1)
            .or(vertices.first())
            .map_or(v.at, |n| n.at);
        let edge = match v.arc {
            None => Edge::Line,
            Some(arc) if arc.radius.is_finite() && arc.radius > ARC_END_TOLERANCE => {
                let (start, end) = (on_circle(&arc, arc.start_deg), on_circle(&arc, arc.end_deg));
                if near(v.at, start) && near(to, end) {
                    Edge::Arc(arc, true)
                } else if near(v.at, end) && near(to, start) {
                    Edge::Arc(arc, false)
                } else {
                    unmatched += 1;
                    Edge::Line
                }
            }
            Some(_) => {
                unmatched += 1;
                Edge::Line
            }
        };
        edges.push((v.at, to, edge));
    }
    let ccw = edges
        .iter()
        .filter(|e| matches!(e.2, Edge::Arc(_, true)))
        .count();
    let cw = edges
        .iter()
        .filter(|e| matches!(e.2, Edge::Arc(_, false)))
        .count();
    let reversed = cw > ccw;
    if reversed {
        edges.reverse();
        for e in &mut edges {
            std::mem::swap(&mut e.0, &mut e.1);
            if let Edge::Arc(_, forward) = &mut e.2 {
                *forward = !*forward;
            }
        }
    }
    let against = ccw.min(cw);
    let segments = edges
        .into_iter()
        .map(|(from, to, edge)| match edge {
            Edge::Arc(arc, true) => Segment::Arc {
                center: arc.center,
                radius: arc.radius,
                start_deg: arc.start_deg,
                end_deg: arc.end_deg,
            },
            Edge::Arc(_, false) | Edge::Line => Segment::Line { from, to },
        })
        .collect();
    if unmatched > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{what}: {unmatched} arc(s) do not end at their vertices; read as straight lines"
            ),
        });
    }
    if against > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{what}: {against} arc(s) turn against the others; read as straight lines"
            ),
        });
    }
    Contour { segments }
}

/// Joins the points with straight lines and closes the loop. A last point equal to the first
/// (as Altium writes the board outline) is not doubled.
fn closed_polygon(points: &[Point]) -> Contour {
    let mut pts: Vec<Point> = points.to_vec();
    if let (Some(first), Some(last)) = (pts.first(), pts.last()) {
        if pts.len() > 1 && first == last {
            pts.pop();
        }
    }
    let mut segments = Vec::with_capacity(pts.len());
    for (i, from) in pts.iter().enumerate() {
        let to = pts.get(i + 1).or(pts.first());
        if let Some(to) = to {
            segments.push(Segment::Line {
                from: *from,
                to: *to,
            });
        }
    }
    Contour { segments }
}

/// The whole stack's thickness: every `DIELHEIGHT` and `COPTHICK` of the `V9_STACK_LAYERn` layers.
pub(crate) fn thickness(board: &Properties, notes: &mut Vec<Note>) -> Result<Option<f64>, String> {
    let mut total = 0.0;
    let mut layers = 0usize;
    for n in 0..=MAX_STACK_LAYERS {
        if board.get(&format!("V9_STACK_LAYER{n}_NAME")).is_none() {
            break;
        }
        if n == MAX_STACK_LAYERS {
            return Err(format!(
                "the layer stack has more than {MAX_STACK_LAYERS} layers"
            ));
        }
        layers += 1;
        for part in ["DIELHEIGHT", "COPTHICK"] {
            if let Some(mm) = board.length_mm(&format!("V9_STACK_LAYER{n}_{part}"))? {
                total += mm;
            }
        }
    }
    if layers == 0 {
        notes.push(Note {
            severity: Severity::Dropped,
            message: "no V9 layer stack in /Board6/Data; the board thickness is unknown".into(),
        });
        return Ok(None);
    }
    if board.get("V9_SUBSTACK1_ID").is_some() {
        notes.push(Note {
            severity: Severity::Warning,
            message: "the board has more than one layer stack (rigid-flex?); the thickness is \
                      the sum of all stack layers and may be wrong"
                .into(),
        });
    }
    Ok(Some(total))
}

/// A region from `/Regions6/Data` that is not a board cutout (copper regions, poured copper, …).
#[derive(Debug)]
pub(crate) struct OtherRegion {
    /// The layer byte (byte 0 of the record body; 1 is Top Layer on our boards).
    pub layer: u8,
    /// The `KIND` value (`0` for a copper region and for poured copper on our boards).
    pub kind: Option<String>,
    /// True when the region belongs to a polygon pour (`SUBPOLYINDEX` is 0 or more).
    pub poured: bool,
    pub contour: Contour,
}

/// The regions of `/Regions6/Data`: board cutouts, and every other region for `copper.rs`.
#[derive(Debug, Default)]
pub(crate) struct Regions {
    pub cutouts: Vec<Contour>,
    pub others: Vec<OtherRegion>,
}

/// Reads `/Regions6/Data`. A board cutout that cannot be read is an error (the board shape would
/// be wrong); any other region that cannot be read is left out with a Dropped note. `shaped`
/// holds, by record number, the outline with arcs from `/ShapeBasedRegions6/Data` where that
/// copy has arcs: a cutout takes it instead of its polygon.
pub(crate) fn regions(
    data: &[u8],
    shaped: &[Option<Vec<Vertex>>],
    notes: &mut Vec<Note>,
) -> Result<Regions, String> {
    let mut found = Regions::default();
    let mut unreadable = 0usize;
    let mut other_arcs = 0usize;
    let mut at = 0usize;
    let mut count = 0usize;
    while at < data.len() {
        count += 1;
        if count > MAX_REGIONS {
            return Err(format!(
                "/Regions6/Data has more than {MAX_REGIONS} records"
            ));
        }
        let kind = data.get(at).copied().ok_or("region record cut short")?;
        if kind != REGION_RECORD {
            notes.push(Note {
                severity: Severity::Dropped,
                message: format!(
                    "/Regions6/Data: unknown record type {kind:#04x} at byte {at}; the rest of the \
                     stream was not read"
                ),
            });
            break;
        }
        let body_len = read_u32(data, at + 1).ok_or("region record length cut short")?;
        let body_start = at + 5;
        let body_end = body_start
            .checked_add(body_len)
            .filter(|&end| end <= data.len())
            .ok_or("region record is longer than the stream")?;
        let body = data
            .get(body_start..body_end)
            .ok_or("region record out of range")?;
        let text_len = read_u32(body, REGION_SKIPPED).ok_or("region text length cut short")?;
        let text_start = REGION_SKIPPED + 4;
        let text_end = text_start
            .checked_add(text_len)
            .ok_or("region text length out of range")?;
        let text = body
            .get(text_start..text_end)
            .ok_or("region text is longer than its record")?;
        let props = Properties::parse(text);
        let corners = region_points(body, text_end);
        let arcs = shaped.get(count - 1).and_then(Option::as_ref);
        if props.get("ISBOARDCUTOUT") == Some("TRUE") {
            match arcs {
                Some(vertices) => found
                    .cutouts
                    .push(contour("a board cutout", vertices, notes)),
                None => found.cutouts.push(closed_polygon(
                    &corners.map_err(|e| format!("a board cutout {e}"))?,
                )),
            }
        } else {
            if arcs.is_some() {
                other_arcs += 1;
            }
            match corners {
                Ok(points) => found.others.push(OtherRegion {
                    layer: body.first().copied().unwrap_or(0),
                    kind: props.get("KIND").map(str::to_string),
                    poured: props
                        .get("SUBPOLYINDEX")
                        .and_then(|v| v.trim().parse::<i64>().ok())
                        .is_some_and(|i| i >= 0),
                    contour: closed_polygon(&points),
                }),
                Err(_) => unreadable += 1,
            }
        }
        at = body_end;
    }
    if unreadable > 0 {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!("/Regions6/Data: {unreadable} region(s) could not be read"),
        });
    }
    if other_arcs > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{other_arcs} region(s) that are not board cutouts have arcs (in \
                 /ShapeBasedRegions6/Data); read as the polygon of /Regions6/Data"
            ),
        });
    }
    Ok(found)
}

/// The corners of a region: a `u32` count at `at`, then that many `f64` (x, y) pairs.
fn region_points(body: &[u8], at: usize) -> Result<Vec<Point>, String> {
    let n = read_u32(body, at).ok_or("has its vertex count cut short")?;
    if n > MAX_VERTICES {
        return Err(format!("has {n} vertices, more than {MAX_VERTICES}"));
    }
    if n < 3 {
        return Err(format!("has {n} vertices; it needs at least 3"));
    }
    let mut points = Vec::with_capacity(n);
    for k in 0..n {
        let p = at + 4 + 16 * k;
        let x = read_f64(body, p).ok_or("has its vertices cut short")?;
        let y = read_f64(body, p + 8).ok_or("has its vertices cut short")?;
        points.push(Point {
            x: unit_to_mm(x)?,
            y: unit_to_mm(y)?,
        });
    }
    Ok(points)
}

fn read_u32(data: &[u8], at: usize) -> Option<usize> {
    let bytes: [u8; 4] = data.get(at..at.checked_add(4)?)?.try_into().ok()?;
    usize::try_from(u32::from_le_bytes(bytes)).ok()
}

fn read_f64(data: &[u8], at: usize) -> Option<f64> {
    let bytes: [u8; 8] = data.get(at..at.checked_add(8)?)?.try_into().ok()?;
    Some(f64::from_le_bytes(bytes))
}

/// An internal coordinate (1/10000 mil) in millimetres.
pub(crate) fn unit_to_mm(v: f64) -> Result<f64, String> {
    let mm = v * MM_PER_UNIT;
    if mm.is_finite() && mm.abs() <= MAX_LENGTH_MM {
        Ok(mm)
    } else {
        Err(format!("coordinate {v} is out of range"))
    }
}

/// The outline (outer shape plus cutouts) and thickness of a board.
pub(crate) struct ShapeAndThickness {
    pub outline: Outline,
    pub thickness: Option<f64>,
    /// The regions that are not board cutouts, for `copper.rs`.
    pub other_regions: Vec<OtherRegion>,
}

/// Reads the board shape, cutouts and thickness from the `/Board6/Data` record and the regions.
pub(crate) fn read(
    board: &Properties,
    regions6: Option<&[u8]>,
    shaped: &[Option<Vec<Vertex>>],
    notes: &mut Vec<Note>,
) -> Result<ShapeAndThickness, String> {
    let outer = outline(board, notes)?;
    let thickness = thickness(board, notes)?;
    let Regions { cutouts, others } = match regions6 {
        Some(data) => regions(data, shaped, notes)?,
        None => Regions::default(),
    };
    let origin = (board.length_mm("ORIGINX")?, board.length_mm("ORIGINY")?);
    if let (Some(x), Some(y)) = origin {
        if x != 0.0 || y != 0.0 {
            notes.push(Note {
                severity: Severity::Warning,
                message: format!(
                    "the board's origin is at ({x:.3}, {y:.3}) mm; coordinates are kept as stored, \
                     not moved to that origin"
                ),
            });
        }
    }
    Ok(ShapeAndThickness {
        outline: Outline { outer, cutouts },
        thickness,
        other_regions: others,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(text: &str) -> Vec<u8> {
        let mut bytes = (text.len() as u32 + 1).to_le_bytes().to_vec();
        bytes.extend_from_slice(text.as_bytes());
        bytes.push(0);
        bytes
    }

    #[test]
    fn lengths_parse_in_mil_only() {
        assert!((parse_length_mm("K", "1181.1024mil").unwrap() - 30.0).abs() < 0.001);
        assert!(parse_length_mm("K", "30mm").is_err());
        assert!(parse_length_mm("K", "abcmil").is_err());
        assert!(parse_length_mm("K", "1e300mil").is_err());
    }

    #[test]
    fn a_square_outline_closes_without_a_doubled_corner() {
        let rec = record(
            "|KIND0=0|VX0=0mil|VY0=0mil|KIND1=0|VX1=100mil|VY1=0mil|KIND2=0|VX2=100mil|VY2=100mil\
             |KIND3=0|VX3=0mil|VY3=100mil|KIND4=0|VX4=0mil|VY4=0mil",
        );
        let mut notes = Vec::new();
        let c = outline(&board_record(&rec).unwrap(), &mut notes).unwrap();
        assert_eq!(c.segments.len(), 4);
        assert!(notes.is_empty());
    }

    #[test]
    fn hostile_board_records_are_errors_not_panics() {
        let mut notes = Vec::new();
        assert!(board_record(&[]).is_err());
        assert!(board_record(&[0xff, 0xff, 0xff, 0xff, b'|']).is_err());
        let few = record("|KIND0=0|VX0=0mil|VY0=0mil");
        assert!(outline(&board_record(&few).unwrap(), &mut notes).is_err());
        let bad = record("|KIND0=0|VX0=xmil|VY0=0mil");
        assert!(outline(&board_record(&bad).unwrap(), &mut notes).is_err());
    }

    #[test]
    fn hostile_region_streams_are_errors_not_panics() {
        let mut notes = Vec::new();
        assert!(regions(&[REGION_RECORD], &[], &mut notes).is_err());
        assert!(regions(&[REGION_RECORD, 0xff, 0xff, 0xff, 0x7f], &[], &mut notes).is_err());
        // A text length that runs past the end of its record.
        let mut long_text = vec![REGION_RECORD, 30, 0, 0, 0];
        long_text.extend_from_slice(&[0u8; 30]);
        long_text[5 + REGION_SKIPPED..5 + REGION_SKIPPED + 4].copy_from_slice(&[0xff; 4]);
        assert!(regions(&long_text, &[], &mut notes).is_err());
        // An empty text is a region that is not a cutout; without corners it is a note, not an error.
        let mut plain = vec![REGION_RECORD, 30, 0, 0, 0];
        plain.extend_from_slice(&[0u8; 30]);
        assert!(regions(&plain, &[], &mut notes).unwrap().cutouts.is_empty());
        // A cutout whose vertex count runs past the record.
        let text = b"|ISBOARDCUTOUT=TRUE\0";
        let mut body = vec![0u8; REGION_SKIPPED];
        body.extend_from_slice(&(text.len() as u32).to_le_bytes());
        body.extend_from_slice(text);
        body.extend_from_slice(&1000u32.to_le_bytes());
        let mut cut = vec![REGION_RECORD];
        cut.extend_from_slice(&(body.len() as u32).to_le_bytes());
        cut.extend_from_slice(&body);
        assert!(regions(&cut, &[], &mut notes).is_err());
        assert!(regions(&[0x42, 1, 2, 3], &[], &mut notes)
            .unwrap()
            .cutouts
            .is_empty());
        assert_eq!(notes.last().map(|n| n.severity), Some(Severity::Dropped));
    }

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn arc(cx: f64, cy: f64, r: f64, start_deg: f64, end_deg: f64) -> Option<Arc> {
        Some(Arc {
            center: p(cx, cy),
            radius: r,
            start_deg,
            end_deg,
        })
    }

    /// A 10 x 10 square with its lower right corner rounded (r 2), listed `ccw` or clockwise.
    fn rounded_square(ccw: bool) -> Vec<Vertex> {
        let v = |x, y, a| Vertex {
            at: p(x, y),
            arc: a,
        };
        if ccw {
            vec![
                v(0.0, 0.0, None),
                v(8.0, 0.0, arc(8.0, 2.0, 2.0, 270.0, 360.0)),
                v(10.0, 2.0, None),
                v(10.0, 10.0, None),
                v(0.0, 10.0, None),
            ]
        } else {
            // Altium's way: the arc's vertex at its end angle, the next one at its start.
            vec![
                v(0.0, 0.0, None),
                v(0.0, 10.0, None),
                v(10.0, 10.0, None),
                v(10.0, 2.0, arc(8.0, 2.0, 2.0, 270.0, 360.0)),
                v(8.0, 0.0, None),
                v(0.0, 0.0, None),
            ]
        }
    }

    fn arcs_of(c: &Contour) -> usize {
        c.segments
            .iter()
            .filter(|s| matches!(s, Segment::Arc { .. }))
            .count()
    }

    /// True when every segment starts where the one before it ends.
    fn chained(c: &Contour) -> bool {
        let ends = |s: &Segment| match *s {
            Segment::Line { from, to } => (from, to),
            Segment::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                let a = Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                };
                (on_circle(&a, start_deg), on_circle(&a, end_deg))
            }
        };
        let n = c.segments.len();
        (0..n).all(|i| near(ends(&c.segments[(i + n - 1) % n]).1, ends(&c.segments[i]).0))
    }

    #[test]
    fn arcs_listed_either_way_give_one_counter_clockwise_chain() {
        for ccw in [true, false] {
            let mut notes = Vec::new();
            let c = contour("test", &rounded_square(ccw), &mut notes);
            assert_eq!((c.segments.len(), arcs_of(&c)), (5, 1), "{ccw}");
            assert!(chained(&c), "{ccw}: {c:?}");
            assert!(notes.is_empty(), "{notes:?}");
        }
    }

    #[test]
    fn a_stray_arc_or_one_against_the_rest_becomes_a_line() {
        // An arc whose ends are not its vertices.
        let mut square = rounded_square(true);
        square[1].arc = arc(5.0, 5.0, 1.0, 0.0, 90.0);
        let mut notes = Vec::new();
        let c = contour("test", &square, &mut notes);
        assert_eq!((c.segments.len(), arcs_of(&c)), (5, 0));
        assert!(chained(&c));
        assert_eq!(notes.len(), 1);
        // A slot (two counter-clockwise half circles) with a dent turning the other way.
        let v = |x, y, a| Vertex {
            at: p(x, y),
            arc: a,
        };
        let dented = [
            v(0.0, 0.0, None),
            v(4.0, 0.0, arc(5.0, 0.0, 1.0, 0.0, 180.0)),
            v(6.0, 0.0, None),
            v(10.0, 0.0, arc(10.0, 5.0, 5.0, 270.0, 90.0)),
            v(10.0, 10.0, None),
            v(0.0, 10.0, arc(0.0, 5.0, 5.0, 90.0, 270.0)),
        ];
        let mut notes = Vec::new();
        let c = contour("test", &dented, &mut notes);
        assert_eq!((c.segments.len(), arcs_of(&c)), (6, 2));
        assert!(chained(&c));
        assert!(
            notes[0].message.contains("1 arc(s) turn against"),
            "{notes:?}"
        );
    }

    #[test]
    fn an_outline_with_arcs_reads_from_the_board_record() {
        // Two half circles of radius 50 mil round a 100 x 100 mil square's left and right.
        let rec = record(
            "|KIND0=1|VX0=0mil|VY0=0mil|CX0=0mil|CY0=50mil|R0=50mil|SA0= 9.0E+0001|EA0=270\
             |KIND1=0|VX1=0mil|VY1=100mil\
             |KIND2=1|VX2=100mil|VY2=100mil|CX2=100mil|CY2=50mil|R2=50mil|SA2=-90|EA2=90\
             |KIND3=0|VX3=100mil|VY3=0mil|KIND4=0|VX4=0mil|VY4=0mil",
        );
        let mut notes = Vec::new();
        let c = outline(&board_record(&rec).unwrap(), &mut notes).unwrap();
        assert_eq!((c.segments.len(), arcs_of(&c)), (4, 2));
        assert!(chained(&c));
        assert!(notes.is_empty(), "{notes:?}");
        // An arc without its radius is an error, an unknown KIND a straight line and a warning.
        let no_radius = record("|KIND0=1|VX0=0mil|VY0=0mil|CX0=0mil|CY0=0mil|SA0=0|EA0=90");
        assert!(outline(&board_record(&no_radius).unwrap(), &mut notes).is_err());
        let rec = record(
            "|KIND0=7|VX0=0mil|VY0=0mil|KIND1=0|VX1=100mil|VY1=0mil|KIND2=0|VX2=0mil|VY2=100mil",
        );
        let c = outline(&board_record(&rec).unwrap(), &mut notes).unwrap();
        assert_eq!((c.segments.len(), arcs_of(&c)), (3, 0));
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn a_missing_stack_is_a_note_not_an_error() {
        let mut notes = Vec::new();
        let t = thickness(&board_record(&record("|KIND0=0")).unwrap(), &mut notes).unwrap();
        assert_eq!(t, None);
        assert_eq!(notes.len(), 1);
    }
}
