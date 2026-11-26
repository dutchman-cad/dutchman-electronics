//! Internal planes: the copper of each plane layer, with the clearances and thermal reliefs Altium
//! cuts around every via and through-hole pad.
//!
//! Worked out from our own CC0 test board `four-layer-planes` and the numbers in its README.txt
//! (see LEGAL.md), comparing the file with the Gerber files Altium wrote for its two planes:
//!
//! - `/Polygons6/Data` is a run of records, each a `u32` length and text. A plane's copper is
//!   one record per region with `POLYGONTYPE=Split Plane`, `LAYER=PLANE1` (or `PLANE2`, … for
//!   layer byte 38 + n, see `copper.rs`), the region's net as `NET=` and an index into
//!   `/Nets6/Data` (no `NET` key: no net), and its outline as the same `KINDn`/`VXn`/`VYn`/…
//!   vertex list as the board shape (see `board.rs`). The outline already leaves out the
//!   pull-back from the board edge (0.508 in from the 50 x 30 outline) and the split line (0.25
//!   either side of x = 25): Layer 1 is one region, Layer 2 two. Each lies a few micrometers
//!   inside where we expected (0.51046, 24.74784, 25.25216): that is the file, not rounding.
//! - Altium stores no clearance or relief shapes. It cuts them from two design rules in
//!   `/Rules6/Data` (records of a `u16` kind, a `u32` length and text, like those of
//!   `/Nets6/Data` after the kind): `RULEKIND=PlaneClearance` with `CLEARANCE`, and
//!   `RULEKIND=PlaneConnect` with `PLANECONNECTSTYLE=Relief`, `RELIEFEXPANSION`,
//!   `RELIEFAIRGAP`, `RELIEFCONDUCTORWIDTH` and `RELIEFENTRIES`. Both of ours apply to every
//!   object (`SCOPE1EXPRESSION=All`, `SCOPE2EXPRESSION=All`).
//! - The Gerber files give their shapes. A via or pad whose net is not the region's gets a round
//!   clearance of its hole plus twice `CLEARANCE` (0.9 around a 0.3 hole, 1.5 around a 0.9 hole,
//!   with `CLEARANCE` 0.3). One of the region's net gets a thermal relief: a ring of air from
//!   the hole plus twice `RELIEFEXPANSION` (1.316 around a 0.3 hole with 0.508) out to that plus
//!   twice `RELIEFAIRGAP` (1.824 with 0.254), crossed by `RELIEFENTRIES` (4) copper spokes
//!   `RELIEFCONDUCTORWIDTH` (0.254) wide at 45, 135, 225 and 315 degrees. Only the hole is
//!   inside the ring; the copper between it and the ring is the plane's.
//!
//! Each region becomes a [`CopperItem::Region`] on its plane layer, its holes the clearances and
//! the four pieces of air of each relief. Only board-wide rules are read: a rule for some objects
//! only, or an override on one via or pad, is left out with a note.

use board_model::{Contour, CopperItem, Note, Point, Segment, Severity};

use crate::board::{vertex_contour, Properties};
use crate::components::text_records;
use crate::copper::{CopperLayers, Drill};

/// The most rule records read before the file is treated as hostile.
const MAX_RULES: usize = 100_000;
/// The plane-clearance rule Altium gave our board, used when a file has none: 20 mil.
const DEFAULT_CLEARANCE: f64 = 0.508;
/// The plane-connect rule Altium gave our board, used when a file has none: a relief with
/// 20 mil expansion, 10 mil air gap, 10 mil spokes, 4 of them.
const DEFAULT_CONNECT: Connect = Connect::Relief {
    expansion: 0.508,
    air_gap: 0.254,
    width: 0.254,
};
/// The relief spokes seen on our board, at 45 degrees plus each quarter turn.
const SPOKES_DEG: [f64; 4] = [45.0, 135.0, 225.0, 315.0];
/// Arcs are cut into straight pieces of at most this many degrees to test where a hole lies.
const FLATTEN_STEP_DEG: f64 = 5.0;

/// How a via or pad of a plane's own net joins the plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Connect {
    /// A thermal relief: a ring of air `air_gap` wide whose inner edge is `expansion` out from
    /// the hole, crossed by four spokes `width` wide.
    Relief {
        expansion: f64,
        air_gap: f64,
        width: f64,
    },
}

/// The board-wide plane rules.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rules {
    /// How far a plane keeps from the hole of a via or pad of another net.
    pub clearance: f64,
    pub connect: Connect,
}

/// Reads the board-wide `PlaneClearance` and `PlaneConnect` rules from `/Rules6/Data`, with the
/// values Altium gave our board for any the file lacks.
pub(crate) fn rules(data: Option<&[u8]>, notes: &mut Vec<Note>) -> Result<Rules, String> {
    let records = match data {
        Some(data) => rule_records(data)?,
        None => Vec::new(),
    };
    let mut narrower = 0usize;
    let mut pick = |kind: &str| -> Option<&Properties> {
        let mut best: Option<(i64, &Properties)> = None;
        for rule in &records {
            if rule.get("RULEKIND").map(str::trim) != Some(kind)
                || rule.get("ENABLED").map(str::trim) == Some("FALSE")
            {
                continue;
            }
            let all = |key: &str| rule.get(key).is_none_or(|v| v.trim() == "All");
            if !(all("SCOPE1EXPRESSION") && all("SCOPE2EXPRESSION")) {
                narrower += 1;
                continue;
            }
            let priority = rule
                .get("PRIORITY")
                .and_then(|p| p.trim().parse().ok())
                .unwrap_or(i64::MAX);
            if best.is_none_or(|(p, _)| priority < p) {
                best = Some((priority, rule));
            }
        }
        best.map(|(_, rule)| rule)
    };
    let clearance_rule = pick("PlaneClearance");
    let connect_rule = pick("PlaneConnect");
    let mut missing = Vec::new();
    let clearance = match clearance_rule {
        Some(rule) => rule.length_mm("CLEARANCE")?,
        None => None,
    }
    .unwrap_or_else(|| {
        missing.push("PlaneClearance");
        DEFAULT_CLEARANCE
    });
    let connect = match connect_rule {
        Some(rule) => connect_of(rule, notes)?,
        None => {
            missing.push("PlaneConnect");
            DEFAULT_CONNECT
        }
    };
    if !missing.is_empty() {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "no board-wide {} rule: Altium's default values are used for the planes",
                missing.join(" or ")
            ),
        });
    }
    if narrower > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{narrower} plane rule(s) for some objects only are not read; the board-wide \
                 rule is used for every via and pad"
            ),
        });
    }
    Ok(Rules { clearance, connect })
}

/// The connection a `PlaneConnect` rule gives.
fn connect_of(rule: &Properties, notes: &mut Vec<Note>) -> Result<Connect, String> {
    let style = rule.get("PLANECONNECTSTYLE").map(str::trim).unwrap_or("");
    if style != "Relief" {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "plane connect style {style:?} is not read yet; vias and pads of a plane's net \
                 are drawn with a thermal relief"
            ),
        });
    }
    let entries = rule.get("RELIEFENTRIES").map(str::trim);
    if entries.is_some_and(|e| e != "4") {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "thermal reliefs with {} spokes are not read yet; drawn with 4",
                entries.unwrap_or_default()
            ),
        });
    }
    let length = |key: &str, default: f64| -> Result<f64, String> {
        Ok(rule.length_mm(key)?.unwrap_or(default))
    };
    let Connect::Relief {
        expansion,
        air_gap,
        width,
    } = DEFAULT_CONNECT;
    Ok(Connect::Relief {
        expansion: length("RELIEFEXPANSION", expansion)?,
        air_gap: length("RELIEFAIRGAP", air_gap)?,
        width: length("RELIEFCONDUCTORWIDTH", width)?,
    })
}

/// Splits `/Rules6/Data` into its text records: a `u16` kind, a `u32` length, then the text.
pub(crate) fn rule_records(data: &[u8]) -> Result<Vec<Properties>, String> {
    let mut records = Vec::new();
    let mut at = 0usize;
    while at < data.len() {
        if records.len() >= MAX_RULES {
            return Err(format!("/Rules6/Data has more than {MAX_RULES} records"));
        }
        let len = at
            .checked_add(2)
            .and_then(|start| data.get(start..start.checked_add(4)?))
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
            .map(u32::from_le_bytes)
            .and_then(|l| usize::try_from(l).ok())
            .ok_or("/Rules6/Data: length cut short")?;
        let start = at + 6;
        let end = start
            .checked_add(len)
            .filter(|&e| e <= data.len())
            .ok_or("/Rules6/Data: record is longer than the stream")?;
        records.push(Properties::parse(
            data.get(start..end)
                .ok_or("/Rules6/Data: record out of range")?,
        ));
        at = end;
    }
    Ok(records)
}

/// What [`read`] found in `/Polygons6/Data`.
#[derive(Debug, Default)]
pub(crate) struct Planes {
    /// The copper of each split-plane region.
    pub copper: Vec<CopperItem>,
    /// Polygon records that are not split planes (polygon pours, whose copper is read from
    /// `/Regions6/Data` instead).
    pub others: usize,
}

/// Reads the split-plane regions of `/Polygons6/Data` as copper regions on their plane layers,
/// with a clearance or a thermal relief cut around each of `drills` that reaches them.
pub(crate) fn read(
    data: &[u8],
    layers: &CopperLayers,
    drills: &[Drill],
    rules: &Rules,
    notes: &mut Vec<Note>,
) -> Result<Planes, String> {
    let mut planes = Planes::default();
    let mut no_layer = 0usize;
    let mut unreadable = 0usize;
    for record in text_records(data, "/Polygons6/Data")? {
        if record.get("POLYGONTYPE").map(str::trim) != Some("Split Plane") {
            planes.others += 1;
            continue;
        }
        let layer = record
            .get("LAYER")
            .and_then(|l| l.trim().strip_prefix("PLANE"))
            .and_then(|n| n.parse::<u8>().ok())
            .and_then(|n| n.checked_add(38))
            .and_then(|byte| layers.plane_index(byte));
        let Some(layer) = layer else {
            no_layer += 1;
            continue;
        };
        let Ok(contour) = vertex_contour(&record, "a split-plane region", notes) else {
            unreadable += 1;
            continue;
        };
        let net = record
            .get("NET")
            .and_then(|n| n.trim().parse::<usize>().ok())
            .and_then(|n| layers.net_name(n));
        let edge = flatten(&contour);
        let mut holes = Vec::new();
        for drill in drills {
            let inside = contains(&edge, drill.at);
            let own = inside && net.is_some() && drill.net.as_deref() == net;
            if own {
                let Connect::Relief {
                    expansion,
                    air_gap,
                    width,
                } = rules.connect;
                let inner = drill.hole / 2.0 + expansion;
                holes.extend(relief(drill.at, inner, inner + air_gap, width));
            } else {
                let radius = drill.hole / 2.0 + rules.clearance;
                if inside || distance(&edge, drill.at) < radius {
                    holes.push(circle(drill.at, radius));
                }
            }
        }
        planes.copper.push(CopperItem::Region {
            layer,
            contour,
            holes,
        });
    }
    if no_layer > 0 {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!(
                "{no_layer} split-plane region(s) name a plane that is not in the layer stack"
            ),
        });
    }
    if unreadable > 0 {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!("{unreadable} split-plane region(s) have an outline that is not read"),
        });
    }
    if !planes.copper.is_empty() {
        notes.push(Note {
            severity: Severity::Info,
            message: format!(
                "{} split-plane region(s) read as plane copper, with clearances and thermal \
                 reliefs from the board-wide plane rules (overrides on single vias or pads are \
                 not read)",
                planes.copper.len()
            ),
        });
    }
    Ok(planes)
}

fn on_circle(center: Point, radius: f64, deg: f64) -> Point {
    let (sin, cos) = deg.to_radians().sin_cos();
    Point {
        x: center.x + radius * cos,
        y: center.y + radius * sin,
    }
}

/// A whole circle, as two half arcs.
fn circle(center: Point, radius: f64) -> Contour {
    let half = |start_deg: f64| Segment::Arc {
        center,
        radius,
        start_deg,
        end_deg: start_deg + 180.0,
        clockwise: false,
    };
    Contour {
        segments: vec![half(0.0), half(180.0)],
    }
}

/// The air of a thermal relief: the ring from `inner` to `outer` (radii about `center`) less
/// four spokes `width` wide at [`SPOKES_DEG`], as four closed pieces. Each runs along the outer
/// edge counter-clockwise from one spoke to the next, then back along the inner edge.
fn relief(center: Point, inner: f64, outer: f64, width: f64) -> Vec<Contour> {
    let half_angle = |r: f64| ((width / 2.0) / r).clamp(-1.0, 1.0).asin().to_degrees();
    let (inner_half, outer_half) = (half_angle(inner), half_angle(outer));
    let mut pieces = Vec::new();
    for (i, &from) in SPOKES_DEG.iter().enumerate() {
        let to = SPOKES_DEG
            .get(i + 1)
            .copied()
            .unwrap_or(SPOKES_DEG[0] + 360.0);
        let (outer_start, outer_end) = (from + outer_half, to - outer_half);
        let (inner_start, inner_end) = (from + inner_half, to - inner_half);
        if outer_end <= outer_start || inner_end <= inner_start {
            // Spokes as wide as the ring leave no air.
            continue;
        }
        pieces.push(Contour {
            segments: vec![
                Segment::Arc {
                    center,
                    radius: outer,
                    start_deg: outer_start,
                    end_deg: outer_end,
                    clockwise: false,
                },
                Segment::Line {
                    from: on_circle(center, outer, outer_end),
                    to: on_circle(center, inner, inner_end),
                },
                Segment::Arc {
                    center,
                    radius: inner,
                    start_deg: inner_end,
                    end_deg: inner_start,
                    clockwise: true,
                },
                Segment::Line {
                    from: on_circle(center, inner, inner_start),
                    to: on_circle(center, outer, outer_start),
                },
            ],
        });
    }
    pieces
}

/// The corners of a contour, its arcs cut into short straight pieces.
fn flatten(contour: &Contour) -> Vec<Point> {
    let mut points = Vec::new();
    for segment in &contour.segments {
        match *segment {
            Segment::Line { from, .. } => points.push(from),
            Segment::Arc {
                center,
                radius,
                start_deg,
                ..
            } => {
                let sweep = segment.sweep_deg().unwrap_or(0.0);
                // At most 72 pieces for a whole turn; the cast is of a small positive number.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let steps = (sweep.abs() / FLATTEN_STEP_DEG).ceil().clamp(1.0, 72.0) as usize;
                for k in 0..steps {
                    #[allow(clippy::cast_precision_loss)]
                    let deg = start_deg + sweep * (k as f64) / (steps as f64);
                    points.push(on_circle(center, radius, deg));
                }
            }
        }
    }
    points
}

/// Whether `p` lies inside the closed polygon `points` (even-odd rule).
fn contains(points: &[Point], p: Point) -> bool {
    let mut inside = false;
    let next = points.iter().skip(1).chain(points.first());
    for (a, b) in points.iter().zip(next) {
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if p.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// The shortest distance from `p` to the edge of the closed polygon `points`.
fn distance(points: &[Point], p: Point) -> f64 {
    let next = points.iter().skip(1).chain(points.first());
    points
        .iter()
        .zip(next)
        .map(|(a, b)| {
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len2 = dx * dx + dy * dy;
            let t = if len2 > 0.0 {
                (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (a.x + t * dx - p.x).hypot(a.y + t * dy - p.y)
        })
        .fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn rule(kind_code: u16, text: &str) -> Vec<u8> {
        let mut out = kind_code.to_le_bytes().to_vec();
        out.extend(u32::try_from(text.len()).unwrap().to_le_bytes());
        out.extend(text.as_bytes());
        out
    }

    #[test]
    fn the_board_wide_rules_are_read_and_narrower_ones_noted() {
        let mut data = rule(
            0x31,
            "|RULEKIND=PlaneClearance|ENABLED=TRUE|PRIORITY=1|SCOPE1EXPRESSION=All\
             |SCOPE2EXPRESSION=All|CLEARANCE=11.811mil",
        );
        data.extend(rule(
            0x32,
            "|RULEKIND=PlaneConnect|ENABLED=TRUE|PRIORITY=1|SCOPE1EXPRESSION=All\
             |SCOPE2EXPRESSION=All|PLANECONNECTSTYLE=Relief|RELIEFEXPANSION=20mil\
             |RELIEFENTRIES=4|RELIEFCONDUCTORWIDTH=10mil|RELIEFAIRGAP=10mil",
        ));
        data.extend(rule(
            0x31,
            "|RULEKIND=PlaneClearance|ENABLED=TRUE|PRIORITY=2|SCOPE1EXPRESSION=InNet('GND')\
             |SCOPE2EXPRESSION=All|CLEARANCE=40mil",
        ));
        let mut notes = Vec::new();
        let r = rules(Some(&data), &mut notes).unwrap();
        assert!((r.clearance - 0.3).abs() < 1e-4, "{r:?}");
        assert_eq!(
            r.connect,
            Connect::Relief {
                expansion: 20.0 * 0.0254,
                air_gap: 10.0 * 0.0254,
                width: 10.0 * 0.0254,
            }
        );
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0]
            .message
            .contains("1 plane rule(s) for some objects"));
    }

    #[test]
    fn missing_rules_fall_back_with_a_note_and_hostile_ones_are_errors() {
        let mut notes = Vec::new();
        let r = rules(None, &mut notes).unwrap();
        assert_eq!(r.clearance, DEFAULT_CLEARANCE);
        assert_eq!(r.connect, DEFAULT_CONNECT);
        assert!(notes[0].message.contains("PlaneClearance or PlaneConnect"));

        let mut notes = Vec::new();
        assert!(rules(Some(&[1, 0, 0xff, 0xff, 0xff, 0x7f]), &mut notes).is_err());
        assert!(rules(Some(&[1, 0, 9]), &mut notes).is_err());
        let bad = rule(
            0x31,
            "|RULEKIND=PlaneClearance|SCOPE1EXPRESSION=All|CLEARANCE=xmil",
        );
        assert!(rules(Some(&bad), &mut notes).is_err());
    }

    #[test]
    fn a_relief_is_four_pieces_of_ring_between_spokes_at_45_degrees() {
        let pieces = relief(p(10.0, 10.0), 0.658, 0.912, 0.254);
        assert_eq!(pieces.len(), 4);
        for piece in &pieces {
            let corners = flatten(piece);
            // Every corner lies on the inner or the outer circle.
            for c in &corners {
                let r = (c.x - 10.0).hypot(c.y - 10.0);
                assert!((r - 0.658).abs() < 1e-9 || (r - 0.912).abs() < 1e-9, "{r}");
            }
            // No corner is closer to a spoke's center line than half the spoke width.
            for c in &corners {
                for spoke in SPOKES_DEG {
                    let (s, k) = spoke.to_radians().sin_cos();
                    let (dx, dy) = (c.x - 10.0, c.y - 10.0);
                    let along = dx * k + dy * s;
                    let across = (-dx * s + dy * k).abs();
                    if along > 0.0 {
                        assert!(across > 0.127 - 1e-9, "{across}");
                    }
                }
            }
        }
        // The first piece runs from just past 45 degrees to just short of 135, outside first.
        let first = &pieces[0].segments[0];
        let Segment::Arc {
            start_deg, end_deg, ..
        } = *first
        else {
            panic!("{first:?}");
        };
        let half = (0.127f64 / 0.912).asin().to_degrees();
        assert!((start_deg - (45.0 + half)).abs() < 1e-9);
        assert!((end_deg - (135.0 - half)).abs() < 1e-9);
        // Spokes as wide as the ring leave no air.
        assert!(relief(p(0.0, 0.0), 0.1, 0.2, 1.0).is_empty());
    }

    #[test]
    fn holes_are_found_inside_and_near_a_region() {
        let square = flatten(&circle(p(0.0, 0.0), 1.0));
        assert!(contains(&square, p(0.5, 0.0)));
        assert!(!contains(&square, p(1.5, 0.0)));
        assert!((distance(&square, p(1.5, 0.0)) - 0.5).abs() < 0.01);
    }
}
