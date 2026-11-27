//! Standard bodies: realistic exact solids for the common packages, for parts that have no
//! usable 3D model (`read --bodies file|standard`).
//!
//! A package is recognized from its pads first (how many, how they sit, their pitch and the
//! distance between the rows) and the footprint's name second: the name only breaks a tie
//! (`QFN` or `QFP`, which family a two-pad part is) or refuses a part whose pads look like a
//! chip but that is something else (a diode, a tantalum). Nothing is read from a vendor's CAD
//! file: each package below gives, in its doc comment, the public standard and the nominal
//! numbers its solid is drawn from. A body is written as exact solids (`board_step::solids`):
//! boxes, and one prism for each gull-wing lead, so nothing is a mesh.
//!
//! A part in the manifest has one color, so a package gives two models: the body, and the
//! leads or terminations (tin gray; gold for pins), which `package` writes as a second part
//! named "<designator> leads" beside the first.

use std::collections::{BTreeMap, BTreeSet};

use board_model::{Board, Contour, CopperItem, Placement, Point, Segment, Side};
use board_step::{Axis, Solid};

/// Which bodies `read` writes for the parts of a board.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Bodies {
    /// The board's own models; a part with none at all (no embedded model, no linked model
    /// found, no extruded body) gets a standard body when its package is recognized.
    #[default]
    File,
    /// As `File`, and a plain extruded body of a recognized package is replaced by the standard
    /// body too. An embedded or linked STEP model is always kept.
    Standard,
}

/// How a body or its leads look: a color and how rough the surface is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub color: &'static str,
    pub roughness: f64,
}

/// The body of a recognized package, in the part's own frame (z up from the board face).
#[derive(Clone, Debug, PartialEq)]
pub struct Standard {
    /// What it was recognized as, such as "0603 chip resistor" or "SOIC-8".
    pub package: String,
    pub body: Vec<Solid>,
    pub body_look: Look,
    /// Leads, terminations or pins: a second model, since a part has one color.
    pub leads: Vec<Solid>,
    pub leads_look: Look,
}

/// A pad in the part's own frame, as the recognizers see it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StdPad {
    pub x: f64,
    pub y: f64,
    /// Its extent along x and along y.
    pub w: f64,
    pub h: f64,
    /// Has a drill.
    pub through: bool,
}

const TIN: Look = Look {
    color: "#c8c8cc",
    roughness: 0.35,
};
const GOLD: Look = Look {
    color: "#d4af37",
    roughness: 0.45,
};
const MOLD: Look = Look {
    color: "#26262a",
    roughness: 0.6,
};

// ---------------------------------------------------------------------------------------------
// Pads of a part
// ---------------------------------------------------------------------------------------------

/// Finds each part's pads on the board, by the designator they are owned by.
pub struct Recognizer<'a> {
    pads: BTreeMap<&'a str, Vec<&'a CopperItem>>,
    /// Designators that more than one part has: their pads cannot be told apart.
    shared: BTreeSet<&'a str>,
}

impl<'a> Recognizer<'a> {
    pub fn new(board: &'a Board) -> Self {
        let mut pads: BTreeMap<&str, Vec<&CopperItem>> = BTreeMap::new();
        for item in &board.copper {
            let owner = match item {
                CopperItem::Pad { owner, .. } | CopperItem::PolygonPad { owner, .. } => owner,
                _ => continue,
            };
            if let Some(owner) = owner.as_deref() {
                pads.entry(owner).or_default().push(item);
            }
        }
        let mut seen = BTreeSet::new();
        let mut shared = BTreeSet::new();
        for p in &board.components {
            let d = p.designator.as_str();
            if !seen.insert(d) {
                shared.insert(d);
            }
        }
        Self { pads, shared }
    }

    /// The standard body of a part, when its pads are a package we know.
    pub fn recognize(&self, part: &Placement) -> Option<Standard> {
        let designator = part.designator.as_str();
        if designator.trim().is_empty() || self.shared.contains(designator) {
            return None;
        }
        let items = self.pads.get(designator)?;
        let mut pads: Vec<StdPad> = Vec::new();
        for item in items {
            let pad = in_part_frame(item, part);
            // Copper of one pad on several layers is one pad.
            if !pads
                .iter()
                .any(|q| (q.x - pad.x).abs() < 0.01 && (q.y - pad.y).abs() < 0.01)
            {
                pads.push(pad);
            }
        }
        recognize(&part.designator, &part.footprint, &pads)
    }
}

/// A pad's center and extents in the part's frame: the board point less the part's place, turned
/// back by its rotation, and for a bottom-side part (flipped about its X axis) with y negated.
fn in_part_frame(item: &CopperItem, part: &Placement) -> StdPad {
    let (sin, cos) = part.rotation_deg.to_radians().sin_cos();
    let flip = if part.side == Side::Bottom { -1.0 } else { 1.0 };
    let frame = |p: Point| {
        let (dx, dy) = (p.x - part.at.x, p.y - part.at.y);
        (dx * cos + dy * sin, flip * (-dx * sin + dy * cos))
    };
    match item {
        CopperItem::Pad {
            at,
            size_x,
            size_y,
            rotation_deg,
            drill,
            ..
        } => {
            let (x, y) = frame(*at);
            let (s, c) = (rotation_deg - part.rotation_deg).to_radians().sin_cos();
            StdPad {
                x,
                y,
                w: (size_x * c).abs() + (size_y * s).abs(),
                h: (size_x * s).abs() + (size_y * c).abs(),
                through: drill.is_some(),
            }
        }
        CopperItem::PolygonPad {
            at, shapes, drill, ..
        } => {
            let (x, y) = frame(*at);
            let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
            let mut take = |p: Point| {
                let (px, py) = frame(p);
                lo = (lo.0.min(px), lo.1.min(py));
                hi = (hi.0.max(px), hi.1.max(py));
            };
            for contour in shapes {
                for segment in &contour.segments {
                    match segment {
                        Segment::Line { from, to } => {
                            take(*from);
                            take(*to);
                        }
                        Segment::Arc { center, radius, .. } => {
                            take(Point {
                                x: center.x - radius,
                                y: center.y - radius,
                            });
                            take(Point {
                                x: center.x + radius,
                                y: center.y + radius,
                            });
                        }
                    }
                }
            }
            let (w, h) = if hi.0 >= lo.0 && hi.1 >= lo.1 {
                (hi.0 - lo.0, hi.1 - lo.1)
            } else {
                (0.0, 0.0)
            };
            StdPad {
                x,
                y,
                w,
                h,
                through: drill.is_some(),
            }
        }
        _ => StdPad {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
            through: false,
        },
    }
}

// ---------------------------------------------------------------------------------------------
// Solids
// ---------------------------------------------------------------------------------------------

fn pt(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// A closed polygon of straight edges.
fn poly(points: &[(f64, f64)]) -> Contour {
    let mut segments = Vec::new();
    for (i, &(x, y)) in points.iter().enumerate() {
        let (nx, ny) = points
            .get(i.saturating_add(1))
            .or_else(|| points.first())
            .copied()
            .unwrap_or((x, y));
        segments.push(Segment::Line {
            from: pt(x, y),
            to: pt(nx, ny),
        });
    }
    Contour { segments }
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Contour {
    poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
}

/// A box between the corners (x0, y0, z0) and (x1, y1, z1), any order.
fn boxed(x: (f64, f64), y: (f64, f64), z: (f64, f64)) -> Solid {
    Solid {
        outline: rect(x.0.min(x.1), y.0.min(y.1), x.0.max(x.1), y.0.max(y.1)),
        openings: Vec::new(),
        axis: Axis::Z,
        from: z.0.min(z.1),
        to: z.0.max(z.1),
    }
}

/// A box of size `(w, h)` about the center `(cx, cy)`.
fn centered(cx: f64, cy: f64, w: f64, h: f64, z: (f64, f64)) -> Solid {
    boxed(
        (cx - w / 2.0, cx + w / 2.0),
        (cy - h / 2.0, cy + h / 2.0),
        z,
    )
}

/// Which way a lead leaves the body.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Dir {
    XPos,
    XNeg,
    YPos,
    YNeg,
}

/// A gull-wing lead, measured from the body's center outward: the lead leaves the body's side
/// `body` from the center, at height `z_center`, runs out to the riser, drops to the board and
/// ends at `tip` with a flat foot of length `foot`. Thickness `t`, width `w`.
#[derive(Clone, Copy, Debug)]
struct Gull {
    body: f64,
    tip: f64,
    foot: f64,
    t: f64,
    w: f64,
    z_center: f64,
}

/// One gull-wing lead as a single prism: its side profile extruded across the lead's width.
/// `along` is the lead's center along the body's side, `(cx, cy)` the body's center.
fn gull(cx: f64, cy: f64, dir: Dir, along: f64, g: &Gull) -> Solid {
    let riser = (g.tip - g.foot).max(g.body + 0.05);
    let (t, zc) = (g.t, g.z_center);
    // The profile in (distance out from the body's center, height).
    let profile = [
        (g.body, zc + t / 2.0),
        (riser + t, zc + t / 2.0),
        (riser + t, t),
        (g.tip, t),
        (g.tip, 0.0),
        (riser, 0.0),
        (riser, zc - t / 2.0),
        (g.body, zc - t / 2.0),
    ];
    let (axis, sign, base, along_center) = match dir {
        Dir::XPos => (Axis::Y, 1.0, cx, along),
        Dir::XNeg => (Axis::Y, -1.0, cx, along),
        Dir::YPos => (Axis::X, 1.0, cy, along),
        Dir::YNeg => (Axis::X, -1.0, cy, along),
    };
    // Axis::Y sketches in (z, x); Axis::X in (y, z).
    let points: Vec<(f64, f64)> = profile
        .iter()
        .map(|&(d, z)| {
            let outward = base + sign * d;
            match axis {
                Axis::Y => (z, outward),
                _ => (outward, z),
            }
        })
        .collect();
    Solid {
        outline: poly(&points),
        openings: Vec::new(),
        axis,
        from: along_center - g.w / 2.0,
        to: along_center + g.w / 2.0,
    }
}

// ---------------------------------------------------------------------------------------------
// Reading pads
// ---------------------------------------------------------------------------------------------

/// A line of pads sharing one cross coordinate.
#[derive(Clone, Debug)]
struct Row {
    cross: f64,
    /// Positions along the row, ascending.
    along: Vec<f64>,
    /// Mean pad extent across the row (a pad's length) and along it (its width).
    cross_size: f64,
    along_size: f64,
}

/// Groups pads into rows that run along x (`horizontal`) or along y.
fn rows(pads: &[StdPad], horizontal: bool) -> Vec<Row> {
    // (cross, along, cross size, along size)
    let mut v: Vec<(f64, f64, f64, f64)> = pads
        .iter()
        .map(|p| {
            if horizontal {
                (p.y, p.x, p.h, p.w)
            } else {
                (p.x, p.y, p.w, p.h)
            }
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut groups: Vec<Vec<(f64, f64, f64, f64)>> = Vec::new();
    for item in v {
        match groups.last_mut() {
            Some(g) if g.last().is_some_and(|l| (item.0 - l.0).abs() <= 0.3) => g.push(item),
            _ => groups.push(vec![item]),
        }
    }
    groups
        .into_iter()
        .map(|g| {
            let n = g.len() as f64;
            let mean = |f: fn(&(f64, f64, f64, f64)) -> f64| g.iter().map(f).sum::<f64>() / n;
            let mut along: Vec<f64> = g.iter().map(|i| i.1).collect();
            along.sort_by(f64::total_cmp);
            Row {
                cross: mean(|i| i.0),
                along,
                cross_size: mean(|i| i.2),
                along_size: mean(|i| i.3),
            }
        })
        .collect()
}

/// The common spacing of positions, when every gap is the same within 10 percent.
fn pitch(along: &[f64]) -> Option<f64> {
    let gaps: Vec<f64> = along
        .iter()
        .zip(along.iter().skip(1))
        .map(|(a, b)| b - a)
        .collect();
    if gaps.is_empty() {
        return None;
    }
    let mean = gaps.iter().sum::<f64>() / gaps.len() as f64;
    (mean > 0.05 && gaps.iter().all(|g| (g - mean).abs() <= 0.1 * mean + 0.02)).then_some(mean)
}

fn middle(values: &[f64]) -> f64 {
    let lo = values.iter().copied().fold(f64::MAX, f64::min);
    let hi = values.iter().copied().fold(f64::MIN, f64::max);
    (lo + hi) / 2.0
}

fn round_half(v: f64) -> f64 {
    (v * 2.0).round() / 2.0
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let (sum, n) = values.fold((0.0, 0usize), |(s, n), v| (s + v, n.saturating_add(1)));
    if n == 0 {
        0.0
    } else {
        sum / n as f64
    }
}

fn has(name: &str, words: &[&str]) -> bool {
    words.iter().any(|w| name.contains(w))
}

// ---------------------------------------------------------------------------------------------
// Recognition
// ---------------------------------------------------------------------------------------------

/// The standard body for a part with these pads (in its own frame), or `None`. The geometry
/// decides; the footprint name only breaks ties and refuses lookalikes.
pub fn recognize(designator: &str, footprint: &str, pads: &[StdPad]) -> Option<Standard> {
    let name = footprint.to_ascii_lowercase();
    if pads
        .iter()
        .any(|p| !(p.x.is_finite() && p.y.is_finite() && p.w.is_finite() && p.h.is_finite()))
    {
        return None;
    }
    if pads.iter().any(|p| p.through) {
        return header(designator, &name, pads);
    }
    let (perimeter, exposed) = split_exposed(pads);
    match perimeter.len() {
        2 => chip(designator, &name, &perimeter),
        3 | 5 | 6 => sot23(&perimeter).or_else(|| dual(&name, &perimeter, exposed.as_ref())),
        _ => dual(&name, &perimeter, exposed.as_ref())
            .or_else(|| quad(&name, &perimeter, exposed.as_ref())),
    }
}

/// Splits off an exposed pad: one far larger than the rest and in the middle of them.
fn split_exposed(pads: &[StdPad]) -> (Vec<StdPad>, Option<StdPad>) {
    let area = |p: &StdPad| p.w * p.h;
    let biggest = pads
        .iter()
        .enumerate()
        .max_by(|a, b| area(a.1).total_cmp(&area(b.1)));
    if let Some((i, ep)) = biggest {
        let others: Vec<StdPad> = pads
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, p)| *p)
            .collect();
        if others.len() >= 4 {
            let typical = mean(others.iter().map(area));
            let (cx, cy) = (
                middle(&others.iter().map(|p| p.x).collect::<Vec<_>>()),
                middle(&others.iter().map(|p| p.y).collect::<Vec<_>>()),
            );
            if area(ep) > 3.0 * typical && (ep.x - cx).abs() < 0.5 && (ep.y - cy).abs() < 0.5 {
                return (others, Some(*ep));
            }
        }
    }
    (pads.to_vec(), None)
}

// --- Chips -----------------------------------------------------------------------------------

/// What a two-pad chip is, from its designator and footprint name.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Resistor,
    Capacitor,
    Inductor,
    Led,
    Other,
}

fn kind_of(designator: &str, name: &str) -> Kind {
    let d = designator.trim().to_ascii_uppercase();
    if has(name, &["led"]) || d.starts_with("LED") {
        Kind::Led
    } else if d.starts_with("FB") || d.starts_with('L') {
        Kind::Inductor
    } else if d.starts_with('C') {
        Kind::Capacitor
    } else if d.starts_with('R') {
        Kind::Resistor
    } else if has(name, &["cap"]) {
        Kind::Capacitor
    } else if has(name, &["res"]) {
        Kind::Resistor
    } else if has(name, &["ind"]) {
        Kind::Inductor
    } else {
        Kind::Other
    }
}

/// An EIA chip size.
struct ChipSize {
    /// The imperial code in the name, and the metric one.
    imperial: &'static str,
    metric: &'static str,
    /// Nominal length, width, thickness of a resistor and of a ceramic capacitor, and each end
    /// termination's length.
    l: f64,
    w: f64,
    h_resistor: f64,
    h_capacitor: f64,
    t: f64,
    /// Center-to-center distance of the pads this size is recognized from.
    pitch: (f64, f64),
}

/// Chip resistors and capacitors: the EIA/IEC 60115-8 and EIA-198 sizes. Length and width are
/// the standard's nominal numbers (0201 0.6 x 0.3, 0402 1.0 x 0.5, 0603 1.6 x 0.8, 0805
/// 2.0 x 1.25, 1206 3.2 x 1.6 mm); the end terminations are the nominal band widths (0.12, 0.2,
/// 0.3, 0.4, 0.5 mm). The standards give no single thickness: a resistor's is the usual
/// datasheet nominal (0.23, 0.35, 0.45, 0.5, 0.55 mm) and a ceramic capacitor's the common
/// general-purpose one (0.3, 0.5, 0.8, 0.85, 0.85 mm).
const CHIPS: [ChipSize; 5] = [
    ChipSize {
        imperial: "0201",
        metric: "0603m",
        l: 0.6,
        w: 0.3,
        h_resistor: 0.23,
        h_capacitor: 0.3,
        t: 0.12,
        pitch: (0.5, 0.85),
    },
    ChipSize {
        imperial: "0402",
        metric: "1005",
        l: 1.0,
        w: 0.5,
        h_resistor: 0.35,
        h_capacitor: 0.5,
        t: 0.2,
        pitch: (0.85, 1.35),
    },
    ChipSize {
        imperial: "0603",
        metric: "1608",
        l: 1.6,
        w: 0.8,
        h_resistor: 0.45,
        h_capacitor: 0.8,
        t: 0.3,
        pitch: (1.35, 1.85),
    },
    ChipSize {
        imperial: "0805",
        metric: "2012",
        l: 2.0,
        w: 1.25,
        h_resistor: 0.5,
        h_capacitor: 0.85,
        t: 0.4,
        pitch: (1.85, 2.6),
    },
    ChipSize {
        imperial: "1206",
        metric: "3216",
        l: 3.2,
        w: 1.6,
        h_resistor: 0.55,
        h_capacitor: 0.85,
        t: 0.5,
        pitch: (2.6, 3.9),
    },
];

/// Two pads on one line: a chip resistor, capacitor, inductor or LED of an EIA size, a dark or
/// tan body between two tin end terminations. A diode package (SOD, SMA...) or a tantalum, whose
/// pads can look the same, is refused by its name.
fn chip(designator: &str, name: &str, pads: &[StdPad]) -> Option<Standard> {
    let [a, b] = pads else { return None };
    if has(
        name,
        &[
            "sod", "sma", "smb", "smc", "do-", "do2", "tant", "case", "elec", "melf", "sot", "usb",
        ],
    ) {
        return None;
    }
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let horizontal = dx.abs() >= dy.abs();
    let (d, off) = if horizontal {
        (dx.abs(), dy.abs())
    } else {
        (dy.abs(), dx.abs())
    };
    let (len_a, cross_a) = if horizontal { (a.w, a.h) } else { (a.h, a.w) };
    let (len_b, cross_b) = if horizontal { (b.w, b.h) } else { (b.h, b.w) };
    if off > 0.1 * d + 0.05
        || (len_a - len_b).abs() > 0.2 * len_a.max(len_b) + 0.05
        || (cross_a - cross_b).abs() > 0.2 * cross_a.max(cross_b) + 0.05
        || cross_a > 2.4
        || len_a > 2.4
        || len_a >= d
    {
        return None;
    }
    let by_pads = CHIPS.iter().find(|c| d >= c.pitch.0 && d < c.pitch.1)?;
    // A size the name says, when the pads allow it, wins over the dividing line between sizes.
    let size = CHIPS
        .iter()
        .find(|c| {
            (name.contains(c.imperial) || name.contains(c.metric))
                && d >= c.pitch.0 * 0.8
                && d < c.pitch.1 * 1.25
        })
        .unwrap_or(by_pads);
    let kind = kind_of(designator, name);
    let h = match kind {
        Kind::Capacitor => size.h_capacitor,
        _ => size.h_resistor,
    };
    let (cx, cy) = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let (length, width) = (size.l, size.w);
    let inner = length / 2.0 - size.t;
    let z = (0.0, h);
    let (body, end_a, end_b) = if horizontal {
        (
            centered(cx, cy, 2.0 * inner, width, z),
            boxed(
                (cx - length / 2.0, cx - inner),
                (cy - width / 2.0, cy + width / 2.0),
                z,
            ),
            boxed(
                (cx + inner, cx + length / 2.0),
                (cy - width / 2.0, cy + width / 2.0),
                z,
            ),
        )
    } else {
        (
            centered(cx, cy, width, 2.0 * inner, z),
            boxed(
                (cx - width / 2.0, cx + width / 2.0),
                (cy - length / 2.0, cy - inner),
                z,
            ),
            boxed(
                (cx - width / 2.0, cx + width / 2.0),
                (cy + inner, cy + length / 2.0),
                z,
            ),
        )
    };
    let (what, color) = match kind {
        Kind::Resistor => ("resistor", "#1a1a1a"),
        Kind::Capacitor => ("capacitor", "#b08850"),
        Kind::Inductor => ("inductor", "#3e3e42"),
        Kind::Led => ("LED", "#efe8b8"),
        Kind::Other => ("chip", "#2e2e2e"),
    };
    let package = match kind {
        Kind::Other => format!("{} chip", size.imperial),
        _ => format!("{} chip {what}", size.imperial),
    };
    Some(Standard {
        package,
        body: vec![body],
        body_look: Look {
            color,
            roughness: 0.6,
        },
        leads: vec![end_a, end_b],
        leads_look: TIN,
    })
}

// --- SOT-23 ----------------------------------------------------------------------------------

/// SOT-23-3, -5 and -6: two rows of pads 0.95 mm apart along the row, the three positions
/// -0.95, 0 and +0.95 (3 pads: two in one row and one opposite; 5: three and two; 6: three
/// and three).
///
/// SOT-23-3 follows JEDEC TO-236 (variant AB): body 2.92 x 1.30 mm, 1.0 mm above the board with
/// the body bottom 0.05 mm up (A 1.0 nominal, A1 0.05), leads 0.40 mm wide, 0.15 thick, 0.45
/// long on the board, tip to tip 2.4 mm. SOT-23-5 and -6 follow JEDEC MO-178: body
/// 2.90 x 1.60 mm, 1.1 mm high with the body bottom 0.05 mm up, same leads, tip to tip 2.8 mm.
fn sot23(pads: &[StdPad]) -> Option<Standard> {
    let n = pads.len();
    for horizontal in [true, false] {
        let r = rows(pads, horizontal);
        let [a, b] = r.as_slice() else { continue };
        let mut counts = [a.along.len(), b.along.len()];
        counts.sort_unstable();
        if !matches!((n, counts), (3, [1, 2]) | (5, [2, 3]) | (6, [3, 3])) {
            continue;
        }
        // The three positions along the rows: -p, 0 and p.
        let mut all: Vec<f64> = a.along.iter().chain(&b.along).copied().collect();
        all.sort_by(f64::total_cmp);
        let mut distinct: Vec<f64> = Vec::new();
        for v in all {
            if distinct.last().is_none_or(|l| v - l > 0.2) {
                distinct.push(v);
            }
        }
        let Some(p) = (distinct.len() == 3).then(|| pitch(&distinct)).flatten() else {
            continue;
        };
        let sep = (b.cross - a.cross).abs();
        if !(0.85..=1.05).contains(&p) || !(1.9..=3.4).contains(&sep) {
            continue;
        }
        let along_center = middle(&distinct);
        let cross_center = (a.cross + b.cross) / 2.0;
        let (cx, cy) = if horizontal {
            (along_center, cross_center)
        } else {
            (cross_center, along_center)
        };
        let (length, width, top, tip, name) = if n == 3 {
            (2.92, 1.30, 1.0, 1.2, "SOT-23-3")
        } else if n == 5 {
            (2.90, 1.60, 1.1, 1.4, "SOT-23-5")
        } else {
            (2.90, 1.60, 1.1, 1.4, "SOT-23-6")
        };
        let z = (0.05, top);
        let body = if horizontal {
            centered(cx, cy, length, width, z)
        } else {
            centered(cx, cy, width, length, z)
        };
        let g = Gull {
            body: width / 2.0,
            tip,
            foot: 0.45,
            t: 0.15,
            w: 0.40,
            z_center: (z.0 + z.1) / 2.0,
        };
        let mut leads = Vec::new();
        for row in [a, b] {
            let outward = row.cross > cross_center;
            let dir = match (horizontal, outward) {
                (true, true) => Dir::YPos,
                (true, false) => Dir::YNeg,
                (false, true) => Dir::XPos,
                (false, false) => Dir::XNeg,
            };
            for &along in &row.along {
                leads.push(gull(cx, cy, dir, along, &g));
            }
        }
        return Some(Standard {
            package: name.to_string(),
            body: vec![body],
            body_look: MOLD,
            leads,
            leads_look: TIN,
        });
    }
    None
}

// --- Two rows: SOIC, TSSOP, DFN --------------------------------------------------------------

/// Two mirrored rows of pads at one pitch: SOIC (1.27 mm), TSSOP (0.65 mm) or a DFN.
///
/// SOIC-8, -14 and -16, JEDEC MS-012 (150 mil body): body 3.90 mm wide and 4.90, 8.65 and
/// 9.90 mm long, body 0.175 to 1.55 mm above the board (A 1.55, A1 0.175 nominal), leads
/// 0.41 mm wide, 0.21 thick, 0.835 long on the board, tip to tip 6.00 mm.
///
/// TSSOP-8 to -28, JEDEC MO-153: body 4.40 mm wide and 3.0 (8), 5.0 (14, 16), 6.5 (20),
/// 7.8 (24), 9.7 (28) mm long, body 0.1 to 1.1 mm above the board, leads 0.245 wide, 0.15
/// thick, 0.60 long, tip to tip 6.40 mm.
///
/// DFN, JEDEC MO-229 (WDFN): a body 0.75 mm thick, with a 0.4 mm long, 0.05 mm proud terminal
/// at each pad, flush with the body edge; the terminal width is half the pitch (0.18 to 0.35
/// mm). The body size is not fixed by the standard: it is the pad rows' distance and length
/// rounded to the 0.5 mm steps the standard's sizes come in.
fn dual(name: &str, pads: &[StdPad], exposed: Option<&StdPad>) -> Option<Standard> {
    for horizontal in [true, false] {
        let r = rows(pads, horizontal);
        let [a, b] = r.as_slice() else { continue };
        let per_row = a.along.len();
        if per_row < 2
            || per_row != b.along.len()
            || !a
                .along
                .iter()
                .zip(&b.along)
                .all(|(x, y)| (x - y).abs() < 0.15)
        {
            continue;
        }
        let Some(p) = pitch(&a.along) else { continue };
        let sep = (b.cross - a.cross).abs();
        let n = pads.len();
        let along_center = middle(&a.along);
        let cross_center = (a.cross + b.cross) / 2.0;
        let (cx, cy) = if horizontal {
            (along_center, cross_center)
        } else {
            (cross_center, along_center)
        };
        let soic = (1.2..=1.33).contains(&p) && (4.6..=6.6).contains(&sep);
        let tssop = (0.6..=0.7).contains(&p) && (5.0..=6.6).contains(&sep);
        let gull_spec = if soic {
            let length = match n {
                8 => 4.90,
                14 => 8.65,
                16 => 9.90,
                _ => continue,
            };
            Some((
                format!("SOIC-{n}"),
                length,
                3.90,
                (0.175, 1.55),
                Gull {
                    body: 1.95,
                    tip: 3.0,
                    foot: 0.835,
                    t: 0.21,
                    w: 0.41,
                    z_center: 0.86,
                },
            ))
        } else if tssop {
            let length = match n {
                8 => 3.0,
                14 | 16 => 5.0,
                20 => 6.5,
                24 => 7.8,
                28 => 9.7,
                _ => continue,
            };
            Some((
                format!("TSSOP-{n}"),
                length,
                4.40,
                (0.1, 1.1),
                Gull {
                    body: 2.2,
                    tip: 3.2,
                    foot: 0.6,
                    t: 0.15,
                    w: 0.245,
                    z_center: 0.6,
                },
            ))
        } else {
            None
        };
        if let Some((package, length, width, z, g)) = gull_spec {
            let body = if horizontal {
                centered(cx, cy, length, width, z)
            } else {
                centered(cx, cy, width, length, z)
            };
            let mut leads = Vec::new();
            for row in [a, b] {
                let dir = match (horizontal, row.cross > cross_center) {
                    (true, true) => Dir::YPos,
                    (true, false) => Dir::YNeg,
                    (false, true) => Dir::XPos,
                    (false, false) => Dir::XNeg,
                };
                for &along in &row.along {
                    leads.push(gull(cx, cy, dir, along, &g));
                }
            }
            return Some(Standard {
                package,
                body: vec![body],
                body_look: MOLD,
                leads,
                leads_look: TIN,
            });
        }
        // A DFN: short pads, fine pitch, rows close together; or a name that says so.
        let named = has(name, &["dfn", "son", "mlf"]);
        let flat = a.cross_size < 1.2 && b.cross_size < 1.2;
        if !((0.35..=0.85).contains(&p) && sep <= 4.6 && (flat || named)) {
            continue;
        }
        if has(name, &["qfp", "sop", "sot"]) {
            continue;
        }
        let across = round_half(sep).max(1.0);
        let extent = (per_row as f64 - 1.0) * p + a.along_size;
        let along_len = ((extent + 0.9) * 2.0).floor() / 2.0;
        let along_len = along_len.max(1.0);
        let (bx, by) = if horizontal {
            (along_len, across)
        } else {
            (across, along_len)
        };
        let mut leads = Vec::new();
        let term = (0.5 * p).clamp(0.18, 0.35);
        for row in [a, b] {
            let out = if row.cross > cross_center { 1.0 } else { -1.0 };
            for &along in &row.along {
                let reach = out * across / 2.0;
                let inward = reach - out * 0.4;
                let (x, y) = if horizontal {
                    (
                        (along - term / 2.0, along + term / 2.0),
                        (cy + inward, cy + reach),
                    )
                } else {
                    (
                        (cx + inward, cx + reach),
                        (along - term / 2.0, along + term / 2.0),
                    )
                };
                leads.push(boxed(x, y, (0.0, 0.05)));
            }
        }
        leads.extend(exposed_pad(exposed, bx, by));
        return Some(Standard {
            package: format!("DFN-{n} ({bx:.1} x {by:.1} mm)"),
            body: vec![centered(cx, cy, bx, by, (0.05, 0.75))],
            body_look: MOLD,
            leads,
            leads_look: TIN,
        });
    }
    None
}

/// The exposed pad's terminal: the pad less 0.2 mm each way, as thick as the other terminals.
fn exposed_pad(exposed: Option<&StdPad>, body_x: f64, body_y: f64) -> Option<Solid> {
    let ep = exposed?;
    let (w, h) = (
        (ep.w - 0.2).max(0.5).min(body_x),
        (ep.h - 0.2).max(0.5).min(body_y),
    );
    Some(centered(ep.x, ep.y, w, h, (0.0, 0.05)))
}

// --- Four sides: QFP, QFN --------------------------------------------------------------------

/// A TQFP or LQFP in the sizes people use.
struct Qfp {
    pins: usize,
    pitch: f64,
    /// Body and tip-to-tip size, a square.
    body: f64,
    span: f64,
    /// Lead width.
    w: f64,
}

/// TQFP and LQFP, JEDEC MS-026: a square body, leads on four sides with a 0.6 mm foot, 0.15 mm
/// thick (c 0.09 to 0.20), the body bottom 0.1 mm above the board (A1 0.05 to 0.15). Sizes:
/// 32 pins at 0.8 mm 7 x 7 mm body, 9 x 9 mm over the leads; 44 at 0.8 mm 10 x 10 and 12 x 12;
/// 48 at 0.5 mm 7 x 7 and 9 x 9; 64 at 0.5 mm 10 x 10 and 12 x 12; 64 at 0.8 mm 14 x 14 and
/// 16 x 16; 100 at 0.5 mm 14 x 14 and 16 x 16. The lead width is 0.37 mm at 0.8 mm pitch and
/// 0.22 mm at 0.5 mm. A thin body (TQFP) is 1.0 mm thick; a low-profile one (LQFP, and the
/// 14 x 14 mm bodies) 1.4 mm.
const QFPS: [Qfp; 6] = [
    Qfp {
        pins: 32,
        pitch: 0.8,
        body: 7.0,
        span: 9.0,
        w: 0.37,
    },
    Qfp {
        pins: 44,
        pitch: 0.8,
        body: 10.0,
        span: 12.0,
        w: 0.37,
    },
    Qfp {
        pins: 48,
        pitch: 0.5,
        body: 7.0,
        span: 9.0,
        w: 0.22,
    },
    Qfp {
        pins: 64,
        pitch: 0.5,
        body: 10.0,
        span: 12.0,
        w: 0.22,
    },
    Qfp {
        pins: 64,
        pitch: 0.8,
        body: 14.0,
        span: 16.0,
        w: 0.37,
    },
    Qfp {
        pins: 100,
        pitch: 0.5,
        body: 14.0,
        span: 16.0,
        w: 0.22,
    },
];

/// Pads on four sides: a QFP or a QFN, told apart by an exposed pad, the pads' length and the
/// name. QFN, JEDEC MO-220 (VQFN): a body 0.85 mm thick, 0.05 mm above the board, with 0.4 mm
/// long terminals 0.05 mm proud of it flush with each edge, as wide as half the pitch (0.18 to
/// 0.35 mm), and the exposed pad 0.2 mm smaller than its land. The body is as large as the ring
/// of pads (their centers sit on its edge) rounded to the standard's 0.5 mm steps.
fn quad(name: &str, pads: &[StdPad], exposed: Option<&StdPad>) -> Option<Standard> {
    let n = pads.len();
    let (cx, cy) = (
        middle(&pads.iter().map(|p| p.x).collect::<Vec<_>>()),
        middle(&pads.iter().map(|p| p.y).collect::<Vec<_>>()),
    );
    let side = |keep: fn(f64, f64) -> bool| -> Vec<StdPad> {
        pads.iter()
            .filter(|p| keep(p.x - cx, p.y - cy))
            .copied()
            .collect()
    };
    let left = side(|dx, dy| dx < 0.0 && dx.abs() > dy.abs());
    let right = side(|dx, dy| dx >= 0.0 && dx.abs() > dy.abs());
    let bottom = side(|dx, dy| dy < 0.0 && dy.abs() >= dx.abs());
    let top = side(|dx, dy| dy >= 0.0 && dy.abs() >= dx.abs());
    let (a, b) = (left.len(), bottom.len());
    if a < 2 || b < 2 || right.len() != a || top.len() != b {
        return None;
    }
    let along_of = |list: &[StdPad], x: bool| -> Vec<f64> {
        let mut v: Vec<f64> = list.iter().map(|p| if x { p.x } else { p.y }).collect();
        v.sort_by(f64::total_cmp);
        v
    };
    let (la, ra) = (along_of(&left, false), along_of(&right, false));
    let (ba, ta) = (along_of(&bottom, true), along_of(&top, true));
    let pitches = [pitch(&la)?, pitch(&ra)?, pitch(&ba)?, pitch(&ta)?];
    let p = mean(pitches.iter().copied());
    if pitches.iter().any(|q| (q - p).abs() > 0.05 * p) {
        return None;
    }
    let rx = mean(left.iter().chain(&right).map(|q| (q.x - cx).abs()));
    let ry = mean(bottom.iter().chain(&top).map(|q| (q.y - cy).abs()));
    // How far the pads reach out from their rows: a QFP's land is long, a QFN's short.
    let length = mean(
        left.iter()
            .chain(&right)
            .map(|q| q.w)
            .chain(bottom.iter().chain(&top).map(|q| q.h)),
    );
    let wants_qfn = has(name, &["qfn", "dfn", "mlf", "vqfn", "wqfn", "lga"]);
    let wants_qfp = has(name, &["qfp"]);
    let as_qfp = wants_qfp || (!wants_qfn && exposed.is_none() && length >= 1.0);
    if as_qfp {
        let q = QFPS.iter().find(|q| {
            q.pins == n
                && (q.pitch - p).abs() < 0.1
                && (rx - q.span / 2.0).abs().max((ry - q.span / 2.0).abs()) < 1.0
                && rx >= q.span / 2.0 - 1.0
                && ry >= q.span / 2.0 - 1.0
        })?;
        let thick = if has(name, &["lqfp"]) || q.body >= 14.0 {
            1.4
        } else {
            1.0
        };
        let z = (0.1, 0.1 + thick);
        let g = Gull {
            body: q.body / 2.0,
            tip: q.span / 2.0,
            foot: 0.6,
            t: 0.15,
            w: q.w,
            z_center: 0.1 + thick / 2.0,
        };
        let mut leads = Vec::new();
        for (list, dir) in [
            (&la, Dir::XNeg),
            (&ra, Dir::XPos),
            (&ba, Dir::YNeg),
            (&ta, Dir::YPos),
        ] {
            for &along in list {
                leads.push(gull(cx, cy, dir, along, &g));
            }
        }
        let family = if thick > 1.2 { "LQFP" } else { "TQFP" };
        return Some(Standard {
            package: format!("{family}-{n} ({:.0} x {:.0} mm)", q.body, q.body),
            body: vec![centered(cx, cy, q.body, q.body, z)],
            body_look: MOLD,
            leads,
            leads_look: TIN,
        });
    }
    if !(0.3..=1.0).contains(&p) || n < 8 {
        return None;
    }
    let (bx, by) = (round_half(2.0 * rx).max(1.0), round_half(2.0 * ry).max(1.0));
    let term = (0.5 * p).clamp(0.18, 0.35);
    let mut leads = Vec::new();
    for (list, dir) in [
        (&la, Dir::XNeg),
        (&ra, Dir::XPos),
        (&ba, Dir::YNeg),
        (&ta, Dir::YPos),
    ] {
        for &along in list {
            let (x, y) = match dir {
                Dir::XPos => (
                    (cx + bx / 2.0 - 0.4, cx + bx / 2.0),
                    (along - term / 2.0, along + term / 2.0),
                ),
                Dir::XNeg => (
                    (cx - bx / 2.0, cx - bx / 2.0 + 0.4),
                    (along - term / 2.0, along + term / 2.0),
                ),
                Dir::YPos => (
                    (along - term / 2.0, along + term / 2.0),
                    (cy + by / 2.0 - 0.4, cy + by / 2.0),
                ),
                Dir::YNeg => (
                    (along - term / 2.0, along + term / 2.0),
                    (cy - by / 2.0, cy - by / 2.0 + 0.4),
                ),
            };
            leads.push(boxed(x, y, (0.0, 0.05)));
        }
    }
    leads.extend(exposed_pad(exposed, bx, by));
    Some(Standard {
        package: format!("QFN-{n} ({bx:.1} x {by:.1} mm)"),
        body: vec![centered(cx, cy, bx, by, (0.05, 0.85))],
        body_look: MOLD,
        leads,
        leads_look: TIN,
    })
}

// --- Pin headers -----------------------------------------------------------------------------

/// A 2.54 mm (0.100 inch) pin header, one or two rows. No standard fixes the numbers; these are
/// the common 0.100 inch header's: a black plastic strip 2.54 mm to a pin and 2.5 mm high, and
/// 0.64 mm (0.025 inch) square gold pins from 3.0 mm below the board to 6.0 mm above the
/// plastic. Recognized from through-hole pads on a 2.54 mm grid in one or two rows, in a part
/// whose designator (J, P, H, X, CN, CON, JP) or footprint name (header, pin, conn, socket)
/// says it is a connector.
fn header(designator: &str, name: &str, pads: &[StdPad]) -> Option<Standard> {
    let d = designator.trim().to_ascii_uppercase();
    let by_designator = ["J", "P", "H", "X", "CN", "CON", "JP"]
        .iter()
        .any(|p| d.starts_with(p));
    let by_name = has(name, &["header", "hdr", "pin", "conn", "socket"]);
    if !(by_designator || by_name) || has(name, &["dip", "led", "cap", "res", "xtal"]) {
        return None;
    }
    if pads.len() < 2 || !pads.iter().all(|p| p.through) {
        return None;
    }
    const PITCH: f64 = 2.54;
    for horizontal in [true, false] {
        let r = rows(pads, horizontal);
        let ok = match r.as_slice() {
            [a] => a.along.len() >= 2,
            [a, b] => {
                a.along.len() == b.along.len()
                    && (((b.cross - a.cross).abs() - PITCH).abs() < 0.06)
                    && a.along
                        .iter()
                        .zip(&b.along)
                        .all(|(x, y)| (x - y).abs() < 0.1)
            }
            _ => false,
        };
        if !ok {
            continue;
        }
        let spaced = r.iter().all(|row| {
            row.along.len() < 2 || pitch(&row.along).is_some_and(|p| (p - PITCH).abs() < 0.06)
        });
        if !spaced {
            continue;
        }
        let (Some(first), Some(count)) = (r.first(), r.first().map(|row| row.along.len())) else {
            continue;
        };
        if count > 40 {
            continue;
        }
        let along_lo = first.along.first().copied()? - PITCH / 2.0;
        let along_hi = first.along.last().copied()? + PITCH / 2.0;
        let cross_values: Vec<f64> = r.iter().map(|row| row.cross).collect();
        let cross_lo = cross_values.iter().copied().fold(f64::MAX, f64::min) - PITCH / 2.0;
        let cross_hi = cross_values.iter().copied().fold(f64::MIN, f64::max) + PITCH / 2.0;
        let square = |c: (f64, f64)| -> Contour {
            let (x, y) = if horizontal { (c.0, c.1) } else { (c.1, c.0) };
            rect(x - 0.32, y - 0.32, x + 0.32, y + 0.32)
        };
        let mut openings = Vec::new();
        let mut pins = Vec::new();
        for row in &r {
            for &along in &row.along {
                openings.push(square((along, row.cross)));
                let (x, y) = if horizontal {
                    (along, row.cross)
                } else {
                    (row.cross, along)
                };
                pins.push(centered(x, y, 0.64, 0.64, (-3.0, 8.5)));
            }
        }
        let outline = if horizontal {
            rect(along_lo, cross_lo, along_hi, cross_hi)
        } else {
            rect(cross_lo, along_lo, cross_hi, along_hi)
        };
        let rows_n = r.len();
        return Some(Standard {
            package: format!("{rows_n}x{count} pin header, 2.54 mm"),
            body: vec![Solid {
                outline,
                openings,
                axis: Axis::Z,
                from: 0.0,
                to: 2.5,
            }],
            body_look: Look {
                color: "#1a1a1a",
                roughness: 0.6,
            },
            leads: pins,
            leads_look: GOLD,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pad of a size, centered, SMD.
    fn smd(x: f64, y: f64, w: f64, h: f64) -> StdPad {
        StdPad {
            x,
            y,
            w,
            h,
            through: false,
        }
    }

    fn thru(x: f64, y: f64) -> StdPad {
        StdPad {
            x,
            y,
            w: 1.7,
            h: 1.7,
            through: true,
        }
    }

    /// The topology of a STEP file's solids from its text alone: every edge used once in each
    /// direction, every loop closed head to tail, V - E + F = 2 for each closed shell with the
    /// number of openings counted in (a face with a hole adds one to the genus-free count).
    /// Returns the number of solids.
    fn closed_solids(text: &str) -> usize {
        use std::collections::HashMap;
        let mut all: HashMap<usize, (String, String)> = HashMap::new();
        for line in text.lines() {
            let Some(rest) = line.strip_prefix('#') else {
                continue;
            };
            let Some((id, body)) = rest.split_once('=') else {
                continue;
            };
            let Some((name, args)) = body.trim_end_matches(';').split_once('(') else {
                continue;
            };
            if let Ok(id) = id.parse() {
                all.insert(id, (name.to_string(), args.to_string()));
            }
        }
        let refs = |args: &str| -> Vec<usize> {
            args.split('#')
                .skip(1)
                .filter_map(|s| {
                    s.split(|c: char| !c.is_ascii_digit())
                        .next()
                        .and_then(|d| d.parse().ok())
                })
                .collect()
        };
        let of = |name: &'static str| all.iter().filter(move |(_, (n, _))| n == name);
        let edges: HashMap<usize, (usize, usize)> = of("EDGE_CURVE")
            .map(|(id, (_, args))| {
                let r = refs(args);
                (*id, (r[0], r[1]))
            })
            .collect();
        let oriented: HashMap<usize, (usize, bool)> = of("ORIENTED_EDGE")
            .map(|(id, (_, args))| (*id, (refs(args)[0], args.contains(".T."))))
            .collect();
        let mut uses: HashMap<usize, (usize, usize)> = HashMap::new();
        for (edge, forward) in oriented.values() {
            let u = uses.entry(*edge).or_default();
            if *forward {
                u.0 += 1;
            } else {
                u.1 += 1;
            }
        }
        assert_eq!(uses.len(), edges.len(), "every edge is used");
        for (edge, u) in &uses {
            assert_eq!(*u, (1, 1), "edge #{edge} must be used once each way");
        }
        for (id, (_, args)) in of("EDGE_LOOP") {
            let chain: Vec<(usize, usize)> = refs(args)
                .iter()
                .map(|o| {
                    let (edge, forward) = oriented[o];
                    let (a, b) = edges[&edge];
                    if forward {
                        (a, b)
                    } else {
                        (b, a)
                    }
                })
                .collect();
            for (k, step) in chain.iter().enumerate() {
                assert_eq!(step.1, chain[(k + 1) % chain.len()].0, "loop #{id}");
            }
        }
        // Every face's bounds are loops; the shell closes when every edge is used twice (above).
        let shells = of("CLOSED_SHELL").count();
        let solids = of("MANIFOLD_SOLID_BREP").count();
        assert_eq!(shells, solids);
        solids
    }

    /// The extent of every point in a STEP file: (min, max) per axis, over the vertices only.
    fn extent(text: &str) -> ([f64; 3], [f64; 3]) {
        let mut vertex_points = std::collections::HashSet::new();
        let mut points = std::collections::HashMap::new();
        for line in text.lines() {
            let Some(rest) = line.strip_prefix('#') else {
                continue;
            };
            let Some((id, body)) = rest.split_once('=') else {
                continue;
            };
            if body.starts_with("CARTESIAN_POINT") {
                let nums: Vec<f64> = body
                    .split(['(', ')', ','])
                    .filter_map(|s| s.parse().ok())
                    .collect();
                points.insert(id.to_string(), nums);
            } else if body.starts_with("VERTEX_POINT") {
                if let Some(p) = body.split('#').nth(1) {
                    vertex_points.insert(p.trim_end_matches([')', ';']).to_string());
                }
            }
        }
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for id in vertex_points {
            for (k, v) in points[&id].iter().enumerate() {
                lo[k] = lo[k].min(*v);
                hi[k] = hi[k].max(*v);
            }
        }
        (lo, hi)
    }

    fn written(solids: &[Solid]) -> String {
        board_step::solids(solids, "test", "Standard body")
            .unwrap()
            .text
    }

    /// Both models of a standard body are closed solids, as many as the list holds.
    fn assert_closed(s: &Standard) {
        assert_eq!(closed_solids(&written(&s.body)), s.body.len());
        assert_eq!(closed_solids(&written(&s.leads)), s.leads.len());
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn chips_are_recognized_by_their_pad_distance_in_either_direction() {
        // (code, pad centers distance, pad size along, across)
        for (code, d, la, lc) in [
            ("0201", 0.66, 0.35, 0.35),
            ("0402", 1.0, 0.5, 0.6),
            ("0603", 1.6, 0.9, 1.0),
            ("0805", 2.1, 1.2, 1.4),
            ("1206", 3.2, 1.2, 1.8),
        ] {
            for horizontal in [true, false] {
                let pads = if horizontal {
                    vec![smd(-d / 2.0, 0.0, la, lc), smd(d / 2.0, 0.0, la, lc)]
                } else {
                    vec![smd(0.0, -d / 2.0, lc, la), smd(0.0, d / 2.0, lc, la)]
                };
                let s = recognize("R1", "", &pads).unwrap();
                assert_eq!(s.package, format!("{code} chip resistor"), "{code}");
                assert_eq!((s.body.len(), s.leads.len()), (1, 2));
                assert_closed(&s);
                let (lo, hi) = extent(&written(&s.leads));
                let long = if horizontal { 0 } else { 1 };
                let size = CHIPS.iter().find(|c| c.imperial == code).unwrap();
                assert!(near(hi[long] - lo[long], size.l), "{code} {lo:?} {hi:?}");
                assert!(near(hi[2] - lo[2], size.h_resistor));
            }
        }
    }

    #[test]
    fn a_chip_is_a_resistor_a_capacitor_an_inductor_or_an_led_by_its_designator_and_name() {
        let pads = [smd(-0.8, 0.0, 0.9, 1.0), smd(0.8, 0.0, 0.9, 1.0)];
        let colors: Vec<(String, &str)> = [("R1", ""), ("C1", ""), ("L1", ""), ("D1", "LED_0603")]
            .iter()
            .map(|(d, n)| {
                let s = recognize(d, n, &pads).unwrap();
                (s.package, s.body_look.color)
            })
            .collect();
        assert_eq!(colors[0], ("0603 chip resistor".into(), "#1a1a1a"));
        assert_eq!(colors[1], ("0603 chip capacitor".into(), "#b08850"));
        assert_eq!(colors[2], ("0603 chip inductor".into(), "#3e3e42"));
        assert_eq!(colors[3], ("0603 chip LED".into(), "#efe8b8"));
        // A capacitor is thicker than a resistor of its size.
        let c = recognize("C1", "", &pads).unwrap();
        let r = recognize("R1", "", &pads).unwrap();
        assert!(extent(&written(&c.body)).1[2] > extent(&written(&r.body)).1[2]);
    }

    #[test]
    fn the_footprint_name_picks_the_size_near_a_dividing_line_and_refuses_lookalikes() {
        // 1.86 mm apart is 0805 by the pads; the name says 0603, and the pads allow it.
        let pads = [smd(-0.93, 0.0, 0.9, 1.0), smd(0.93, 0.0, 0.9, 1.0)];
        assert_eq!(
            recognize("R1", "", &pads).unwrap().package,
            "0805 chip resistor"
        );
        assert_eq!(
            recognize("R1", "R0603", &pads).unwrap().package,
            "0603 chip resistor"
        );
        // 3.3 mm apart is 1206, but a SOD-123 diode has pads there too.
        let diode = [smd(-1.65, 0.0, 1.0, 1.2), smd(1.65, 0.0, 1.0, 1.2)];
        assert_eq!(recognize("D1", "", &diode).unwrap().package, "1206 chip");
        assert!(recognize("D1", "SOD-123", &diode).is_none());
        // Two pads that are not a chip: too far apart, or not in line, or not alike.
        assert!(recognize(
            "R1",
            "",
            &[smd(-3.0, 0.0, 1.0, 1.0), smd(3.0, 0.0, 1.0, 1.0)]
        )
        .is_none());
        assert!(recognize(
            "R1",
            "",
            &[smd(-0.8, 0.0, 0.9, 1.0), smd(0.8, 1.0, 0.9, 1.0)]
        )
        .is_none());
        assert!(recognize(
            "R1",
            "",
            &[smd(-0.8, 0.0, 0.9, 1.0), smd(0.8, 0.0, 0.3, 0.3)]
        )
        .is_none());
    }

    fn sot_pads(n: usize, sep: f64) -> Vec<StdPad> {
        let top = |x: f64| smd(x, sep / 2.0, 0.6, 1.0);
        let bottom = |x: f64| smd(x, -sep / 2.0, 0.6, 1.0);
        match n {
            3 => vec![bottom(-0.95), bottom(0.95), top(0.0)],
            5 => vec![
                bottom(-0.95),
                bottom(0.0),
                bottom(0.95),
                top(-0.95),
                top(0.95),
            ],
            _ => vec![
                bottom(-0.95),
                bottom(0.0),
                bottom(0.95),
                top(-0.95),
                top(0.0),
                top(0.95),
            ],
        }
    }

    #[test]
    fn sot23_with_three_five_and_six_pads() {
        for (n, sep, leads) in [(3, 2.3, 3), (5, 2.6, 5), (6, 2.6, 6)] {
            let s = recognize("Q1", "", &sot_pads(n, sep)).unwrap();
            assert_eq!(s.package, format!("SOT-23-{n}"));
            assert_eq!((s.body.len(), s.leads.len()), (1, leads));
            assert_closed(&s);
            // The body is 2.9 mm long along the rows and the leads reach their tip span across.
            let (lo, hi) = extent(&written(&s.body));
            assert!((hi[0] - lo[0] - if n == 3 { 2.92 } else { 2.90 }).abs() < 1e-6);
            let (lo, hi) = extent(&written(&s.leads));
            assert!(
                near(hi[1] - lo[1], if n == 3 { 2.4 } else { 2.8 }),
                "{lo:?} {hi:?}"
            );
            assert!(near(lo[2], 0.0), "the feet stand on the board");
        }
        // The same part turned a quarter turn.
        let turned: Vec<StdPad> = sot_pads(5, 2.6)
            .iter()
            .map(|p| smd(-p.y, p.x, p.h, p.w))
            .collect();
        let s = recognize("Q1", "", &turned).unwrap();
        assert_eq!(s.package, "SOT-23-5");
        let (lo, hi) = extent(&written(&s.leads));
        assert!(near(hi[0] - lo[0], 2.8), "{lo:?} {hi:?}");
        assert_closed(&s);
        // Pitch 0.65 is not a SOT-23.
        let sc70 = [
            smd(-0.65, 1.0, 0.4, 0.6),
            smd(0.65, 1.0, 0.4, 0.6),
            smd(0.0, -1.0, 0.4, 0.6),
        ];
        assert!(recognize("Q1", "", &sc70).is_none());
    }

    fn dual_pads(n: usize, pitch: f64, sep: f64, len: f64, width: f64) -> Vec<StdPad> {
        let per = n / 2;
        let x0 = -(per as f64 - 1.0) * pitch / 2.0;
        let mut v = Vec::new();
        for i in 0..per {
            let x = x0 + i as f64 * pitch;
            v.push(smd(x, sep / 2.0, width, len));
            v.push(smd(x, -sep / 2.0, width, len));
        }
        v
    }

    #[test]
    fn soic_and_tssop_with_their_pin_counts() {
        for (n, name) in [(8, "SOIC-8"), (14, "SOIC-14"), (16, "SOIC-16")] {
            let s = recognize("U1", "", &dual_pads(n, 1.27, 5.3, 1.5, 0.6)).unwrap();
            assert_eq!(s.package, name);
            assert_eq!(s.leads.len(), n);
            assert_closed(&s);
            let (lo, hi) = extent(&written(&s.leads));
            assert!(near(hi[1] - lo[1], 6.0), "{lo:?} {hi:?}");
        }
        for (n, length) in [
            (8, 3.0),
            (14, 5.0),
            (16, 5.0),
            (20, 6.5),
            (24, 7.8),
            (28, 9.7),
        ] {
            let s = recognize("U1", "", &dual_pads(n, 0.65, 5.7, 1.5, 0.4)).unwrap();
            assert_eq!(s.package, format!("TSSOP-{n}"));
            assert_closed(&s);
            let (lo, hi) = extent(&written(&s.body));
            assert!(
                near(hi[0] - lo[0], length) && near(hi[1] - lo[1], 4.4),
                "{n}"
            );
        }
        // Rows running along y.
        let turned: Vec<StdPad> = dual_pads(8, 1.27, 5.3, 1.5, 0.6)
            .iter()
            .map(|p| smd(p.y, p.x, p.h, p.w))
            .collect();
        let s = recognize("U1", "", &turned).unwrap();
        assert_eq!(s.package, "SOIC-8");
        let (lo, hi) = extent(&written(&s.leads));
        assert!(near(hi[0] - lo[0], 6.0), "{lo:?} {hi:?}");
        assert_closed(&s);
        // A pin count the standard does not list is not recognized.
        assert!(recognize("U1", "", &dual_pads(12, 1.27, 5.3, 1.5, 0.6)).is_none());
    }

    fn quad_pads(per_side: usize, pitch: f64, ring: f64, len: f64, width: f64) -> Vec<StdPad> {
        let x0 = -(per_side as f64 - 1.0) * pitch / 2.0;
        let mut v = Vec::new();
        for i in 0..per_side {
            let t = x0 + i as f64 * pitch;
            v.push(smd(-ring, t, len, width));
            v.push(smd(ring, t, len, width));
            v.push(smd(t, -ring, width, len));
            v.push(smd(t, ring, width, len));
        }
        v
    }

    #[test]
    fn tqfp_and_lqfp_in_their_pin_counts() {
        for (per, pitch, ring, n, body) in [
            (8, 0.8, 4.2, 32, 7.0),
            (11, 0.8, 5.7, 44, 10.0),
            (12, 0.5, 4.2, 48, 7.0),
            (16, 0.5, 5.7, 64, 10.0),
        ] {
            let s = recognize("U1", "", &quad_pads(per, pitch, ring, 1.5, 0.3)).unwrap();
            assert_eq!(s.package, format!("TQFP-{n} ({body:.0} x {body:.0} mm)"));
            assert_eq!((s.body.len(), s.leads.len()), (1, n));
            assert_closed(&s);
            let (lo, hi) = extent(&written(&s.leads));
            assert!(near(hi[0] - lo[0], body + 2.0) && near(hi[1] - lo[1], body + 2.0));
        }
        // A low-profile name makes the body thicker.
        let s = recognize("U1", "LQFP-48", &quad_pads(12, 0.5, 4.2, 1.5, 0.3)).unwrap();
        assert!(s.package.starts_with("LQFP-48"));
        let (lo, hi) = extent(&written(&s.body));
        assert!(near(hi[2] - lo[2], 1.4));
    }

    #[test]
    fn qfn_follows_the_ring_of_pads_and_gets_an_exposed_pad() {
        let mut pads = quad_pads(4, 0.5, 1.5, 0.8, 0.25);
        let s = recognize("U1", "", &pads).unwrap();
        assert_eq!(s.package, "QFN-16 (3.0 x 3.0 mm)");
        assert_eq!(s.leads.len(), 16);
        assert_closed(&s);
        pads.push(smd(0.0, 0.0, 1.8, 1.8));
        let s = recognize("U1", "", &pads).unwrap();
        assert_eq!(s.package, "QFN-16 (3.0 x 3.0 mm)");
        assert_eq!(s.leads.len(), 17, "the exposed pad");
        assert_closed(&s);
        // The same ring of pads, long ones, with no exposed pad and a QFN name stays a QFN.
        let long = quad_pads(4, 0.5, 1.5, 1.2, 0.25);
        assert!(recognize("U1", "QFN-16", &long)
            .unwrap()
            .package
            .starts_with("QFN-16"));
    }

    #[test]
    fn dfn_is_two_rows_of_short_pads_close_together() {
        let mut pads = dual_pads(8, 0.65, 2.9, 0.8, 0.3);
        pads.push(smd(0.0, 0.0, 1.6, 2.2));
        let s = recognize("U1", "", &pads).unwrap();
        assert_eq!(s.package, "DFN-8 (3.0 x 3.0 mm)");
        assert_eq!(s.leads.len(), 9);
        assert_closed(&s);
    }

    #[test]
    fn pin_headers_of_one_and_two_rows() {
        let one: Vec<StdPad> = (0..4).map(|i| thru(i as f64 * 2.54, 0.0)).collect();
        let s = recognize("J1", "", &one).unwrap();
        assert_eq!(s.package, "1x4 pin header, 2.54 mm");
        assert_eq!((s.body.len(), s.leads.len()), (1, 4));
        assert_closed(&s);
        let (lo, hi) = extent(&written(&s.body));
        assert!(near(hi[0] - lo[0], 4.0 * 2.54) && near(hi[1] - lo[1], 2.54));
        let (lo, hi) = extent(&written(&s.leads));
        assert!(near(lo[2], -3.0) && near(hi[2], 8.5));
        let two: Vec<StdPad> = (0..3)
            .flat_map(|i| [thru(0.0, i as f64 * 2.54), thru(2.54, i as f64 * 2.54)])
            .collect();
        let s = recognize("P1", "", &two).unwrap();
        assert_eq!(s.package, "2x3 pin header, 2.54 mm");
        assert_eq!(s.leads.len(), 6);
        assert_closed(&s);
        // The same pads on a part that is not a connector, or at another pitch, are not one.
        assert!(recognize("R1", "AXIAL", &one).is_none());
        let wide: Vec<StdPad> = (0..4).map(|i| thru(i as f64 * 5.08, 0.0)).collect();
        assert!(recognize("J1", "", &wide).is_none());
    }

    #[test]
    fn what_is_not_a_known_package_is_not_recognized() {
        assert!(recognize("U1", "", &[]).is_none());
        assert!(recognize("U1", "", &[smd(0.0, 0.0, 1.0, 1.0)]).is_none());
        assert!(recognize(
            "U1",
            "",
            &[smd(f64::NAN, 0.0, 1.0, 1.0), smd(1.0, 0.0, 1.0, 1.0)]
        )
        .is_none());
        let scatter = [
            smd(0.0, 0.0, 1.0, 1.0),
            smd(3.7, 1.1, 1.0, 1.0),
            smd(-2.0, 4.0, 1.0, 1.0),
            smd(5.0, -3.0, 1.0, 1.0),
        ];
        assert!(recognize("U1", "", &scatter).is_none());
    }
}
