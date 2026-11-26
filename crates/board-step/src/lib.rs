//! Writes a board's substrate as an exact STEP solid (ISO 10303-21, AP214).
//!
//! The solid is the board outline extruded from z = 0 to z = thickness, with every cutout and
//! every drilled hole (through-hole pads and vias) cut through. Straight edges become planar side
//! faces and arcs become cylindrical faces, so the solid is exact, not tessellated.
//!
//! This crate knows the neutral board model and nothing about any input format. It never
//! panics: bad geometry is an error or a [`Note`] saying what was left out.

use std::f64::consts::PI;
use std::fmt::Write as _;

use board_model::{Board, Contour, CopperItem, Note, Point, Segment, Severity};

/// Two points closer than this (mm) are the same point.
pub const TOLERANCE: f64 = 1e-6;
/// The thickness used when the board file does not say, in mm (a common FR-4 board).
pub const DEFAULT_THICKNESS: f64 = 1.6;
/// The most segments one contour may have.
const MAX_SEGMENTS: usize = 200_000;
/// The most drilled holes a board may have.
const MAX_HOLES: usize = 200_000;
/// The most tubes [`tubes`] writes into one file.
pub const MAX_TUBES: usize = MAX_HOLES;
/// Arc sampling step for inside/outside tests, in radians.
const SAMPLE_STEP: f64 = PI / 36.0;

/// Why no solid could be written.
#[derive(Clone, Debug, PartialEq)]
pub enum StepError {
    /// The outer outline is missing, not closed, or has unusable numbers.
    Outline(String),
    /// Too much geometry to write safely.
    TooLarge(String),
    /// A size that cannot make a solid (a tube whose inner radius is not inside its outer one).
    Size(String),
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Outline(why) => write!(f, "cannot make the board solid: {why}"),
            Self::TooLarge(why) => write!(f, "the board is too large to write: {why}"),
            Self::Size(why) => write!(f, "cannot make the solid: {why}"),
        }
    }
}

impl std::error::Error for StepError {}

/// A written board solid.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardStep {
    /// The STEP file's text.
    pub text: String,
    /// Anything left out or assumed.
    pub notes: Vec<Note>,
    /// What the solid is made of, for checks.
    pub stats: Stats,
    /// The solid's height in mm: for the board, its thickness (top face at z = thickness, bottom
    /// at z = 0); for a prism, `z1 - z0`.
    pub thickness: f64,
    /// The drilled holes actually cut into the board solid (the board only; empty for a prism or
    /// tubes), so a plating can follow what was cut.
    pub holes: Vec<CutHole>,
}

/// A drilled hole that was cut through the board solid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CutHole {
    pub at: Point,
    /// The drill radius, in mm.
    pub radius: f64,
    /// Lined with copper: every via, and every pad whose drill is marked plated.
    pub plated: bool,
}

/// Counts of the solid's topology.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub vertices: usize,
    pub edges: usize,
    pub faces: usize,
    /// Inner loops on the top face: cutouts plus holes.
    pub openings: usize,
}

// ---------------------------------------------------------------------------------------------
// Plane geometry
// ---------------------------------------------------------------------------------------------

/// One edge of a contour in the plane.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Edge2 {
    start: Point,
    end: Point,
    kind: Kind2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind2 {
    Line,
    /// A circular arc around `center`, counter-clockwise when `ccw`.
    Arc {
        center: Point,
        radius: f64,
        ccw: bool,
    },
}

fn finite(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// The point of the circle nearest to `p`; `p` itself when it is the center.
fn project_on_circle(center: Point, radius: f64, p: Point) -> Point {
    let d = distance(center, p);
    if d == 0.0 {
        return p;
    }
    Point {
        x: center.x + radius * ((p.x - center.x) / d),
        y: center.y + radius * ((p.y - center.y) / d),
    }
}

/// Where two circles cross, the crossing nearest `near`, when it is within a hair of it.
fn circle_crossing_near(c0: Point, r0: f64, c1: Point, r1: f64, near: Point) -> Option<Point> {
    let d = distance(c0, c1);
    if d == 0.0 {
        return None;
    }
    let a = (r0 * r0 - r1 * r1 + d * d) / (2.0 * d);
    let h2 = r0 * r0 - a * a;
    if h2 < 0.0 {
        return None;
    }
    let h = h2.sqrt();
    let (ux, uy) = ((c1.x - c0.x) / d, (c1.y - c0.y) / d);
    let (mx, my) = (c0.x + a * ux, c0.y + a * uy);
    let one = Point {
        x: mx - h * uy,
        y: my + h * ux,
    };
    let two = Point {
        x: mx + h * uy,
        y: my - h * ux,
    };
    let best = if distance(one, near) <= distance(two, near) {
        one
    } else {
        two
    };
    (distance(best, near) < 1e-3).then_some(best)
}

fn on_circle(center: Point, radius: f64, angle: f64) -> Point {
    Point {
        x: center.x + radius * angle.cos(),
        y: center.y + radius * angle.sin(),
    }
}

/// The angle an arc sweeps from `start` to `end`, in (0, 2π], in its own direction.
fn sweep(edge: &Edge2) -> f64 {
    let Kind2::Arc { center, ccw, .. } = edge.kind else {
        return 0.0;
    };
    let a0 = (edge.start.y - center.y).atan2(edge.start.x - center.x);
    let a1 = (edge.end.y - center.y).atan2(edge.end.x - center.x);
    let raw = if ccw { a1 - a0 } else { a0 - a1 };
    let s = raw.rem_euclid(2.0 * PI);
    if s < 1e-12 {
        2.0 * PI
    } else {
        s
    }
}

/// Points along an edge, including the start and excluding the end.
fn samples(edge: &Edge2, out: &mut Vec<Point>) {
    out.push(edge.start);
    if let Kind2::Arc {
        center,
        radius,
        ccw,
    } = edge.kind
    {
        let total = sweep(edge);
        let a0 = (edge.start.y - center.y).atan2(edge.start.x - center.x);
        let steps = (total / SAMPLE_STEP).ceil().clamp(1.0, 72.0) as usize;
        let sign = if ccw { 1.0 } else { -1.0 };
        for k in 1..steps {
            let a = a0 + sign * total * (k as f64) / (steps as f64);
            out.push(on_circle(center, radius, a));
        }
    }
}

fn polygon(edges: &[Edge2]) -> Vec<Point> {
    let mut points = Vec::new();
    for e in edges {
        samples(e, &mut points);
    }
    points
}

/// The most outline points a prism's self-crossing check looks at.
const MAX_PRISM_POINTS: usize = 4096;

/// True when segments `a0 a1` and `b0 b1` cross or touch.
fn segments_meet(a0: Point, a1: Point, b0: Point, b1: Point) -> bool {
    let side = |p: Point, q: Point, r: Point| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
    let (d1, d2) = (side(b0, b1, a0), side(b0, b1, a1));
    let (d3, d4) = (side(a0, a1, b0), side(a0, a1, b1));
    let apart =
        |u: f64, v: f64| (u > TOLERANCE && v > TOLERANCE) || (u < -TOLERANCE && v < -TOLERANCE);
    !apart(d1, d2) && !apart(d3, d4)
}

/// True when a closed polygon's edges meet only where neighbors share a corner.
fn simple(points: &[Point]) -> bool {
    let n = points.len();
    let edge = |i: usize| {
        let a = points.get(i).copied();
        let b = points.get((i + 1) % n).copied();
        a.zip(b)
    };
    for i in 0..n {
        let Some((a0, a1)) = edge(i) else {
            return false;
        };
        // Skip the edge itself and its two neighbors, which share a corner with it.
        for j in i.saturating_add(2)..n {
            if i == 0 && j + 1 == n {
                continue;
            }
            let Some((b0, b1)) = edge(j) else {
                return false;
            };
            if segments_meet(a0, a1, b0, b1) {
                return false;
            }
        }
    }
    true
}

/// Twice the signed area of a polygon; positive when counter-clockwise.
fn signed_area2(points: &[Point]) -> f64 {
    let next = points.iter().cycle().skip(1);
    points
        .iter()
        .zip(next)
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum()
}

/// True when `p` is inside the polygon (even-odd rule).
fn inside(points: &[Point], p: Point) -> bool {
    let next = points.iter().cycle().skip(1);
    let mut crossings = 0usize;
    for (a, b) in points.iter().zip(next) {
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y);
            if x > p.x {
                crossings = crossings.saturating_add(1);
            }
        }
    }
    crossings % 2 == 1
}

/// The shortest distance from `p` to an edge.
fn distance_to_edge(edge: &Edge2, p: Point) -> f64 {
    match edge.kind {
        Kind2::Line => {
            let (dx, dy) = (edge.end.x - edge.start.x, edge.end.y - edge.start.y);
            let len2 = dx * dx + dy * dy;
            if len2 <= 0.0 {
                return distance(edge.start, p);
            }
            let along = (p.x - edge.start.x) * dx + (p.y - edge.start.y) * dy;
            let t = (along / len2).clamp(0.0, 1.0);
            distance(
                Point {
                    x: edge.start.x + t * dx,
                    y: edge.start.y + t * dy,
                },
                p,
            )
        }
        Kind2::Arc { center, radius, .. } => {
            let along = Edge2 {
                start: edge.start,
                end: on_circle(center, radius, (p.y - center.y).atan2(p.x - center.x)),
                kind: edge.kind,
            };
            let ends = distance(edge.start, p).min(distance(edge.end, p));
            if sweep(&along) <= sweep(edge) {
                (distance(center, p) - radius).abs().min(ends)
            } else {
                ends
            }
        }
    }
}

fn reverse(edges: &mut [Edge2]) {
    edges.reverse();
    for e in edges.iter_mut() {
        std::mem::swap(&mut e.start, &mut e.end);
        if let Kind2::Arc { ccw, .. } = &mut e.kind {
            *ccw = !*ccw;
        }
    }
}

/// Turns a board contour into plane edges: arcs longer than a half circle are split, every
/// edge starts where the previous one ended. Fails when the contour is not closed.
fn contour_edges(contour: &Contour) -> Result<Vec<Edge2>, String> {
    if contour.segments.len() > MAX_SEGMENTS {
        return Err(format!("more than {MAX_SEGMENTS} segments"));
    }
    let mut edges = Vec::new();
    for segment in &contour.segments {
        match *segment {
            Segment::Line { from, to } => {
                if !finite(from) || !finite(to) {
                    return Err("a line has a coordinate that is not a number".into());
                }
                if distance(from, to) > TOLERANCE {
                    edges.push(Edge2 {
                        start: from,
                        end: to,
                        kind: Kind2::Line,
                    });
                }
            }
            Segment::Arc {
                center,
                radius,
                start_deg,
                end_deg,
                clockwise,
            } => {
                if !finite(center) || !radius.is_finite() || radius <= TOLERANCE {
                    return Err("an arc has an unusable center or radius".into());
                }
                if !start_deg.is_finite() || !end_deg.is_finite() {
                    return Err("an arc has an unusable angle".into());
                }
                let a0 = start_deg.to_radians();
                // Signed: negative when the arc runs clockwise.
                let total = segment.sweep_deg().unwrap_or(360.0).to_radians();
                // Pieces of at most half a circle, so every side face has four edges.
                let pieces = (total.abs() / PI).ceil().max(1.0) as usize;
                for k in 0..pieces {
                    let from = a0 + total * (k as f64) / (pieces as f64);
                    let to = a0 + total * ((k + 1) as f64) / (pieces as f64);
                    edges.push(Edge2 {
                        start: on_circle(center, radius, from),
                        end: on_circle(center, radius, to),
                        kind: Kind2::Arc {
                            center,
                            radius,
                            ccw: !clockwise,
                        },
                    });
                }
            }
        }
    }
    if edges.len() < 2 {
        return Err("fewer than two edges".into());
    }
    // Each edge must start where the previous one ends; snap the tiny gaps. A vertex next to an
    // arc is put exactly on that arc's circle, so a coordinate that is a step off its circle
    // (Altium works in 1e-4 mil, 2.54e-6 mm) does not leave an arc end point off its circle.
    let count = edges.len();
    for i in 0..count {
        let prev_index = (i + count - 1) % count;
        let Some(prev) = edges.get(prev_index).copied() else {
            continue;
        };
        let Some(edge) = edges.get(i).copied() else {
            continue;
        };
        let gap = distance(prev.end, edge.start);
        if gap > 1e-3 {
            return Err(format!(
                "not closed: a gap of {gap:.4} mm near ({:.3}, {:.3})",
                edge.start.x, edge.start.y
            ));
        }
        let joint = match (prev.kind, edge.kind) {
            (Kind2::Line, Kind2::Line) => prev.end,
            // Two arcs: a point on both circles when they cross near the joint.
            (
                Kind2::Arc {
                    center: c0,
                    radius: r0,
                    ..
                },
                Kind2::Arc {
                    center: c1,
                    radius: r1,
                    ..
                },
            ) => {
                if c0 == c1 && r0 == r1 {
                    prev.end
                } else {
                    circle_crossing_near(c0, r0, c1, r1, prev.end).unwrap_or(prev.end)
                }
            }
            (Kind2::Arc { .. }, Kind2::Line) => prev.end,
            (Kind2::Line, Kind2::Arc { center, radius, .. }) => {
                project_on_circle(center, radius, prev.end)
            }
        };
        if let Some(p) = edges.get_mut(prev_index) {
            p.end = joint;
        }
        if let Some(e) = edges.get_mut(i) {
            e.start = joint;
        }
    }
    Ok(edges)
}

/// A circle as two half arcs.
fn circle_edges(center: Point, radius: f64) -> Vec<Edge2> {
    let a = Point {
        x: center.x + radius,
        y: center.y,
    };
    let b = Point {
        x: center.x - radius,
        y: center.y,
    };
    let kind = Kind2::Arc {
        center,
        radius,
        ccw: true,
    };
    vec![
        Edge2 {
            start: a,
            end: b,
            kind,
        },
        Edge2 {
            start: b,
            end: a,
            kind,
        },
    ]
}

/// The bounding box of a polygon: (min, max).
fn bounds(points: &[Point]) -> (Point, Point) {
    let mut lo = Point {
        x: f64::INFINITY,
        y: f64::INFINITY,
    };
    let mut hi = Point {
        x: f64::NEG_INFINITY,
        y: f64::NEG_INFINITY,
    };
    for p in points {
        lo.x = lo.x.min(p.x);
        lo.y = lo.y.min(p.y);
        hi.x = hi.x.max(p.x);
        hi.y = hi.y.max(p.y);
    }
    (lo, hi)
}

/// A drilled hole: center and radius.
#[derive(Clone, Copy, Debug)]
struct Drill {
    at: Point,
    radius: f64,
    plated: bool,
}

/// Every drilled hole on the board, from through-hole pads and vias.
fn drills(board: &Board, notes: &mut Vec<Note>) -> Result<Vec<Drill>, StepError> {
    let mut out = Vec::new();
    let mut unusable = 0usize;
    for item in &board.copper {
        let found = match *item {
            CopperItem::Via { at, drill, .. } => Some((at, drill, true)),
            CopperItem::Pad {
                at,
                drill: Some(drill),
                plated,
                ..
            }
            | CopperItem::PolygonPad {
                at,
                drill: Some(drill),
                plated,
                ..
            } => Some((at, drill, plated)),
            _ => None,
        };
        let Some((at, diameter, plated)) = found else {
            continue;
        };
        if !finite(at) || !diameter.is_finite() || diameter <= 2.0 * TOLERANCE {
            unusable = unusable.saturating_add(1);
            continue;
        }
        if out.len() >= MAX_HOLES {
            return Err(StepError::TooLarge(format!("more than {MAX_HOLES} holes")));
        }
        out.push(Drill {
            at,
            radius: diameter / 2.0,
            plated,
        });
    }
    if unusable > 0 {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!("{unusable} drilled hole(s) with an unusable size were not cut"),
        });
    }
    Ok(out)
}

/// True when a hole stays clear of one cutout (outside it and not touching its edge).
fn clear_of_opening(hole: Drill, poly: &(Vec<Point>, (Point, Point)), edges: &[Edge2]) -> bool {
    let (points, (lo, hi)) = poly;
    let far = hole.at.x + hole.radius < lo.x - TOLERANCE
        || hole.at.x - hole.radius > hi.x + TOLERANCE
        || hole.at.y + hole.radius < lo.y - TOLERANCE
        || hole.at.y - hole.radius > hi.y + TOLERANCE;
    far || (!inside(points, hole.at)
        && edges
            .iter()
            .all(|e| distance_to_edge(e, hole.at) > hole.radius + TOLERANCE))
}

/// The board's closed outer contour and the openings that can be cut cleanly.
struct Profile {
    outer: Vec<Edge2>,
    openings: Vec<Vec<Edge2>>,
    holes: Vec<CutHole>,
}

fn profile(board: &Board, notes: &mut Vec<Note>) -> Result<Profile, StepError> {
    if board.outline.outer.segments.is_empty() {
        return Err(StepError::Outline("the board has no outline".into()));
    }
    let mut outer = contour_edges(&board.outline.outer).map_err(StepError::Outline)?;
    let outer_poly = polygon(&outer);
    if signed_area2(&outer_poly).abs() < TOLERANCE {
        return Err(StepError::Outline("the outline encloses no area".into()));
    }
    if signed_area2(&outer_poly) < 0.0 {
        reverse(&mut outer);
    }

    // Cutouts first, as the file gives them.
    let mut openings: Vec<Vec<Edge2>> = Vec::new();
    let mut opening_polys: Vec<(Vec<Point>, (Point, Point))> = Vec::new();
    for (index, cutout) in board.outline.cutouts.iter().enumerate() {
        let mut edges = match contour_edges(cutout) {
            Ok(edges) => edges,
            Err(why) => {
                notes.push(Note {
                    severity: Severity::Dropped,
                    message: format!("cutout {} was not cut: {why}", index.saturating_add(1)),
                });
                continue;
            }
        };
        let poly = polygon(&edges);
        let inside_outer = poly.iter().all(|p| inside(&outer_poly, *p))
            && edges.iter().all(|e| {
                outer
                    .iter()
                    .all(|o| distance_to_edge(o, e.start) > TOLERANCE)
            });
        if !inside_outer {
            notes.push(Note {
                severity: Severity::Dropped,
                message: format!(
                    "cutout {} touches or crosses the board edge and was not cut",
                    index.saturating_add(1)
                ),
            });
            continue;
        }
        if signed_area2(&poly) > 0.0 {
            reverse(&mut edges);
        }
        let b = bounds(&poly);
        opening_polys.push((poly, b));
        openings.push(edges);
    }

    // Then drilled holes, skipping any that would touch the edge, a cutout or another hole.
    let mut holes = drills(board, notes)?;
    holes.sort_by(|a, b| (a.at.x - a.radius).total_cmp(&(b.at.x - b.radius)));
    let widest = holes.iter().map(|h| 2.0 * h.radius).fold(0.0, f64::max);
    let mut kept: Vec<Drill> = Vec::new();
    let (mut at_edge, mut in_cutout, mut overlapping) = (0usize, 0usize, 0usize);
    for hole in holes {
        let clear_of_edge = inside(&outer_poly, hole.at)
            && outer
                .iter()
                .all(|o| distance_to_edge(o, hole.at) > hole.radius + TOLERANCE);
        if !clear_of_edge {
            at_edge = at_edge.saturating_add(1);
            continue;
        }
        let clear_of_cutouts = opening_polys
            .iter()
            .zip(&openings)
            .all(|(poly, edges)| clear_of_opening(hole, poly, edges));
        if !clear_of_cutouts {
            in_cutout = in_cutout.saturating_add(1);
            continue;
        }
        // Kept holes are sorted by their left edge, and none is wider than `widest`, so only
        // those whose left edge is within `widest` of this one can reach it.
        let left = hole.at.x - hole.radius;
        let clear_of_holes = kept
            .iter()
            .rev()
            .take_while(|k| k.at.x - k.radius >= left - widest - TOLERANCE)
            .all(|k| distance(k.at, hole.at) > k.radius + hole.radius + TOLERANCE);
        if !clear_of_holes {
            overlapping = overlapping.saturating_add(1);
            continue;
        }
        kept.push(hole);
    }
    for (count, why) in [
        (at_edge, "touch or cross the board edge"),
        (in_cutout, "touch or fall inside a cutout"),
        (overlapping, "overlap another hole"),
    ] {
        if count > 0 {
            notes.push(Note {
                severity: Severity::Dropped,
                message: format!("{count} drilled hole(s) {why} and were not cut"),
            });
        }
    }
    let mut cut = Vec::new();
    for hole in kept {
        cut.push(CutHole {
            at: hole.at,
            radius: hole.radius,
            plated: hole.plated,
        });
        let mut edges = circle_edges(hole.at, hole.radius);
        reverse(&mut edges);
        openings.push(edges);
    }
    Ok(Profile {
        outer,
        openings,
        holes: cut,
    })
}

// ---------------------------------------------------------------------------------------------
// STEP text
// ---------------------------------------------------------------------------------------------

/// A STEP real: always with a decimal point, never "-0.".
fn real(v: f64) -> String {
    let v = if v.abs() < 1e-12 { 0.0 } else { v };
    let mut s = format!("{v:.9}");
    while s.ends_with('0') {
        s.pop();
    }
    s
}

/// A STEP string: printable ASCII kept, quotes doubled, anything else replaced.
fn text(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        match c {
            '\'' => out.push_str("''"),
            '\\' => out.push_str("\\\\"),
            ' '..='~' => out.push(c),
            _ => out.push('_'),
        }
    }
    out.push('\'');
    out
}

/// Collects numbered entities.
#[derive(Default)]
struct Writer {
    lines: Vec<String>,
}

impl Writer {
    fn add(&mut self, entity: String) -> usize {
        self.lines.push(entity);
        self.lines.len()
    }

    fn point(&mut self, x: f64, y: f64, z: f64) -> usize {
        self.add(format!(
            "CARTESIAN_POINT('',({},{},{}))",
            real(x),
            real(y),
            real(z)
        ))
    }

    fn direction(&mut self, x: f64, y: f64, z: f64) -> usize {
        self.add(format!(
            "DIRECTION('',({},{},{}))",
            real(x),
            real(y),
            real(z)
        ))
    }

    fn placement(&mut self, origin: [f64; 3], axis: [f64; 3], reference: [f64; 3]) -> usize {
        let [x, y, z] = origin;
        let p = self.point(x, y, z);
        let [ax, ay, az] = axis;
        let a = self.direction(ax, ay, az);
        let [rx, ry, rz] = reference;
        let r = self.direction(rx, ry, rz);
        self.add(format!("AXIS2_PLACEMENT_3D('',#{p},#{a},#{r})"))
    }

    fn line(&mut self, from: [f64; 3], to: [f64; 3]) -> usize {
        let [x0, y0, z0] = from;
        let [x1, y1, z1] = to;
        let (dx, dy, dz) = (x1 - x0, y1 - y0, z1 - z0);
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        let p = self.point(x0, y0, z0);
        let d = self.direction(dx / len, dy / len, dz / len);
        let v = self.add(format!("VECTOR('',#{d},{})", real(len)));
        self.add(format!("LINE('',#{p},#{v})"))
    }

    fn circle(&mut self, center: Point, z: f64, radius: f64) -> usize {
        let a = self.placement([center.x, center.y, z], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
        self.add(format!("CIRCLE('',#{a},{})", real(radius)))
    }
}

/// An edge in the solid, used once in each direction by the faces around it.
#[derive(Clone, Copy)]
struct Edge3 {
    id: usize,
}

/// One contour's edges at the bottom and the top, and the vertical edges at its corners.
struct Ring {
    edges: Vec<Edge2>,
    bottom: Vec<Edge3>,
    top: Vec<Edge3>,
    up: Vec<Edge3>,
}

fn oriented(w: &mut Writer, edge: Edge3, forward: bool) -> usize {
    let sense = if forward { ".T." } else { ".F." };
    w.add(format!("ORIENTED_EDGE('',*,*,#{},{sense})", edge.id))
}

fn edge_loop(w: &mut Writer, edges: &[(Edge3, bool)]) -> usize {
    let ids: Vec<String> = edges
        .iter()
        .map(|(e, forward)| format!("#{}", oriented(w, *e, *forward)))
        .collect();
    w.add(format!("EDGE_LOOP('',({}))", ids.join(",")))
}

fn ring(w: &mut Writer, edges: Vec<Edge2>, (z0, z1): (f64, f64), stats: &mut Stats) -> Ring {
    let mut vb = Vec::new();
    let mut vt = Vec::new();
    for e in &edges {
        let pb = w.point(e.start.x, e.start.y, z0);
        vb.push(w.add(format!("VERTEX_POINT('',#{pb})")));
        let pt = w.point(e.start.x, e.start.y, z1);
        vt.push(w.add(format!("VERTEX_POINT('',#{pt})")));
    }
    stats.vertices = stats.vertices.saturating_add(vb.len().saturating_mul(2));
    let count = edges.len();
    let mut bottom = Vec::new();
    let mut top = Vec::new();
    let mut up = Vec::new();
    for (i, e) in edges.iter().enumerate() {
        let next = (i + 1) % count;
        let (Some(&b0), Some(&b1), Some(&t0), Some(&t1)) =
            (vb.get(i), vb.get(next), vt.get(i), vt.get(next))
        else {
            continue;
        };
        for (z, v0, v1, list) in [(z0, b0, b1, &mut bottom), (z1, t0, t1, &mut top)] {
            let (curve, sense) = match e.kind {
                Kind2::Line => (
                    w.line([e.start.x, e.start.y, z], [e.end.x, e.end.y, z]),
                    ".T.",
                ),
                Kind2::Arc {
                    center,
                    radius,
                    ccw,
                } => (w.circle(center, z, radius), if ccw { ".T." } else { ".F." }),
            };
            let id = w.add(format!("EDGE_CURVE('',#{v0},#{v1},#{curve},{sense})"));
            list.push(Edge3 { id });
        }
        let vertical = w.line([e.start.x, e.start.y, z0], [e.start.x, e.start.y, z1]);
        let id = w.add(format!("EDGE_CURVE('',#{b0},#{t0},#{vertical},.T.)"));
        up.push(Edge3 { id });
    }
    stats.edges = stats.edges.saturating_add(count.saturating_mul(3));
    Ring {
        edges,
        bottom,
        top,
        up,
    }
}

/// The side faces of one ring. Material is on the left of every edge (outer counter-clockwise,
/// openings clockwise), so each face's outward normal is to the right.
fn side_faces(w: &mut Writer, ring: &Ring, z0: f64, faces: &mut Vec<usize>) {
    let count = ring.edges.len();
    for (i, e) in ring.edges.iter().enumerate() {
        let next = (i + 1) % count;
        let (Some(&b), Some(&t), Some(&u0), Some(&u1)) = (
            ring.bottom.get(i),
            ring.top.get(i),
            ring.up.get(i),
            ring.up.get(next),
        ) else {
            continue;
        };
        let lp = edge_loop(w, &[(b, true), (u1, true), (t, false), (u0, false)]);
        let bound = w.add(format!("FACE_OUTER_BOUND('',#{lp},.T.)"));
        let (surface, sense) = match e.kind {
            Kind2::Line => {
                let (dx, dy) = (e.end.x - e.start.x, e.end.y - e.start.y);
                let len = dx.hypot(dy);
                let (tx, ty) = (dx / len, dy / len);
                let a = w.placement([e.start.x, e.start.y, z0], [ty, -tx, 0.0], [tx, ty, 0.0]);
                (w.add(format!("PLANE('',#{a})")), ".T.")
            }
            Kind2::Arc {
                center,
                radius,
                ccw,
            } => {
                let a = w.placement([center.x, center.y, z0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
                let s = w.add(format!("CYLINDRICAL_SURFACE('',#{a},{})", real(radius)));
                (s, if ccw { ".T." } else { ".F." })
            }
        };
        faces.push(w.add(format!("ADVANCED_FACE('',(#{bound}),#{surface},{sense})")));
    }
}

/// Adds one body to `w`: `outer` (counter-clockwise) with `openings` (clockwise) cut through
/// it, extruded from `z0` up to `z1`. Returns the id of its `MANIFOLD_SOLID_BREP`.
fn body(
    w: &mut Writer,
    outer: Vec<Edge2>,
    openings: Vec<Vec<Edge2>>,
    (z0, z1): (f64, f64),
    name: &str,
    stats: &mut Stats,
) -> usize {
    let mut rings = vec![ring(w, outer, (z0, z1), stats)];
    for opening in openings {
        rings.push(ring(w, opening, (z0, z1), stats));
    }
    let mut faces = Vec::new();
    for r in &rings {
        side_faces(w, r, z0, &mut faces);
    }
    // Top face, normal +Z: every ring's top edges forward.
    let mut top_bounds = Vec::new();
    // Bottom face, normal -Z: every ring's bottom edges backwards.
    let mut bottom_bounds = Vec::new();
    for (k, r) in rings.iter().enumerate() {
        let kind = if k == 0 {
            "FACE_OUTER_BOUND"
        } else {
            "FACE_BOUND"
        };
        let forward: Vec<(Edge3, bool)> = r.top.iter().map(|e| (*e, true)).collect();
        let lt = edge_loop(w, &forward);
        top_bounds.push(w.add(format!("{kind}('',#{lt},.T.)")));
        let backward: Vec<(Edge3, bool)> = r.bottom.iter().rev().map(|e| (*e, false)).collect();
        let lb = edge_loop(w, &backward);
        bottom_bounds.push(w.add(format!("{kind}('',#{lb},.T.)")));
    }
    let ids = |v: &[usize]| {
        v.iter()
            .map(|i| format!("#{i}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    let top_plane = w.placement([0.0, 0.0, z1], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
    let top_surface = w.add(format!("PLANE('',#{top_plane})"));
    faces.push(w.add(format!(
        "ADVANCED_FACE('',({}),#{top_surface},.T.)",
        ids(&top_bounds)
    )));
    let bottom_plane = w.placement([0.0, 0.0, z0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]);
    let bottom_surface = w.add(format!("PLANE('',#{bottom_plane})"));
    faces.push(w.add(format!(
        "ADVANCED_FACE('',({}),#{bottom_surface},.T.)",
        ids(&bottom_bounds)
    )));
    stats.faces = stats.faces.saturating_add(faces.len());

    let shell = w.add(format!("CLOSED_SHELL('',({}))", ids(&faces)));
    w.add(format!("MANIFOLD_SOLID_BREP({},#{shell})", text(name)))
}

/// The ids as a STEP list: `#1,#2`.
fn id_list(ids: &[usize]) -> String {
    ids.iter()
        .map(|i| format!("#{i}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Wraps the bodies `breps` already added to `w` into a whole STEP file.
fn finish(mut w: Writer, breps: &[usize], name: &str, description: &str) -> String {
    let origin = w.placement([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
    let length = w.add("( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) )".into());
    let angle = w.add("( NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.) )".into());
    let solid_angle = w.add("( NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT() )".into());
    let uncertainty = w.add(format!(
        "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-06),#{length},\
         'distance_accuracy_value','confusion accuracy')"
    ));
    let context = w.add(format!(
        "( GEOMETRIC_REPRESENTATION_CONTEXT(3) \
         GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{uncertainty})) \
         GLOBAL_UNIT_ASSIGNED_CONTEXT((#{length},#{angle},#{solid_angle})) \
         REPRESENTATION_CONTEXT('Context #1','3D Context with UNIT and UNCERTAINTY') )"
    ));
    let shape = w.add(format!(
        "ADVANCED_BREP_SHAPE_REPRESENTATION('',(#{origin},{}),#{context})",
        id_list(breps)
    ));
    let app = w.add("APPLICATION_CONTEXT('automotive design')".into());
    w.add(format!(
        "APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{app})"
    ));
    let product_context = w.add(format!("PRODUCT_CONTEXT('',#{app},'mechanical')"));
    let product = w.add(format!(
        "PRODUCT({0},{0},'',(#{product_context}))",
        text(name)
    ));
    w.add(format!(
        "PRODUCT_RELATED_PRODUCT_CATEGORY('part',$,(#{product}))"
    ));
    let formation = w.add(format!("PRODUCT_DEFINITION_FORMATION('','',#{product})"));
    let definition_context = w.add(format!(
        "PRODUCT_DEFINITION_CONTEXT('part definition',#{app},'design')"
    ));
    let definition = w.add(format!(
        "PRODUCT_DEFINITION('design','',#{formation},#{definition_context})"
    ));
    let definition_shape = w.add(format!("PRODUCT_DEFINITION_SHAPE('','',#{definition})"));
    w.add(format!(
        "SHAPE_DEFINITION_REPRESENTATION(#{definition_shape},#{shape})"
    ));

    let mut out = String::new();
    out.push_str("ISO-10303-21;\nHEADER;\n");
    let _ = writeln!(out, "FILE_DESCRIPTION(({}),'2;1');", text(description));
    // A fixed time stamp: the same board always gives the same file.
    let _ = writeln!(
        out,
        "FILE_NAME({},'2000-01-01T00:00:00',(''),(''),'dutchman-electronics','dutchman-electronics','');",
        text(name)
    );
    out.push_str("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\nENDSEC;\nDATA;\n");
    for (i, line) in w.lines.iter().enumerate() {
        let _ = writeln!(out, "#{}={line};", i.saturating_add(1));
    }
    out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    out
}

/// Writes one solid as a whole STEP file: `outer` with `openings` cut through it, from `z0` to
/// `z1`.
fn solid(
    outer: Vec<Edge2>,
    openings: Vec<Vec<Edge2>>,
    z: (f64, f64),
    name: &str,
    description: &str,
    stats: &mut Stats,
) -> String {
    let mut w = Writer::default();
    let brep = body(&mut w, outer, openings, z, name, stats);
    finish(w, &[brep], name, description)
}

/// Writes the board substrate as a STEP file. `name` becomes the product name.
pub fn write(board: &Board, name: &str) -> Result<BoardStep, StepError> {
    let mut notes = Vec::new();
    let thickness = match board.thickness {
        Some(t) if t.is_finite() && t > TOLERANCE => t,
        Some(_) => {
            return Err(StepError::Outline(
                "the board thickness is not usable".into(),
            ))
        }
        None => {
            notes.push(Note {
                severity: Severity::Warning,
                message: format!(
                    "the board file gives no thickness; the solid uses {DEFAULT_THICKNESS} mm"
                ),
            });
            DEFAULT_THICKNESS
        }
    };
    let profile = profile(board, &mut notes)?;
    let mut stats = Stats {
        openings: profile.openings.len(),
        ..Stats::default()
    };

    let text = solid(
        profile.outer,
        profile.openings,
        (0.0, thickness),
        name,
        "Board substrate",
        &mut stats,
    );
    Ok(BoardStep {
        text,
        notes,
        stats,
        thickness,
        holes: profile.holes,
    })
}

/// Writes a prism as a STEP file: the closed `outline` (in the XY plane, either winding)
/// extruded from `z0` up to `z1`. Used for components' extruded 3D bodies. `name` becomes the
/// product name.
pub fn prism(outline: &Contour, z0: f64, z1: f64, name: &str) -> Result<BoardStep, StepError> {
    if !(z0.is_finite() && z1.is_finite() && z1 - z0 > TOLERANCE) {
        return Err(StepError::Outline(format!(
            "a prism needs its top above its bottom (from {z0} to {z1})"
        )));
    }
    if outline.segments.is_empty() {
        return Err(StepError::Outline("the prism has no outline".into()));
    }
    let mut outer = contour_edges(outline).map_err(StepError::Outline)?;
    let points = polygon(&outer);
    if points.len() > MAX_PRISM_POINTS {
        return Err(StepError::TooLarge(format!(
            "an extruded outline of more than {MAX_PRISM_POINTS} points"
        )));
    }
    if !simple(&points) {
        return Err(StepError::Outline("the outline crosses itself".into()));
    }
    let area = signed_area2(&points);
    if area.abs() < TOLERANCE {
        return Err(StepError::Outline("the outline encloses no area".into()));
    }
    if area < 0.0 {
        reverse(&mut outer);
    }
    let mut stats = Stats::default();
    let text = solid(
        outer,
        Vec::new(),
        (z0, z1),
        name,
        "Extruded body",
        &mut stats,
    );
    Ok(BoardStep {
        text,
        notes: Vec::new(),
        stats,
        thickness: z1 - z0,
        holes: Vec::new(),
    })
}

/// One plated hole's lining: a tube round `at` (in the XY plane) with the given radii.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tube {
    pub at: Point,
    /// The drill radius, in mm: the tube's outside, where the copper meets the substrate.
    pub outer_radius: f64,
    /// The radius left open in the middle, in mm; above 0 and below `outer_radius`.
    pub inner_radius: f64,
}

/// Writes tubes as one STEP file with one closed `MANIFOLD_SOLID_BREP` for each, all running from
/// `z0` up to `z1`. Each is a ring (a circle with a circle cut out of it) extruded, so its sides
/// are exact cylinders. An error when no tube is given, more than [`MAX_TUBES`], or a tube's
/// position or radii are unusable (not numbers, an inner radius of 0 or less, or one that is
/// not smaller than the outer radius). `name` becomes the product name.
pub fn tubes(list: &[Tube], z0: f64, z1: f64, name: &str) -> Result<BoardStep, StepError> {
    if !(z0.is_finite() && z1.is_finite() && z1 - z0 > TOLERANCE) {
        return Err(StepError::Size(format!(
            "tubes need their top above their bottom (from {z0} to {z1})"
        )));
    }
    if list.is_empty() {
        return Err(StepError::Size("no tubes were given".into()));
    }
    if list.len() > MAX_TUBES {
        return Err(StepError::TooLarge(format!("more than {MAX_TUBES} tubes")));
    }
    let mut w = Writer::default();
    let mut stats = Stats::default();
    let mut breps = Vec::new();
    for t in list {
        let usable = finite(t.at)
            && t.outer_radius.is_finite()
            && t.inner_radius.is_finite()
            && t.inner_radius > TOLERANCE
            && t.inner_radius < t.outer_radius - TOLERANCE
            && (t.at.x + t.outer_radius).is_finite()
            && (t.at.x - t.outer_radius).is_finite()
            && (t.at.y + t.outer_radius).is_finite()
            && (t.at.y - t.outer_radius).is_finite();
        if !usable {
            return Err(StepError::Size(format!(
                "a tube at ({}, {}) has radii {} and {}; the inner radius must be above 0 and \
                 below the outer radius",
                t.at.x, t.at.y, t.inner_radius, t.outer_radius
            )));
        }
        let outer = circle_edges(t.at, t.outer_radius);
        let mut inner = circle_edges(t.at, t.inner_radius);
        reverse(&mut inner);
        breps.push(body(&mut w, outer, vec![inner], (z0, z1), name, &mut stats));
    }
    stats.openings = list.len();
    Ok(BoardStep {
        text: finish(w, &breps, name, "Plating of plated holes"),
        notes: Vec::new(),
        stats,
        thickness: z1 - z0,
        holes: Vec::new(),
    })
}

#[cfg(test)]
mod tests;
