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
//! the four pieces of air of each relief.
//!
//! Rules for some objects (`plane-polygon-connect`, from `four-layer-planes`: free GND pads R, D
//! and N at (6, 25), (12, 25) and (18, 25) on the GND plane Layer 1): `PlaneConnect_N` with
//! `SCOPE1EXPRESSION=IsPad And (Name = 'Free-N')`, `PLANECONNECTSTYLE=NoConnect` and
//! `PRIORITY=1`, `PlaneConnect_D` the same for `Free-D` with `Direct` and 2, and the board's
//! own `All` rule (Relief) at 3. As everywhere in Altium the rule with the lowest priority
//! number that takes an object wins. `Direct` joins the pad to the plane with no relief, so the
//! plane keeps only the drill; `NoConnect` keeps the plane away as from another net's pad, a
//! round clearance of the hole plus twice `CLEARANCE` (1.400 across with 0.3); our Layer 2,
//! another net, has exactly that round all three pads. Altium's own Gerber of Layer 1
//! (`tests/gerber/plane-polygon-connect.GP1`) confirms all three: a thermal of 2.324 / 1.816 with
//! 0.254 spokes at 45 degrees round R, no flash at D, a 1.400 circle round N. Altium's Top Layer
//! pour shows the same
//! three joins poured in (four relief gaps round R, none round D, a round clearance round N),
//! and is read as poured (see `board.rs`).
//!
//! The scopes read exactly: `All`, and `IsPad And (Name = '…')` for a free pad (`Free-` and its
//! designator). Any other scope, or a name scope on a component's pad, whose query name no test
//! board shows yet, is not read, and a note names the rule.

use board_model::{Contour, CopperItem, Note, Point, Segment, Severity};

use crate::board::{vertex_contour, Properties};
use crate::components::text_records;
use crate::copper::{CopperLayers, Drill, DrillOf};

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
/// The pull-back from the board edge Altium gives a new plane, used when the stack has none: 20 mil.
const DEFAULT_PULL_BACK: f64 = 0.508;
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
    /// Solid copper up to the hole.
    Direct,
    /// Not joined: kept away as from another net's via or pad.
    Isolated,
}

/// A `PlaneConnect` rule for some objects only, in a scope that is read exactly.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ObjectRule {
    pub name: String,
    pub priority: i64,
    /// The free pad's query name the rule takes (`Free-D`).
    pub pad: String,
    pub connect: Connect,
}

/// The plane rules: the board-wide ones, and the connect rules for single pads.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Rules {
    /// How far a plane keeps from the hole of a via or pad of another net.
    pub clearance: f64,
    pub connect: Connect,
    /// The board-wide connect rule's priority (`i64::MAX` when the file has none).
    pub connect_priority: i64,
    /// Connect rules for single pads, by priority, best first.
    pub for_pads: Vec<ObjectRule>,
}

impl Rules {
    /// How a via or pad joins its own net's plane: the best of the rules that take it.
    fn connect_for(&self, of: &DrillOf) -> Connect {
        if let DrillOf::Pad(name) = of {
            if let Some(rule) = self
                .for_pads
                .iter()
                .find(|r| &r.pad == name && r.priority < self.connect_priority)
            {
                return rule.connect;
            }
        }
        self.connect
    }
}

/// The free pad a scope takes, for the one form read exactly: `IsPad And (Name = 'X')`.
fn pad_scope(scope: &str) -> Option<String> {
    let rest = scope.trim().strip_prefix("IsPad And (Name = '")?;
    let name = rest.strip_suffix("')")?;
    // Only a free pad's query name is known; one for a component's pad is not read yet.
    (name.len() > "Free-".len() && name.starts_with("Free-") && !name.contains('\''))
        .then(|| name.to_string())
}

/// Reads the board-wide `PlaneClearance` and `PlaneConnect` rules from `/Rules6/Data`, with the
/// values Altium gave our board for any the file lacks.
pub(crate) fn rules(data: Option<&[u8]>, notes: &mut Vec<Note>) -> Result<Rules, String> {
    let records = match data {
        Some(data) => rule_records(data)?,
        None => Vec::new(),
    };
    let mut narrower: Vec<String> = Vec::new();
    let mut for_pads = Vec::new();
    let mut pick = |kind: &str| -> Option<(i64, &Properties)> {
        let mut best: Option<(i64, &Properties)> = None;
        for rule in &records {
            if rule.get("RULEKIND").map(str::trim) != Some(kind)
                || rule.get("ENABLED").map(str::trim) == Some("FALSE")
            {
                continue;
            }
            let all = |key: &str| rule.get(key).is_none_or(|v| v.trim() == "All");
            let priority = rule
                .get("PRIORITY")
                .and_then(|p| p.trim().parse().ok())
                .unwrap_or(i64::MAX);
            if !(all("SCOPE1EXPRESSION") && all("SCOPE2EXPRESSION")) {
                let pad = rule.get("SCOPE1EXPRESSION").and_then(pad_scope);
                match pad.filter(|_| kind == "PlaneConnect" && all("SCOPE2EXPRESSION")) {
                    Some(pad) => for_pads.push((priority, pad, rule)),
                    None => narrower.push(
                        rule.get("NAME")
                            .map_or_else(|| kind.to_string(), |n| n.trim().to_string()),
                    ),
                }
                continue;
            }
            if best.is_none_or(|(p, _)| priority < p) {
                best = Some((priority, rule));
            }
        }
        best
    };
    let clearance_rule = pick("PlaneClearance").map(|(_, r)| r);
    let connect_pick = pick("PlaneConnect");
    let connect_priority = connect_pick.map_or(i64::MAX, |(p, _)| p);
    let connect_rule = connect_pick.map(|(_, r)| r);
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
    if !narrower.is_empty() {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{} plane rule(s) for some objects only are not read ({}); the board-wide rule \
                 is used for the vias and pads they name",
                narrower.len(),
                narrower.join(", ")
            ),
        });
    }
    let mut pads = Vec::new();
    for (priority, pad, rule) in for_pads {
        pads.push(ObjectRule {
            name: rule.get("NAME").unwrap_or_default().trim().to_string(),
            priority,
            pad,
            connect: connect_of(rule, notes)?,
        });
    }
    pads.sort_by_key(|r| r.priority);
    Ok(Rules {
        clearance,
        connect,
        connect_priority,
        for_pads: pads,
    })
}

/// The connection a `PlaneConnect` rule gives.
fn connect_of(rule: &Properties, notes: &mut Vec<Note>) -> Result<Connect, String> {
    let style = rule.get("PLANECONNECTSTYLE").map(str::trim).unwrap_or("");
    match style {
        "Direct" => return Ok(Connect::Direct),
        "NoConnect" => return Ok(Connect::Isolated),
        _ => {}
    }
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
    let (expansion, air_gap, width) = (0.508, 0.254, 0.254);
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
    cutouts: &[Contour],
    rules: &Rules,
    notes: &mut Vec<Note>,
) -> Result<Planes, String> {
    let mut planes = Planes::default();
    let mut cut = 0usize;
    let mut default_pull_back = 0usize;
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
            let connect = if own {
                rules.connect_for(&drill.of)
            } else {
                Connect::Isolated
            };
            if let Connect::Relief {
                expansion,
                air_gap,
                width,
            } = connect
            {
                let inner = drill.hole / 2.0 + expansion;
                holes.extend(relief(drill.at, inner, inner + air_gap, width));
            } else if connect == Connect::Direct {
                // Solid copper up to the hole, which every overlay has cut anyway.
            } else {
                let radius = drill.hole / 2.0 + rules.clearance;
                if inside || distance(&edge, drill.at) < radius {
                    holes.push(circle(drill.at, radius));
                }
            }
        }
        // The region leaves the pull-back from the board's outer edge out, but not from a board
        // cutout (#123): cut every cutout, grown by the plane's pull-back, out of it.
        if !cutouts.is_empty() {
            let pull_back = layers.pull_back(layer).unwrap_or_else(|| {
                default_pull_back += 1;
                DEFAULT_PULL_BACK
            });
            let region_box = bounds(&edge);
            for cutout in cutouts {
                let pieces = grown(cutout, pull_back);
                let near = pieces
                    .iter()
                    .any(|piece| overlap(bounds(&flatten(piece)), region_box));
                if near {
                    holes.extend(pieces);
                    cut += 1;
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
    if cut > 0 {
        notes.push(Note {
            severity: Severity::Info,
            message: format!(
                "board cutouts cut out of split-plane regions {cut} time(s), each grown by its \
                 plane's pull-back from the board edge"
            ),
        });
    }
    if default_pull_back > 0 {
        notes.push(Note {
            severity: Severity::Info,
            message: format!(
                "{default_pull_back} split-plane region(s) on a plane whose pull-back the stack \
                 does not give; Altium's default of 0.508 is used around cutouts"
            ),
        });
    }
    if !planes.copper.is_empty() {
        notes.push(Note {
            severity: Severity::Info,
            message: format!(
                "{} split-plane region(s) read as plane copper, with clearances and thermal \
                 reliefs from the plane rules",
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

/// A board cutout grown by `by` on every side, as contours whose union is the grown shape: a
/// round cutout (arcs about one center, one radius, a whole turn) as one larger circle; any other
/// as the cutout itself and, along each straight piece of its edge (arcs cut as in [`flatten`]),
/// a capsule reaching `by` to either side, round at both ends.
fn grown(cutout: &Contour, by: f64) -> Vec<Contour> {
    if let Some((center, radius)) = round(cutout) {
        return vec![circle(center, radius + by)];
    }
    let points = flatten(cutout);
    let next = points.iter().skip(1).chain(points.first());
    let mut out = vec![cutout.clone()];
    out.extend(
        points
            .iter()
            .zip(next)
            .filter_map(|(&a, &b)| capsule(a, b, by)),
    );
    out
}

/// The center and radius when every segment of `c` is an arc about one center with one radius
/// (to 1e-9 mm) and the arcs make a whole turn.
fn round(c: &Contour) -> Option<(Point, f64)> {
    let mut first: Option<(Point, f64)> = None;
    let mut turn = 0.0;
    for s in &c.segments {
        let Segment::Arc { center, radius, .. } = *s else {
            return None;
        };
        let same = |(p, r): (Point, f64)| {
            (p.x - center.x).abs() < 1e-9
                && (p.y - center.y).abs() < 1e-9
                && (r - radius).abs() < 1e-9
        };
        match first {
            None => first = Some((center, radius)),
            Some(f) if same(f) => {}
            Some(_) => return None,
        }
        turn += s.sweep_deg().unwrap_or(0.0).abs();
    }
    first.filter(|_| (turn - 360.0).abs() < 1e-6)
}

/// The points within `by` of the segment `a`-`b`: two straight sides and a half circle at each
/// end, counter-clockwise. `None` for a segment of no length.
fn capsule(a: Point, b: Point, by: f64) -> Option<Contour> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx.hypot(dy);
    if length <= 0.0 || by <= 0.0 {
        return None;
    }
    let deg = dy.atan2(dx).to_degrees();
    let (nx, ny) = (-dy / length * by, dx / length * by);
    let at = |p: Point, sx: f64, sy: f64| Point {
        x: p.x + sx,
        y: p.y + sy,
    };
    let half = |center: Point, start_deg: f64| Segment::Arc {
        center,
        radius: by,
        start_deg,
        end_deg: start_deg + 180.0,
        clockwise: false,
    };
    Some(Contour {
        segments: vec![
            Segment::Line {
                from: at(a, -nx, -ny),
                to: at(b, -nx, -ny),
            },
            half(b, deg - 90.0),
            Segment::Line {
                from: at(b, nx, ny),
                to: at(a, nx, ny),
            },
            half(a, deg + 90.0),
        ],
    })
}

/// The smallest and largest x and y of `points`.
fn bounds(points: &[Point]) -> (f64, f64, f64, f64) {
    points.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
    )
}

/// Whether two bounds overlap.
fn overlap(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3
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

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Contour {
        let c = [p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)];
        Contour {
            segments: (0..4)
                .map(|i| Segment::Line {
                    from: c[i],
                    to: c[(i + 1) % 4],
                })
                .collect(),
        }
    }

    #[test]
    fn a_round_cutout_grows_into_one_larger_circle() {
        let pieces = grown(&circle(p(46.5, 5.5), 1.5), 0.508);
        assert_eq!(pieces.len(), 1);
        assert_eq!(round(&pieces[0]), Some((p(46.5, 5.5), 2.008)));
    }

    #[test]
    fn a_square_cutout_grows_by_a_capsule_along_each_edge() {
        let cutout = square(10.0, 10.0, 12.0, 12.0);
        assert_eq!(round(&cutout), None);
        let pieces = grown(&cutout, 0.5);
        // The cutout itself and four capsules.
        assert_eq!(pieces.len(), 5);
        assert_eq!(pieces[0], cutout);
        // Every point of every capsule lies within 0.5 (to 1e-9) of the cutout's edge, and the
        // capsules reach 0.5 out: the grown square spans 9.5 to 12.5.
        let edge = flatten(&cutout);
        let mut reach = (f64::INFINITY, f64::NEG_INFINITY);
        for piece in &pieces[1..] {
            for q in flatten(piece) {
                assert!(distance(&edge, q) <= 0.5 + 1e-9, "{q:?}");
                reach = (reach.0.min(q.x), reach.1.max(q.x));
            }
        }
        assert!(
            (reach.0 - 9.5).abs() < 1e-9 && (reach.1 - 12.5).abs() < 1e-9,
            "{reach:?}"
        );
    }

    #[test]
    fn a_segment_of_no_length_gives_no_capsule() {
        assert_eq!(capsule(p(1.0, 1.0), p(1.0, 1.0), 0.5), None);
        assert_eq!(capsule(p(1.0, 1.0), p(2.0, 1.0), 0.0), None);
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

    /// plane-polygon-connect's rules: Direct for Free-D (priority 2), NoConnect for Free-N (1)
    /// and the board-wide Relief (3); a pad takes the best rule that names it. A scope that is
    /// not read exactly (a net, a component's pad) is named in a note.
    #[test]
    fn connect_rules_for_single_free_pads_win_by_priority() {
        let connect = |priority: u32, scope: &str, style: &str, name: &str| {
            rule(
                0x32,
                &format!(
                    "|RULEKIND=PlaneConnect|NAME={name}|ENABLED=TRUE|PRIORITY={priority}\
                     |SCOPE1EXPRESSION={scope}|SCOPE2EXPRESSION=All|PLANECONNECTSTYLE={style}\
                     |RELIEFEXPANSION=20mil|RELIEFENTRIES=4|RELIEFCONDUCTORWIDTH=10mil\
                     |RELIEFAIRGAP=10mil"
                ),
            )
        };
        let mut data = connect(
            1,
            "IsPad And (Name = 'Free-N')",
            "NoConnect",
            "PlaneConnect_N",
        );
        data.extend(connect(
            2,
            "IsPad And (Name = 'Free-D')",
            "Direct",
            "PlaneConnect_D",
        ));
        data.extend(connect(3, "All", "Relief", "PlaneConnect"));
        data.extend(connect(1, "InNet('GND')", "Direct", "ByNet"));
        data.extend(connect(
            1,
            "IsPad And (Name = 'R1-1')",
            "Direct",
            "ByComponentPad",
        ));
        let mut notes = Vec::new();
        let r = rules(Some(&data), &mut notes).unwrap();
        let pad = |name: &str| r.connect_for(&DrillOf::Pad(name.into()));
        assert_eq!(pad("Free-N"), Connect::Isolated);
        assert_eq!(pad("Free-D"), Connect::Direct);
        assert!(matches!(pad("Free-R"), Connect::Relief { .. }));
        assert!(matches!(
            r.connect_for(&DrillOf::Via),
            Connect::Relief { .. }
        ));
        assert!(matches!(
            r.connect_for(&DrillOf::ComponentPad),
            Connect::Relief { .. }
        ));
        let warnings: Vec<&str> = notes
            .iter()
            .map(|n| n.message.as_str())
            .filter(|m| m.contains("for some objects"))
            .collect();
        assert_eq!(
            warnings,
            [
                "2 plane rule(s) for some objects only are not read (ByNet, ByComponentPad); the \
              board-wide rule is used for the vias and pads they name"
            ]
        );
        assert_eq!(pad_scope("IsPad And (Name = 'Free-')"), None);
        assert_eq!(
            pad_scope("IsPad And (Name = 'Free-1')").as_deref(),
            Some("Free-1")
        );
    }

    /// A rule for one pad with a worse priority than the board-wide rule loses to it.
    #[test]
    fn a_board_wide_rule_of_better_priority_wins() {
        let mut data = rule(
            0x32,
            "|RULEKIND=PlaneConnect|NAME=D|ENABLED=TRUE|PRIORITY=2\
             |SCOPE1EXPRESSION=IsPad And (Name = 'Free-D')|SCOPE2EXPRESSION=All\
             |PLANECONNECTSTYLE=Direct",
        );
        data.extend(rule(
            0x32,
            "|RULEKIND=PlaneConnect|NAME=All|ENABLED=TRUE|PRIORITY=1|SCOPE1EXPRESSION=All\
             |SCOPE2EXPRESSION=All|PLANECONNECTSTYLE=Relief",
        ));
        let mut notes = Vec::new();
        let r = rules(Some(&data), &mut notes).unwrap();
        assert!(matches!(
            r.connect_for(&DrillOf::Pad("Free-D".into())),
            Connect::Relief { .. }
        ));
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
