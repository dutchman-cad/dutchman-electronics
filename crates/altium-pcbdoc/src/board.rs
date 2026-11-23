//! Roadmap step 2: the board outline, its cutouts and the board thickness.
//!
//! Everything here was worked out from our own CC0 test boards in `tests/boards/` and the numbers
//! in their README.txt (see LEGAL.md):
//!
//! - `/Board6/Data` holds one record: a little-endian `u32` length, then text of the form
//!   `|KEY=VALUE|KEY=VALUE…` ending in a NUL byte.
//! - The board shape is a numbered vertex list in that record: `KINDn`, `VXn`, `VYn` (and `CXn`,
//!   `CYn`, `SAn`, `EAn`, `Rn`, all zero on our boards, whose edges are all straight with
//!   `KINDn=0`). The last vertex repeats the first. Lengths are text such as `1181.1024mil`.
//! - The layer stack is in `V9_STACK_LAYERn_…` keys: each solder mask and dielectric has a
//!   `DIELHEIGHT`, each copper layer a `COPTHICK`. Their sum is the 1.600 mm we typed in.
//! - `/Regions6/Data` is a run of records: a type byte (`0x0b`), a `u32` length, then 18 bytes we
//!   do not read yet, a `u32` text length, the text (with `ISBOARDCUTOUT=TRUE` for a board
//!   cutout), a `u32` vertex count, and that many pairs of little-endian `f64` (x, y) in
//!   1/10000 mil. The polygon does not repeat its first vertex.

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

/// The board shape from the `KINDn`/`VXn`/`VYn` vertex list.
pub(crate) fn outline(board: &Properties, notes: &mut Vec<Note>) -> Result<Contour, String> {
    let mut points = Vec::new();
    let mut arcs = 0usize;
    for i in 0..=MAX_VERTICES {
        let Some(kind) = board.get(&format!("KIND{i}")) else {
            break;
        };
        if i == MAX_VERTICES {
            return Err(format!(
                "the board outline has more than {MAX_VERTICES} vertices"
            ));
        }
        if kind.trim() != "0" {
            arcs += 1;
        }
        let x = board.length_mm(&format!("VX{i}"))?;
        let y = board.length_mm(&format!("VY{i}"))?;
        match (x, y) {
            (Some(x), Some(y)) => points.push(Point { x, y }),
            _ => return Err(format!("outline vertex {i} has no VX{i}/VY{i}")),
        }
    }
    if points.len() < 3 {
        return Err(format!(
            "the board outline has {} vertices; a board needs at least 3",
            points.len()
        ));
    }
    if arcs > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{arcs} board outline edge(s) are not straight (KIND not 0); read as straight lines"
            ),
        });
    }
    Ok(closed_polygon(&points))
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

/// The board cutouts from `/Regions6/Data`. Other regions are counted in a note, not read.
pub(crate) fn cutouts(data: &[u8], notes: &mut Vec<Note>) -> Result<Vec<Contour>, String> {
    let mut contours = Vec::new();
    let mut other = 0usize;
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
        match region(body)? {
            Some(contour) => contours.push(contour),
            None => other += 1,
        }
        at = body_end;
    }
    if other > 0 {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!(
                "/Regions6/Data: {other} region(s) that are not board cutouts were not read yet"
            ),
        });
    }
    Ok(contours)
}

/// One region record body: `Some(contour)` for a board cutout, `None` for any other region.
fn region(body: &[u8]) -> Result<Option<Contour>, String> {
    let text_len = read_u32(body, REGION_SKIPPED).ok_or("region text length cut short")?;
    let text_start = REGION_SKIPPED + 4;
    let text_end = text_start
        .checked_add(text_len)
        .ok_or("region text length out of range")?;
    let text = body
        .get(text_start..text_end)
        .ok_or("region text is longer than its record")?;
    let props = Properties::parse(text);
    if props.get("ISBOARDCUTOUT") != Some("TRUE") {
        return Ok(None);
    }
    let n = read_u32(body, text_end).ok_or("region vertex count cut short")?;
    if n > MAX_VERTICES {
        return Err(format!(
            "a board cutout has {n} vertices, more than {MAX_VERTICES}"
        ));
    }
    if n < 3 {
        return Err(format!(
            "a board cutout has {n} vertices; it needs at least 3"
        ));
    }
    let mut points = Vec::with_capacity(n);
    for k in 0..n {
        let at = text_end + 4 + 16 * k;
        let x = read_f64(body, at).ok_or("board cutout vertices cut short")?;
        let y = read_f64(body, at + 8).ok_or("board cutout vertices cut short")?;
        points.push(Point {
            x: unit_to_mm(x)?,
            y: unit_to_mm(y)?,
        });
    }
    Ok(Some(closed_polygon(&points)))
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
fn unit_to_mm(v: f64) -> Result<f64, String> {
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
}

/// Reads the board shape, cutouts and thickness from the two streams.
pub(crate) fn read(
    board6: &[u8],
    regions6: Option<&[u8]>,
    notes: &mut Vec<Note>,
) -> Result<ShapeAndThickness, String> {
    let board = board_record(board6)?;
    let outer = outline(&board, notes)?;
    let thickness = thickness(&board, notes)?;
    let cutouts = match regions6 {
        Some(data) => cutouts(data, notes)?,
        None => Vec::new(),
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
        assert!(cutouts(&[REGION_RECORD], &mut notes).is_err());
        assert!(cutouts(&[REGION_RECORD, 0xff, 0xff, 0xff, 0x7f], &mut notes).is_err());
        // A text length that runs past the end of its record.
        let mut long_text = vec![REGION_RECORD, 30, 0, 0, 0];
        long_text.extend_from_slice(&[0u8; 30]);
        long_text[5 + REGION_SKIPPED..5 + REGION_SKIPPED + 4].copy_from_slice(&[0xff; 4]);
        assert!(cutouts(&long_text, &mut notes).is_err());
        // An empty text is a region that is not a cutout: reported, not an error.
        let mut plain = vec![REGION_RECORD, 30, 0, 0, 0];
        plain.extend_from_slice(&[0u8; 30]);
        assert!(cutouts(&plain, &mut notes).unwrap().is_empty());
        // A cutout whose vertex count runs past the record.
        let text = b"|ISBOARDCUTOUT=TRUE\0";
        let mut body = vec![0u8; REGION_SKIPPED];
        body.extend_from_slice(&(text.len() as u32).to_le_bytes());
        body.extend_from_slice(text);
        body.extend_from_slice(&1000u32.to_le_bytes());
        let mut cut = vec![REGION_RECORD];
        cut.extend_from_slice(&(body.len() as u32).to_le_bytes());
        cut.extend_from_slice(&body);
        assert!(cutouts(&cut, &mut notes).is_err());
        assert!(cutouts(&[0x42, 1, 2, 3], &mut notes).unwrap().is_empty());
        assert_eq!(notes.last().map(|n| n.severity), Some(Severity::Dropped));
    }

    #[test]
    fn a_missing_stack_is_a_note_not_an_error() {
        let mut notes = Vec::new();
        let t = thickness(&board_record(&record("|KIND0=0")).unwrap(), &mut notes).unwrap();
        assert_eq!(t, None);
        assert_eq!(notes.len(), 1);
    }
}
