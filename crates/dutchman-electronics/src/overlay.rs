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
/// The most points one arc is drawn with: enough to hold [`CHORD`] on a whole circle as big as
/// [`MAX_COORDINATE`] allows.
const MAX_ARC_POINTS: usize = 4096;
/// The farthest from the board origin, in mm, an overlay point may be (25 m, the reader's limit).
const MAX_COORDINATE: f64 = 25_000.0;
/// The most copper and silkscreen items drawn; the rest are a Dropped note.
const MAX_ITEMS: usize = 200_000;
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

fn same(a: &Coord, b: &Coord) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

/// A polygon from a ring of points, always counter-clockwise: the union picks its fill rule from
/// the first ring's winding, so one clockwise ring would wipe out the others. `None` for a ring
/// that is too far out, not a number, or encloses no area.
fn polygon(mut ring: Vec<Coord>) -> Option<Polygon> {
    if ring
        .iter()
        .any(|c| !(c.x.abs() <= MAX_COORDINATE && c.y.abs() <= MAX_COORDINATE))
    {
        return None;
    }
    ring.dedup_by(|a, b| same(a, b));
    while ring.len() > 1
        && ring
            .first()
            .zip(ring.last())
            .is_some_and(|(a, b)| same(a, b))
    {
        ring.pop();
    }
    let n = ring.len();
    if n < 3 {
        return None;
    }
    let twice_area: f64 = ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum();
    if twice_area.abs() < 1e-12 {
        return None;
    }
    if twice_area < 0.0 {
        ring.reverse();
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

/// An arc stroke with round ends: the band between `radius - width / 2` and `radius + width /
/// 2`, with a half circle at each end. A whole circle is a ring (an outline with one hole). An
/// arc narrower than its stroke (the band would reach the centre) is drawn as round-ended
/// pieces along its polyline instead.
fn arc_stroke(
    center: Point,
    radius: f64,
    start_deg: f64,
    end_deg: f64,
    width: f64,
) -> Vec<Polygon> {
    if !(radius.is_finite() && radius > 0.0 && width.is_finite() && width > 0.0 && finite(center)) {
        return Vec::new();
    }
    let sweep = sweep(start_deg, end_deg);
    let (outer, inner) = (radius + width / 2.0, radius - width / 2.0);
    if inner <= CHORD {
        let mut points = Vec::new();
        arc_points(center, radius, start_deg, sweep, &mut points);
        return points
            .windows(2)
            .filter_map(|w| match w {
                [a, b] => stroke(Point { x: a.x, y: a.y }, Point { x: b.x, y: b.y }, width),
                _ => None,
            })
            .collect();
    }
    if sweep >= 360.0 {
        let (Some(o), Some(i)) = (circle(center, outer), circle(center, inner)) else {
            return Vec::new();
        };
        // The hole runs clockwise, against the outline, so the union leaves it open.
        let mut hole = i.exterior().clone();
        hole.0.reverse();
        return vec![Polygon::new(o.exterior().clone(), vec![hole])];
    }
    let end = start_deg + sweep;
    let at = |deg: f64| turned(center, radius, 0.0, deg);
    let mut ring = Vec::new();
    arc_points(center, outer, start_deg, sweep, &mut ring);
    arc_points(at(end), width / 2.0, end, 180.0, &mut ring);
    arc_points(center, inner, end, -sweep, &mut ring);
    arc_points(
        at(start_deg),
        width / 2.0,
        start_deg + 180.0,
        180.0,
        &mut ring,
    );
    polygon(ring).into_iter().collect()
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
                ..
            } => {
                if !(radius.is_finite() && radius > 0.0) {
                    return None;
                }
                arc_points(
                    center,
                    radius,
                    start_deg,
                    segment.sweep_deg().unwrap_or(360.0),
                    &mut ring,
                );
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
        pts.dedup();
        while pts.len() > 1 && pts.first() == pts.last() {
            pts.pop();
        }
        pts
    };
    shape
        .orient(Direction::Default)
        .iter()
        .filter_map(|p| {
            let outer = ring(p.exterior());
            if outer.len() < 3 {
                return None;
            }
            let holes: Vec<Vec<[f64; 2]>> = p
                .interiors()
                .iter()
                .map(ring)
                .filter(|h| h.len() >= 3)
                .collect();
            Some(json!({ "outer": outer, "holes": holes }))
        })
        .collect()
}

/// The overlays of a board whose substrate is `thickness` thick (its top face at z = thickness,
/// its bottom at z = 0, in the frame of the part named `board_part`), and notes on what was left
/// out.
pub fn overlays(board: &Board, thickness: f64, board_part: &str) -> (Vec<Value>, Vec<Value>) {
    let top = SideFaces::default();
    let bottom = SideFaces::default();
    let mut holes = Pile::default();
    let (mut inner, mut unusable, mut other_graphics) = (0usize, 0usize, 0usize);
    // Where a copper item's layer puts it.
    enum On {
        Face(Side),
        /// A copper layer that belongs to neither face: inner copper, or (for a drilled pad)
        /// both faces.
        Through,
        /// No such layer, or not a copper layer.
        Unusable,
    }
    let on = |i: usize| match board.layers.get(i) {
        Some(l) if l.kind == LayerKind::Copper => l.side.map_or(On::Through, On::Face),
        _ => On::Unusable,
    };
    let mut faces = (top, bottom);
    let shown = board.copper.len().min(MAX_ITEMS);
    let skipped = board.copper.len().saturating_sub(MAX_ITEMS).saturating_add(
        board
            .graphics
            .len()
            .saturating_sub(MAX_ITEMS.saturating_sub(shown)),
    );
    for item in board.copper.iter().take(MAX_ITEMS) {
        let (layer, shapes, bare, drill): (usize, Vec<Polygon>, bool, Option<(Point, f64)>) =
            match item {
                CopperItem::Track {
                    layer,
                    from,
                    to,
                    width,
                    ..
                } => (
                    *layer,
                    stroke(*from, *to, *width).into_iter().collect(),
                    false,
                    None,
                ),
                CopperItem::TrackArc {
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
                    false,
                    None,
                ),
                CopperItem::Region { layer, contour: c } => {
                    (*layer, contour(c).into_iter().collect(), false, None)
                }
                CopperItem::Via {
                    at,
                    diameter,
                    drill,
                } => {
                    match circle(*at, diameter / 2.0) {
                        Some(ring) => {
                            faces.0.masked.polygons.push(ring.clone());
                            faces.1.masked.polygons.push(ring);
                            holes.add(circle(*at, drill / 2.0), &mut unusable);
                        }
                        None => unusable = unusable.saturating_add(1),
                    }
                    continue;
                }
                CopperItem::Pad {
                    layer,
                    at,
                    shape,
                    size_x,
                    size_y,
                    rotation_deg,
                    drill,
                    ..
                } => (
                    *layer,
                    pad(*at, *shape, *size_x, *size_y, *rotation_deg)
                        .into_iter()
                        .collect(),
                    true,
                    drill.map(|d| (*at, d)),
                ),
            };
        if shapes.is_empty() {
            unusable = unusable.saturating_add(1);
            continue;
        }
        let sides: &[Side] = match (on(layer), bare && drill.is_some()) {
            (On::Face(Side::Top), _) => &[Side::Top],
            (On::Face(Side::Bottom), _) => &[Side::Bottom],
            // A drilled pad on a copper layer of neither face goes through: it is on both.
            (On::Through, true) => &[Side::Top, Side::Bottom],
            (On::Through, false) => {
                inner = inner.saturating_add(1);
                continue;
            }
            (On::Unusable, _) => {
                unusable = unusable.saturating_add(1);
                continue;
            }
        };
        for side in sides {
            let f = match side {
                Side::Top => &mut faces.0,
                Side::Bottom => &mut faces.1,
            };
            let target = if bare { &mut f.bare } else { &mut f.masked };
            target.polygons.extend(shapes.iter().cloned());
        }
        if let Some((at, d)) = drill {
            holes.add(circle(at, d / 2.0), &mut unusable);
        }
    }
    let (mut top, mut bottom) = faces;
    for g in board.graphics.iter().take(MAX_ITEMS.saturating_sub(shown)) {
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
        let pile = match board.layers.get(l).map(|x| (x.kind, x.side)) {
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
            // Silkscreen is not printed over holes or on bare pads.
            let silk = f.silk.union().difference(&drilled).difference(&bare);
            built.push((
                format!("Copper {side}"),
                "copper",
                Some("masked"),
                side,
                z_copper,
                masked,
            ));
            built.push((
                format!("Copper {side} pads"),
                "copper",
                Some("bare"),
                side,
                z_copper,
                bare,
            ));
            built.push((
                format!("Silkscreen {side}"),
                "silkscreen",
                None,
                side,
                z_silk,
                silk,
            ));
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
        (
            skipped,
            "copper or silkscreen item(s) past the first 200000 were not drawn",
        ),
        (
            inner,
            "copper item(s) on inner layers are left out, as they lie inside the board",
        ),
        (
            unusable,
            "copper or silkscreen item(s) have unusable sizes or layers and were left out",
        ),
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
