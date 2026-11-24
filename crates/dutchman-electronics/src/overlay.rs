//! Copper and silkscreen as flat overlays for the parts package (`overlays` in
//! `dutchman.parts/1`, agreed on dutchman-cad/dutchman#1005).
//!
//! Dutchman only draws polygons, so all the geometry happens here: every item becomes a polygon
//! (a track a stroke with round ends, a pad its shape, a via its ring), the polygons of one
//! overlay are united, every drill is cut out, and masked copper stops where a pad begins. Arcs
//! are polylines within [`CHORD`] mm. Up to six overlays come out, for each side: copper under
//! the mask (`masked`), bare pad copper (`bare`), and silkscreen.
//!
//! This module knows the neutral board model only, nothing about any input format.

use std::f64::consts::PI;
use std::panic::{catch_unwind, AssertUnwindSafe};

use board_model::{Board, Contour, CopperItem, Graphic, LayerKind, PadShape, Point, Segment, Side};
use geo::orient::{Direction, Orient};
use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon};
use serde_json::{json, Value};

/// The most an arc's polyline strays from the true arc, in mm.
pub const CHORD: f64 = 0.02;
/// How far copper is drawn off the board face, in mm (as SolidCraft's KiCad import does).
pub const COPPER_LIFT: f64 = 0.001;
/// How far silkscreen is drawn off the board face, in mm.
pub const SILK_LIFT: f64 = 0.01;
/// The most points one arc is drawn with.
const MAX_ARC_POINTS: usize = 720;
/// The fewest sides a whole circle is drawn with, so small vias and pads stay round.
const MIN_CIRCLE_SIDES: f64 = 32.0;

fn coord(p: Point) -> Coord {
    Coord { x: p.x, y: p.y }
}

fn finite(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// Points along a counter-clockwise (positive `sweep_deg`) or clockwise arc, both ends included.
fn arc_points(center: Point, radius: f64, start_deg: f64, sweep_deg: f64, out: &mut Vec<Coord>) {
    let step = if radius > CHORD {
        2.0 * (1.0 - CHORD / radius).acos()
    } else {
        PI / 4.0
    };
    let steps = (sweep_deg.abs().to_radians() / step)
        .max(MIN_CIRCLE_SIDES * sweep_deg.abs() / 360.0)
        .ceil()
        .clamp(1.0, MAX_ARC_POINTS as f64) as usize;
    for k in 0..=steps {
        let a = (start_deg + sweep_deg * (k as f64) / (steps as f64)).to_radians();
        out.push(Coord {
            x: center.x + radius * a.cos(),
            y: center.y + radius * a.sin(),
        });
    }
}

fn polygon(mut ring: Vec<Coord>) -> Option<Polygon> {
    ring.dedup_by(|a, b| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9);
    if ring.first() == ring.last() {
        ring.pop();
    }
    if ring.len() < 3 || ring.iter().any(|c| !(c.x.is_finite() && c.y.is_finite())) {
        return None;
    }
    Some(Polygon::new(LineString::from(ring), vec![]))
}

fn circle(center: Point, radius: f64) -> Option<Polygon> {
    if !(radius.is_finite() && radius > 0.0 && finite(center)) {
        return None;
    }
    let mut ring = Vec::new();
    arc_points(center, radius, 0.0, 360.0, &mut ring);
    polygon(ring)
}

/// A straight stroke of `width` from `a` to `b` with round ends.
fn stroke(a: Point, b: Point, width: f64) -> Option<Polygon> {
    if !(width.is_finite() && width > 0.0 && finite(a) && finite(b)) {
        return None;
    }
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if dx.hypot(dy) < 1e-9 {
        return circle(a, width / 2.0);
    }
    let heading = dy.atan2(dx).to_degrees();
    let mut ring = Vec::new();
    arc_points(b, width / 2.0, heading - 90.0, 180.0, &mut ring);
    arc_points(a, width / 2.0, heading + 90.0, 180.0, &mut ring);
    polygon(ring)
}

/// The counter-clockwise sweep from `start_deg` to `end_deg`; a whole turn when they are equal.
fn sweep(start_deg: f64, end_deg: f64) -> f64 {
    let s = (end_deg - start_deg).rem_euclid(360.0);
    if s < 1e-9 {
        360.0
    } else {
        s
    }
}

/// An arc stroke: a round-ended stroke along each piece of the arc's polyline.
fn arc_stroke(center: Point, radius: f64, start_deg: f64, end_deg: f64, width: f64) -> Vec<Polygon> {
    if !(radius.is_finite() && radius > 0.0 && finite(center)) {
        return Vec::new();
    }
    let mut points = Vec::new();
    arc_points(center, radius, start_deg, sweep(start_deg, end_deg), &mut points);
    points
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => stroke(Point { x: a.x, y: a.y }, Point { x: b.x, y: b.y }, width),
            _ => None,
        })
        .collect()
}

/// A board point turned by `deg` about `center`.
fn turned(center: Point, dx: f64, dy: f64, deg: f64) -> Point {
    let (s, c) = deg.to_radians().sin_cos();
    Point {
        x: center.x + dx * c - dy * s,
        y: center.y + dx * s + dy * c,
    }
}

fn pad(at: Point, shape: PadShape, size_x: f64, size_y: f64, rotation_deg: f64) -> Option<Polygon> {
    if !(size_x.is_finite() && size_y.is_finite() && size_x > 0.0 && size_y > 0.0) {
        return None;
    }
    let (hx, hy) = (size_x / 2.0, size_y / 2.0);
    match shape {
        PadShape::Rectangle => polygon(
            [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)]
                .iter()
                .map(|&(x, y)| coord(turned(at, x, y, rotation_deg)))
                .collect(),
        ),
        PadShape::Round | PadShape::Oval if (size_x - size_y).abs() < 1e-9 => circle(at, hx),
        PadShape::Round | PadShape::Oval => {
            // A slot: a stroke along the longer side, as wide as the shorter one.
            let (along, width) = if size_x > size_y {
                ((hx - hy, 0.0), size_y)
            } else {
                ((0.0, hy - hx), size_x)
            };
            stroke(
                turned(at, -along.0, -along.1, rotation_deg),
                turned(at, along.0, along.1, rotation_deg),
                width,
            )
        }
    }
}

fn contour(c: &Contour) -> Option<Polygon> {
    let mut ring = Vec::new();
    for segment in &c.segments {
        match *segment {
            Segment::Line { from, .. } => ring.push(coord(from)),
            Segment::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                if !(radius.is_finite() && radius > 0.0) {
                    return None;
                }
                arc_points(center, radius, start_deg, sweep(start_deg, end_deg), &mut ring);
            }
        }
    }
    polygon(ring)
}

/// Polygons gathered for one overlay before they are united.
#[derive(Default)]
struct Pile {
    polygons: Vec<Polygon>,
}

impl Pile {
    fn add(&mut self, p: Option<Polygon>, unusable: &mut usize) {
        match p {
            Some(p) => self.polygons.push(p),
            None => *unusable = unusable.saturating_add(1),
        }
    }

    fn union(&self) -> MultiPolygon {
        geo::unary_union(&self.polygons)
    }
}

#[derive(Default)]
struct SideFaces {
    masked: Pile,
    bare: Pile,
    silk: Pile,
}

/// The area of a multipolygon's rings as the package carries them: `outer` counter-clockwise,
/// `holes` clockwise, each ring open (its first point not repeated), in mm to 1 nm.
fn areas(shape: &MultiPolygon) -> Vec<Value> {
    let ring = |l: &LineString| -> Vec<[f64; 2]> {
        let mut pts: Vec<[f64; 2]> = l
            .coords()
            .map(|c| [(c.x * 1e6).round() / 1e6, (c.y * 1e6).round() / 1e6])
            .collect();
        if pts.len() > 1 && pts.first() == pts.last() {
            pts.pop();
        }
        pts
    };
    shape
        .orient(Direction::Default)
        .iter()
        .map(|p| {
            json!({
                "outer": ring(p.exterior()),
                "holes": p.interiors().iter().map(ring).collect::<Vec<_>>(),
            })
        })
        .collect()
}

/// The overlays of a board whose substrate is `thickness` thick (its top face at z = thickness,
/// its bottom at z = 0, in the frame of the part named `board_part`), and notes on what was left
/// out.
pub fn overlays(board: &Board, thickness: f64, board_part: &str) -> (Vec<Value>, Vec<Value>) {
    let mut top = SideFaces::default();
    let mut bottom = SideFaces::default();
    let mut holes = Pile::default();
    let (mut inner, mut unusable, mut other_graphics) = (0usize, 0usize, 0usize);
    let layer = |i: usize| board.layers.get(i).map(|l| (l.kind, l.side));
    // Items are sorted by side first, then piled per overlay.
    let mut masked: Vec<(Option<Side>, Option<Polygon>)> = Vec::new();
    let mut masked_arcs: Vec<(Option<Side>, Vec<Polygon>)> = Vec::new();
    let mut bare: Vec<(Option<Side>, Option<Polygon>)> = Vec::new();
    for item in &board.copper {
        match item {
            CopperItem::Track {
                layer: l,
                from,
                to,
                width,
                ..
            } => masked.push((layer(*l).and_then(|k| k.1), stroke(*from, *to, *width))),
            CopperItem::TrackArc {
                layer: l,
                center,
                radius,
                start_deg,
                end_deg,
                width,
                ..
            } => masked_arcs.push((
                layer(*l).and_then(|k| k.1),
                arc_stroke(*center, *radius, *start_deg, *end_deg, *width),
            )),
            CopperItem::Region { layer: l, contour: c } => {
                masked.push((layer(*l).and_then(|k| k.1), contour(c)));
            }
            CopperItem::Via {
                at,
                diameter,
                drill,
            } => {
                let ring = circle(*at, diameter / 2.0);
                masked.push((Some(Side::Top), ring.clone()));
                masked.push((Some(Side::Bottom), ring));
                holes.add(circle(*at, drill / 2.0), &mut unusable);
            }
            CopperItem::Pad {
                layer: l,
                at,
                shape,
                size_x,
                size_y,
                rotation_deg,
                drill,
                ..
            } => {
                let p = pad(*at, *shape, *size_x, *size_y, *rotation_deg);
                match (layer(*l).and_then(|k| k.1), drill) {
                    (Some(side), _) => bare.push((Some(side), p)),
                    // A pad through the board with no side of its own is on both faces.
                    (None, Some(_)) => {
                        bare.push((Some(Side::Top), p.clone()));
                        bare.push((Some(Side::Bottom), p));
                    }
                    (None, None) => inner = inner.saturating_add(1),
                }
                if let Some(d) = drill {
                    holes.add(circle(*at, d / 2.0), &mut unusable);
                }
            }
        }
    }
    for (side, p) in masked {
        match side {
            Some(Side::Top) => top.masked.add(p, &mut unusable),
            Some(Side::Bottom) => bottom.masked.add(p, &mut unusable),
            None => inner = inner.saturating_add(1),
        }
    }
    for (side, ps) in masked_arcs {
        let pile = match side {
            Some(Side::Top) => &mut top.masked,
            Some(Side::Bottom) => &mut bottom.masked,
            None => {
                inner = inner.saturating_add(1);
                continue;
            }
        };
        if ps.is_empty() {
            unusable = unusable.saturating_add(1);
        }
        pile.polygons.extend(ps);
    }
    for (side, p) in bare {
        match side {
            Some(Side::Top) => top.bare.add(p, &mut unusable),
            Some(Side::Bottom) => bottom.bare.add(p, &mut unusable),
            None => inner = inner.saturating_add(1),
        }
    }
    for g in &board.graphics {
        let (l, shapes) = match g {
            Graphic::Line {
                layer,
                from,
                to,
                width,
                ..
            } => (*layer, stroke(*from, *to, *width).into_iter().collect()),
            Graphic::Arc {
                layer,
                center,
                radius,
                start_deg,
                end_deg,
                width,
                ..
            } => (
                *layer,
                arc_stroke(*center, *radius, *start_deg, *end_deg, *width),
            ),
        };
        let pile = match layer(l) {
            Some((LayerKind::Silkscreen, Some(Side::Top))) => &mut top.silk,
            Some((LayerKind::Silkscreen, Some(Side::Bottom))) => &mut bottom.silk,
            _ => {
                other_graphics = other_graphics.saturating_add(1);
                continue;
            }
        };
        if shapes.is_empty() {
            unusable = unusable.saturating_add(1);
        }
        pile.polygons.extend(shapes);
    }

    let mut out = Vec::new();
    let mut notes = Vec::new();
    // A failure inside the polygon library must not take the whole package down with it.
    let built = catch_unwind(AssertUnwindSafe(|| {
        let drilled = holes.union();
        let mut built = Vec::new();
        for (side, f, z_copper, z_silk) in [
            ("top", &top, thickness + COPPER_LIFT, thickness + SILK_LIFT),
            ("bottom", &bottom, -COPPER_LIFT, -SILK_LIFT),
        ] {
            let bare = f.bare.union().difference(&drilled);
            let masked = f.masked.union().difference(&drilled).difference(&bare);
            let silk = f.silk.union();
            built.push((format!("Copper {side}"), "copper", Some("masked"), side, z_copper, masked));
            built.push((format!("Copper {side} pads"), "copper", Some("bare"), side, z_copper, bare));
            built.push((format!("Silkscreen {side}"), "silkscreen", None, side, z_silk, silk));
        }
        built
    }));
    match built {
        Ok(built) => {
            for (name, kind, finish, side, z, shape) in built {
                if shape.0.is_empty() {
                    continue;
                }
                let mut o = json!({
                    "name": name,
                    "part": board_part,
                    "kind": kind,
                    "side": side,
                    "z": (z * 1e6).round() / 1e6,
                    "areas": areas(&shape),
                });
                if let (Some(finish), Some(map)) = (finish, o.as_object_mut()) {
                    map.insert("finish".into(), json!(finish));
                }
                out.push(o);
            }
        }
        Err(_) => notes.push(json!({
            "severity": "Dropped",
            "message": "the copper and silkscreen overlays could not be drawn",
        })),
    }
    for (count, what) in [
        (inner, "copper item(s) on inner layers are not drawn yet"),
        (unusable, "copper or silkscreen item(s) have unusable sizes and were left out"),
        (
            other_graphics,
            "line(s) and arc(s) on layers other than the silkscreen are not drawn yet",
        ),
    ] {
        if count > 0 {
            notes.push(json!({ "severity": "Dropped", "message": format!("{count} {what}") }));
        }
    }
    (out, notes)
}

#[cfg(test)]
mod tests;
