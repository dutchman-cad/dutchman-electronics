//! Copper, silkscreen and solder mask as thin overlays for the parts package (`overlays` in
//! `dutchman.parts/1`, agreed on dutchman-cad/dutchman#1005).
//!
//! Dutchman only draws polygons, so all the geometry happens here: every item becomes a polygon
//! (a track a stroke with round ends, a pad its shape, a via its ring), the polygons of one
//! overlay are united, every drill is cut out, and masked copper stops where a pad begins. Arcs
//! are polylines within [`CHORD`] mm. Up to eight overlays come out, for each side: copper under
//! the mask (`masked`), bare pad copper (`bare`), silkscreen, and the solder mask itself (the
//! board face less the bare pads, the masked copper and every drill). Each carries its color from
//! [`crate::colors`] and a `thickness`: every area is extruded from its `z` away from the board.
//! Copper starts at the face and is [`COPPER_THICKNESS`] thick, the mask starts there too and is
//! [`MASK_THICKNESS`] thick (so copper stands proud of it), silkscreen starts on top of the
//! mask and is [`SILK_THICKNESS`] thick, so its top clears the mask and the copper. The bottom
//! side mirrors the top. Past [`MAX_SOLID_FACES`] the largest overlays are drawn flat, and a note
//! says which.
//!
//! Copper on inner layers lies inside the board and is left out by default. With [`Copper::All`]
//! each inner copper layer whose place in the stack is known gets an overlay of its own (side
//! `inner`): its tracks, arcs, pours and texts, every via and every drilled pad through it, with
//! every drill cut out, at its height in the stack and its own copper thickness, extruded upward
//! from its lower face, in plain copper colour and with no mask.
//!
//! This module knows the neutral board model only, nothing about any input format.

use std::f64::consts::PI;
use std::panic::{catch_unwind, AssertUnwindSafe};

use board_model::{
    Board, Contour, CopperItem, EmbeddedFont, Graphic, LayerKind, PadShape, Point, Segment, Side,
    TextFont, DEFAULT_SOLDER_MASK_EXPANSION,
};
use geo::orient::{Direction, Orient};
use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon};
use serde_json::{json, Value};

use crate::colors::{
    hex, masked_copper, pad_look, Finish, COPPER_COLOR, COPPER_FINISH, COPPER_MASKED_COLOR,
    MASK_COLOR, MASK_FINISH, MASK_OPACITY, SILKSCREEN_COLOR, SILKSCREEN_FINISH,
};

/// Which copper layers get overlays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Copper {
    /// The top and bottom copper only: inner copper lies inside the board.
    #[default]
    Outer,
    /// Every copper layer whose place in the stack is known, inner ones too.
    All,
}

/// The most an arc's polyline strays from the true arc, in mm.
pub const CHORD: f64 = 0.02;
/// How thick copper is, in mm (35 um, 1 oz): bare pads and copper under the mask are each
/// extruded this far from the board face, away from the board.
pub const COPPER_THICKNESS: f64 = 0.035;
/// How thick the solder mask is, in mm (20 um). It fills only between the copper, so copper
/// stands [`COPPER_THICKNESS`] less this proud of it.
pub const MASK_THICKNESS: f64 = 0.020;
/// How thick the silkscreen solid is, in mm. Real ink is about 15 um, but SolidCraft 0.49.1 hides
/// silkscreen in oblique views (isometric and the like; the top view shows it) wherever it lies
/// over the mask unless its top stands about 0.1 mm or more above the mask's top (a lead of
/// 0.075 mm still vanished, 0.115 mm showed). The ink starts on the mask ([`SILK_LIFT`]), so
/// this is also how far its top stands above the mask.
pub const SILK_THICKNESS: f64 = 0.13;
/// How far silkscreen starts off the board face, in mm: on the top of the solder mask
/// ([`MASK_THICKNESS`]), with no gap under it. Over copper, which stands higher than the mask,
/// it starts inside the copper (its top is still above the copper's), so it is never in the air.
pub const SILK_LIFT: f64 = MASK_THICKNESS;
/// How far a flat sheet of copper under the mask (no `thickness`, see [`MAX_SOLID_FACES`]) is
/// drawn off the board face, in mm, away from the board, so it does not share the face's plane.
pub const FLAT_MASKED_COPPER_LIFT: f64 = 0.007;
/// The same for a flat solder mask sheet, in mm.
pub const FLAT_MASK_LIFT: f64 = 0.005;
/// The same for a flat sheet of bare pads, in mm.
pub const FLAT_PADS_LIFT: f64 = 0.001;
/// The same for a flat silkscreen sheet, in mm.
pub const FLAT_SILK_LIFT: f64 = 0.01;
/// The most faces the solids of one board's overlays may have together, as estimated by
/// [`faces`]. Dutchman refuses more, so the largest overlays are sent flat (without `thickness`)
/// until the rest fit.
pub const MAX_SOLID_FACES: usize = 250_000;
/// The most points one arc is drawn with: enough to hold [`CHORD`] on a whole circle as big as
/// [`MAX_COORDINATE`] allows.
const MAX_ARC_POINTS: usize = 4096;
/// The farthest from the board origin, in mm, an overlay point may be (25 m, the reader's limit).
const MAX_COORDINATE: f64 = 25_000.0;
/// The most copper and silkscreen items drawn; the rest are a Dropped note.
const MAX_ITEMS: usize = 200_000;
/// The most holes cut from one copper region; the rest are a Dropped note. Each hole is a polygon
/// that is united with the others before it is cut, so a hostile region of millions of holes
/// must not get that far.
const MAX_REGION_HOLES: usize = 10_000;
/// The most hole vertices cut from copper regions on one board, summed over all regions; a
/// region that would go past it is drawn solid and counted in a note.
const MAX_HOLE_VERTICES: usize = 200_000;
/// The most pen segments (or, for a text drawn with its font's outlines, flattened outline
/// points) drawn for all texts of a board together. Each segment becomes a polygon that is
/// united with the rest, so without a cap a hostile board of many long texts could ask
/// for billions of them. Texts past the budget are counted in a Dropped note.
const MAX_TEXT_SEGMENTS: usize = 2_000_000;
/// The most TrueType font names listed one by one in notes; the rest are counted in one note.
const MAX_FONT_NOTES: usize = 20;
/// The most a board's solder mask expansion may be, in mm; a larger value is taken as a mistake and
/// the default is used.
const MAX_MASK_EXPANSION: f64 = 2.0;
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
/// arc narrower than its stroke (the band would reach the center) is drawn as round-ended
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
        PadShape::RoundedRectangle { radius_ratio } => {
            let r = radius_ratio.clamp(0.0, 0.5) * size_x.min(size_y);
            if r.is_nan() || r <= CHORD / 10.0 {
                return pad(at, PadShape::Rectangle, size_x, size_y, rotation_deg);
            }
            // The four corner arcs, counter-clockwise from the bottom left; the ring closes with
            // straight edges between them.
            let mut ring = Vec::new();
            for (sx, sy, start) in [
                (-1.0, -1.0, 180.0),
                (1.0, -1.0, 270.0),
                (1.0, 1.0, 0.0),
                (-1.0, 1.0, 90.0),
            ] {
                let center = turned(at, sx * (hx - r), sy * (hy - r), rotation_deg);
                arc_points(center, r, start + rotation_deg, 90.0, &mut ring);
            }
            polygon(ring)
        }
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

/// A contour grown outwards by `by`: the contour itself plus a round-ended stroke `2 * by` wide
/// along every edge, whose union is the contour offset by `by` (to within [`CHORD`]).
fn grown(c: &Contour, by: f64) -> Vec<Polygon> {
    let mut out: Vec<Polygon> = contour(c).into_iter().collect();
    if !(by.is_finite() && by > 0.0) {
        return out;
    }
    for segment in &c.segments {
        match *segment {
            Segment::Line { from, to } => out.extend(stroke(from, to, 2.0 * by)),
            Segment::Arc {
                center,
                radius,
                start_deg,
                end_deg,
                clockwise,
            } => {
                let (a, b) = if clockwise {
                    (end_deg, start_deg)
                } else {
                    (start_deg, end_deg)
                };
                if segment.sweep_deg().is_some_and(|s| s.abs() >= 360.0) {
                    out.extend(circle(center, radius + by));
                } else {
                    out.extend(arc_stroke(center, radius, a, b, 2.0 * by));
                }
            }
        }
    }
    out
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

/// Like [`contour`], but the polygon always covers the true region the contour encloses: every
/// arc bulging out of the region is drawn as its circumscribed polygon, every arc cutting into it
/// as its inscribed one (the existing chords).
///
/// The solder mask face needs this. The board body is exact (true arcs), so a face drawn with
/// inscribed chords falls short of a convex corner by up to [`CHORD`] and a hairline of the body
/// shows outside the mask; for a cutout, the cut polygon must hold the whole true hole, or the
/// mask would cover its rim.
///
/// The circumscribed polygon of an arc of step angle `d` (the angle one chord spans) touches the
/// arc at the chord points `k * d` (both ends included, so it joins its neighbors exactly) and
/// has its corners where two neighboring tangents meet: at the middle angles `(k + 1/2) * d`, at
/// radius `r / cos(d / 2)`. Each edge lies on a tangent of the circle, so the polygon holds the
/// arc and strays at most `r * (1 / cos(d / 2) - 1)` from it. [`arc_points`] draws with
/// `d <= 360 / MIN_CIRCLE_SIDES` = 11.25 degrees (and, once a chord sags less than [`CHORD`],
/// `r * (1 - cos(d / 2)) <= CHORD`), so the stray is `<= CHORD / cos(5.625 deg)` < 1.005 * [`CHORD`]:
/// 0.0201 mm.
fn contour_covering(c: &Contour) -> Option<Polygon> {
    // A contour with a bad radius is not drawn at all.
    contour(c)?;
    // `contour` returns its ring counter-clockwise whatever the contour's own winding, so measure
    // the contour's own turn, on its chord points, to know which side of each arc the region is.
    let mut own = Vec::new();
    for segment in &c.segments {
        match *segment {
            Segment::Line { from, .. } => own.push(coord(from)),
            Segment::Arc {
                center,
                radius,
                start_deg,
                ..
            } => arc_points(
                center,
                radius,
                start_deg,
                segment.sweep_deg().unwrap_or(360.0),
                &mut own,
            ),
        }
    }
    let own_ccw = own
        .iter()
        .zip(own.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum::<f64>()
        >= 0.0;
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
                let sweep_deg = segment.sweep_deg().unwrap_or(360.0);
                // The center is on the left of travel for a counter-clockwise arc, which is the
                // region's side when the contour winds counter-clockwise.
                if (sweep_deg > 0.0) == own_ccw {
                    arc_points_covering(center, radius, start_deg, sweep_deg, &mut ring);
                } else {
                    arc_points(center, radius, start_deg, sweep_deg, &mut ring);
                }
            }
        }
    }
    polygon(ring)
}

/// The circumscribed counterpart of [`arc_points`]: same number of steps, see [`contour_covering`].
fn arc_points_covering(
    center: Point,
    radius: f64,
    start_deg: f64,
    sweep_deg: f64,
    out: &mut Vec<Coord>,
) {
    let mut exact = Vec::new();
    arc_points(center, radius, start_deg, sweep_deg, &mut exact);
    let (Some(&first), Some(&last)) = (exact.first(), exact.last()) else {
        return;
    };
    let steps = exact.len() - 1;
    let d = sweep_deg / steps as f64;
    let reach = radius / (d.to_radians() / 2.0).cos();
    out.push(first);
    for k in 0..steps {
        let a = (start_deg + d * (k as f64 + 0.5)).to_radians();
        out.push(Coord {
            x: center.x + reach * a.cos(),
            y: center.y + reach * a.sin(),
        });
    }
    out.push(last);
}

/// What went wrong drawing the holes of copper regions, for the notes.
#[derive(Default)]
struct HoleCounts {
    /// Holes that could not be drawn (or whose cut failed) and were left filled.
    undrawn: usize,
    /// Holes past [`MAX_REGION_HOLES`] in a region.
    over: usize,
    /// Regions drawn solid because the board's hole budget ([`MAX_HOLE_VERTICES`]) was used up.
    solid: usize,
    /// Regions the holes removed entirely: a legitimate empty result, not an unusable item.
    emptied: usize,
}

/// A copper region: its outline less its holes. A hole that cannot be drawn is counted in
/// `counts.undrawn` and left filled; holes past [`MAX_REGION_HOLES`] are counted in
/// `counts.over`; when the holes' vertices would overspend `budget` (shared by all regions) the
/// region is drawn solid and counted in `counts.solid`. Holes may wind either way, and the union
/// of them is cut from the outline, so a hole that strays outside it or overlaps another cannot
/// break the shape. The cut runs under `catch_unwind`: if it panics the region is drawn solid.
/// Empty when the outline cannot be drawn, or the holes leave nothing.
fn region(
    outline: &Contour,
    holes: &[Contour],
    budget: &mut usize,
    counts: &mut HoleCounts,
) -> Vec<Polygon> {
    let Some(outer) = contour(outline) else {
        return Vec::new();
    };
    if holes.is_empty() {
        return vec![outer];
    }
    let wanted: usize = holes
        .iter()
        .take(MAX_REGION_HOLES)
        .map(|h| h.segments.len())
        .fold(0, usize::saturating_add);
    if wanted > *budget {
        counts.solid = counts.solid.saturating_add(1);
        return vec![outer];
    }
    *budget -= wanted;
    counts.over = counts
        .over
        .saturating_add(holes.len().saturating_sub(MAX_REGION_HOLES));
    let mut cuts = Vec::new();
    for h in holes.iter().take(MAX_REGION_HOLES) {
        match contour(h) {
            Some(h) => cuts.push(h),
            None => counts.undrawn = counts.undrawn.saturating_add(1),
        }
    }
    if cuts.is_empty() {
        return vec![outer];
    }
    let cut = catch_unwind(AssertUnwindSafe(|| {
        outer
            .difference(&geo::unary_union(&cuts))
            .0
            .into_iter()
            .collect::<Vec<Polygon>>()
    }));
    match cut {
        Ok(shapes) => {
            if shapes.is_empty() {
                counts.emptied = counts.emptied.saturating_add(1);
            }
            shapes
        }
        Err(_) => {
            counts.undrawn = counts.undrawn.saturating_add(cuts.len());
            vec![outer]
        }
    }
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

/// What happened to a board's texts, for the notes.
#[derive(Default)]
struct TextCounts {
    drawn: usize,
    other_layers: usize,
    missing: usize,
    cut: usize,
    /// Texts left out because the board's texts had used up [`MAX_TEXT_SEGMENTS`].
    over_budget: usize,
    /// TrueType texts drawn as strokes, by font name: `false` when the board does not embed the
    /// font, `true` when its file cannot be used.
    truetype: std::collections::BTreeMap<(String, bool), usize>,
    /// Texts drawn with their font's own outlines.
    outlined: usize,
    /// Characters the embedded font has no glyph for.
    no_glyph: usize,
    /// Glyphs left out because the texts' budget was used up.
    glyphs_over_budget: usize,
    /// Glyphs that are too complex, out of range or could not be built.
    glyphs_failed: usize,
}

#[derive(Default)]
struct SideFaces {
    masked: Pile,
    bare: Pile,
    /// The mask openings of the bare pads: each pad grown by the mask expansion.
    opening: Pile,
    silk: Pile,
}

/// An estimate of the faces of the solid an overlay's `shape` extrudes to: for each area, a top
/// and a bottom, plus one side face for each vertex of its outer ring and its holes.
fn faces(shape: &MultiPolygon) -> usize {
    shape.iter().fold(0usize, |total, p| {
        let ring = |l: &LineString| l.0.len().saturating_sub(1);
        let sides = p
            .interiors()
            .iter()
            .map(ring)
            .fold(ring(p.exterior()), usize::saturating_add);
        total.saturating_add(sides).saturating_add(2)
    })
}

/// Draws the largest overlays flat (no `thickness`) until the solids of the rest have at most
/// `budget` faces together. Returns the names of the overlays drawn flat, in the order they were
/// dropped.
fn fit_faces(built: &mut [Built], budget: usize) -> Vec<String> {
    let mut sizes: Vec<(usize, usize)> = built
        .iter()
        .enumerate()
        .filter(|(_, b)| b.thickness.is_some())
        .map(|(i, b)| (faces(&b.shape), i))
        .collect();
    let mut total = sizes.iter().fold(0usize, |t, (n, _)| t.saturating_add(*n));
    // Largest first; the earlier overlay first among equals.
    sizes.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut flat = Vec::new();
    for (n, i) in sizes {
        if total <= budget {
            break;
        }
        if let Some(b) = built.get_mut(i) {
            b.thickness = None;
            b.z = b.flat_z;
            total = total.saturating_sub(n);
            flat.push(b.name.clone());
        }
    }
    flat
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

/// One overlay before it is written into the manifest.
struct Built {
    name: String,
    kind: &'static str,
    finish: Option<&'static str>,
    side: &'static str,
    z: f64,
    /// The height the overlay is drawn at when it is flat (see [`MAX_SOLID_FACES`]).
    flat_z: f64,
    /// How far the areas are extruded from `z`, away from the board; `None` for a flat sheet.
    thickness: Option<f64>,
    shape: MultiPolygon,
    color: String,
    finish_look: Finish,
    opacity: Option<f64>,
}

/// The board's outer contour as a polygon (`None` when it cannot be drawn), its drawable cutouts
/// gathered (united when the mask is cut; at most [`MAX_ITEMS`]), how many cutouts could not be
/// drawn, and how many were past the cap.
fn mask_outline(board: &Board) -> (Option<MultiPolygon>, Pile, usize, usize) {
    let outer = contour_covering(&board.outline.outer);
    let mut bad = 0usize;
    let mut cuts = Pile::default();
    for c in board.outline.cutouts.iter().take(MAX_ITEMS) {
        cuts.add(contour_covering(c), &mut bad);
    }
    let outline = outer.map(|p| MultiPolygon::new(vec![p]));
    let over = board.outline.cutouts.len().saturating_sub(MAX_ITEMS);
    (outline, cuts, bad, over)
}

/// The overlays of a board whose substrate is `thickness` thick (its top face at z = thickness,
/// its bottom at z = 0, in the frame of the part named `board_part`), and notes on what was left
/// out. `fonts` are the TrueType files the board embeds, which texts in those fonts are drawn
/// with. `copper` says whether inner copper layers get overlays too.
pub fn overlays(
    board: &Board,
    fonts: &[EmbeddedFont],
    thickness: f64,
    board_part: &str,
    copper: Copper,
) -> (Vec<Value>, Vec<Value>) {
    overlays_within(
        board,
        fonts,
        thickness,
        board_part,
        copper,
        MAX_TEXT_SEGMENTS,
        MAX_HOLE_VERTICES,
        MAX_SOLID_FACES,
    )
}

/// [`overlays`] with `text_budget` as the most text segments and outline points drawn (a
/// parameter so a test can use a small one), `hole_budget` as the most hole vertices cut from
/// copper regions and `face_budget` as the most faces the overlays' solids have together.
#[allow(clippy::too_many_arguments)]
fn overlays_within(
    board: &Board,
    fonts: &[EmbeddedFont],
    thickness: f64,
    board_part: &str,
    copper: Copper,
    text_budget: usize,
    hole_budget: usize,
    face_budget: usize,
) -> (Vec<Value>, Vec<Value>) {
    let top = SideFaces::default();
    let bottom = SideFaces::default();
    // The drills of pads cut copper, mask and silkscreen. Vias are drawn tented: most boards
    // cover their vias with mask, so a via is a solid copper disc under its mask (a small raised
    // dot), not a black hole, and its drill cuts only the silkscreen, which is not printed over
    // a via. The board part and the plating keep the real hole. When the board file says
    // whether a via is tented, that flag can replace this rule.
    let mut holes = Pile::default();
    let mut via_holes = Pile::default();
    let (mut inner, mut unusable, mut other_graphics) = (0usize, 0usize, 0usize);
    let mut hole_counts = HoleCounts::default();
    let mut hole_budget = hole_budget;
    // Where a copper item's layer puts it.
    enum On {
        Face(Side),
        /// An inner copper layer whose place in the stack is known, by layer index.
        Inner(usize),
        /// A layer that stands for all copper (Multi-Layer), or inner copper with no place.
        Through,
        /// No such layer, or not a copper layer.
        Unusable,
    }
    let on = |i: usize| match board.layers.get(i) {
        Some(l) if l.kind == LayerKind::Copper => match (l.side, l.position) {
            (Some(side), _) => On::Face(side),
            (None, Some(_)) => On::Inner(i),
            (None, None) => On::Through,
        },
        _ => On::Unusable,
    };
    // The inner layers that get overlays (none unless asked for), and their piles.
    let inner_layers: Vec<usize> = if copper == Copper::All {
        (0..board.layers.len())
            .filter(|&i| matches!(on(i), On::Inner(_)))
            .collect()
    } else {
        Vec::new()
    };
    let mut inner_piles: Vec<Pile> = inner_layers.iter().map(|_| Pile::default()).collect();
    let mut faces = (top, bottom);
    // Real fabs open the mask a little wider than each pad. Vias are tented: no opening.
    let expansion = board
        .solder_mask_expansion
        .filter(|e| e.is_finite() && (0.0..=MAX_MASK_EXPANSION).contains(e))
        .unwrap_or(DEFAULT_SOLDER_MASK_EXPANSION);
    let shown = board.copper.len().min(MAX_ITEMS);
    let skipped = board
        .copper
        .len()
        .saturating_sub(MAX_ITEMS)
        .saturating_add(
            board
                .graphics
                .len()
                .saturating_sub(MAX_ITEMS.saturating_sub(shown)),
        )
        .saturating_add(board.texts.len().saturating_sub(MAX_ITEMS));
    for item in board.copper.iter().take(MAX_ITEMS) {
        let emptied_before = hole_counts.emptied;
        let mut opened: Vec<Polygon> = Vec::new();
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
                CopperItem::Region {
                    layer,
                    contour: c,
                    holes: cuts,
                } => (
                    *layer,
                    region(c, cuts, &mut hole_budget, &mut hole_counts),
                    false,
                    None,
                ),
                CopperItem::Via {
                    at,
                    diameter,
                    drill,
                } => {
                    match circle(*at, diameter / 2.0) {
                        Some(ring) => {
                            for pile in &mut inner_piles {
                                pile.polygons.push(ring.clone());
                            }
                            faces.0.masked.polygons.push(ring.clone());
                            faces.1.masked.polygons.push(ring);
                            via_holes.add(circle(*at, drill / 2.0), &mut unusable);
                        }
                        None => unusable = unusable.saturating_add(1),
                    }
                    continue;
                }
                CopperItem::PolygonPad {
                    layer,
                    at,
                    shapes: outlines,
                    drill,
                    ..
                } => {
                    opened = outlines.iter().flat_map(|c| grown(c, expansion)).collect();
                    (
                        *layer,
                        outlines.iter().filter_map(contour).collect(),
                        true,
                        drill.map(|d| (*at, d)),
                    )
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
                } => {
                    // The opening is the pad grown by `expansion` on every side, so a rounded
                    // corner's radius grows by `expansion` too (not in proportion to the size).
                    let open_shape = match *shape {
                        PadShape::RoundedRectangle { radius_ratio } => {
                            let short = size_x.min(*size_y);
                            let bigger = short + 2.0 * expansion;
                            PadShape::RoundedRectangle {
                                radius_ratio: (radius_ratio.clamp(0.0, 0.5) * short + expansion)
                                    / bigger,
                            }
                        }
                        other => other,
                    };
                    opened = pad(
                        *at,
                        open_shape,
                        *size_x + 2.0 * expansion,
                        *size_y + 2.0 * expansion,
                        *rotation_deg,
                    )
                    .into_iter()
                    .collect();
                    (
                        *layer,
                        pad(*at, *shape, *size_x, *size_y, *rotation_deg)
                            .into_iter()
                            .collect(),
                        true,
                        drill.map(|d| (*at, d)),
                    )
                }
            };
        if shapes.is_empty() {
            if hole_counts.emptied == emptied_before {
                unusable = unusable.saturating_add(1);
            }
            continue;
        }
        let sides: &[Side] = match (on(layer), bare && drill.is_some()) {
            (On::Face(Side::Top), _) => &[Side::Top],
            (On::Face(Side::Bottom), _) => &[Side::Bottom],
            // A drilled pad goes through every copper layer.
            (On::Through | On::Inner(_), true) => {
                for pile in &mut inner_piles {
                    pile.polygons.extend(shapes.iter().cloned());
                }
                &[Side::Top, Side::Bottom]
            }
            (On::Inner(i), false) => {
                match inner_layers
                    .iter()
                    .position(|&l| l == i)
                    .and_then(|k| inner_piles.get_mut(k))
                {
                    Some(pile) => pile.polygons.extend(shapes),
                    None => inner = inner.saturating_add(1),
                }
                continue;
            }
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
            if bare {
                // A pad whose opening cannot be drawn is opened as the pad itself.
                let opening = if opened.is_empty() { &shapes } else { &opened };
                f.opening.polygons.extend(opening.iter().cloned());
            }
        }
        if let Some((at, d)) = drill {
            holes.add(circle(at, d / 2.0), &mut unusable);
        }
    }
    let (mut top, mut bottom) = faces;
    // Texts on a face's copper or silkscreen, drawn as pen strokes.
    let mut texts = TextCounts::default();
    let mut budget = text_budget;
    for t in board.texts.iter().take(MAX_ITEMS) {
        let pile = match board.layers.get(t.layer).map(|x| (x.kind, x.side)) {
            Some((LayerKind::Silkscreen, Some(Side::Top))) => &mut top.silk,
            Some((LayerKind::Silkscreen, Some(Side::Bottom))) => &mut bottom.silk,
            Some((LayerKind::Copper, Some(Side::Top))) => &mut top.masked,
            Some((LayerKind::Copper, Some(Side::Bottom))) => &mut bottom.masked,
            Some((LayerKind::Copper, None)) => {
                match inner_layers
                    .iter()
                    .position(|&l| l == t.layer)
                    .and_then(|k| inner_piles.get_mut(k))
                {
                    Some(pile) => pile,
                    None => {
                        inner = inner.saturating_add(1);
                        continue;
                    }
                }
            }
            _ => {
                texts.other_layers = texts.other_layers.saturating_add(1);
                continue;
            }
        };
        if budget == 0 {
            texts.over_budget = texts.over_budget.saturating_add(1);
            continue;
        }
        // A TrueType text whose font the board embeds is drawn with the font's outlines; one
        // whose font is missing or unusable falls back to strokes.
        let mut unreadable = false;
        if let TextFont::TrueType { .. } = &t.font {
            if let Some(font) = fonts.iter().find(|f| f.serves(&t.font)) {
                match text::outline(t, &font.data, budget) {
                    Ok(Some(o)) => {
                        budget = budget.saturating_sub(o.points);
                        texts.cut = texts.cut.saturating_add(o.cut);
                        texts.no_glyph = texts.no_glyph.saturating_add(o.missing);
                        texts.glyphs_over_budget =
                            texts.glyphs_over_budget.saturating_add(o.over_budget);
                        texts.glyphs_failed = texts.glyphs_failed.saturating_add(o.failed);
                        pile.polygons.extend(o.areas);
                        texts.outlined = texts.outlined.saturating_add(1);
                        continue;
                    }
                    Ok(None) => {
                        unusable = unusable.saturating_add(1);
                        continue;
                    }
                    Err(_) => unreadable = true,
                }
            }
        }
        let Some(drawn) = text::draw(t) else {
            unusable = unusable.saturating_add(1);
            continue;
        };
        texts.missing = texts.missing.saturating_add(drawn.missing);
        texts.cut = texts.cut.saturating_add(drawn.cut);
        let (mut added, mut failed) = (0usize, 0usize);
        let mut segment = |a: Point, b: Point| match stroke(a, b, drawn.pen) {
            Some(p) => {
                pile.polygons.push(p);
                added = added.saturating_add(1);
            }
            None => failed = failed.saturating_add(1),
        };
        for s in &drawn.strokes {
            match s.as_slice() {
                [only] => segment(*only, *only),
                _ => {
                    for pair in s.windows(2) {
                        if let [a, b] = pair {
                            segment(*a, *b);
                        }
                    }
                }
            }
        }
        budget = budget.saturating_sub(added.saturating_add(failed));
        if added > 0 {
            texts.drawn = texts.drawn.saturating_add(1);
            if let TextFont::TrueType { name, .. } = &t.font {
                let n = texts
                    .truetype
                    .entry((name.clone(), unreadable))
                    .or_default();
                *n = n.saturating_add(1);
            }
        }
        if added == 0 || failed > 0 {
            unusable = unusable.saturating_add(1);
        }
    }
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
    // The board's face for the solder mask: its outer contour, arcs included, minus its cutouts.
    let (outline, cutouts, bad_cutouts, over_cutouts) = mask_outline(board);
    if bad_cutouts > 0 {
        notes.push(json!({
            "severity": "Dropped",
            "message": format!(
                "{bad_cutouts} cutout(s) could not be drawn and are not cut from the solder mask"
            ),
        }));
    }
    if over_cutouts > 0 {
        notes.push(json!({
            "severity": "Dropped",
            "message": format!(
                "{over_cutouts} cutout(s) past the first {MAX_ITEMS} are not cut from the solder \
                 mask"
            ),
        }));
    }
    // A board with no outline at all has nothing to draw (and nothing dropped); one with an
    // outline that cannot be drawn says so.
    if outline.is_none() && !board.outline.outer.segments.is_empty() {
        notes.push(json!({
            "severity": "Dropped",
            "message": "the solder mask overlays could not be drawn: the board outline cannot \
                        be drawn",
        }));
    }
    // A failure inside the polygon library must not take the whole package down with it.
    let built = catch_unwind(AssertUnwindSafe(|| {
        let drilled = holes.union();
        let via_drilled = via_holes.union();
        let face = outline.map(|o| o.difference(&cutouts.union()));
        let mut built = Vec::new();
        for (side, f, sign) in [("top", &top, 1.0), ("bottom", &bottom, -1.0)] {
            // The top side stacks up from the face at z = thickness, the bottom down from z = 0.
            let base = if sign > 0.0 { thickness } else { 0.0 };
            let at = |lift: f64| base + sign * lift;
            // The file's colors for this side, or the defaults.
            let (mask_file, silk_file, finish_file) = if sign > 0.0 {
                (
                    &board.appearance.top_solder_mask,
                    &board.appearance.top_silkscreen,
                    &board.appearance.top_finish,
                )
            } else {
                (
                    &board.appearance.bottom_solder_mask,
                    &board.appearance.bottom_silkscreen,
                    &board.appearance.bottom_finish,
                )
            };
            let mask_color = mask_file.as_ref().map_or(MASK_COLOR.to_string(), hex);
            let copper_color = mask_file
                .as_ref()
                .map_or(COPPER_MASKED_COLOR.to_string(), masked_copper);
            let silk_color = silk_file.as_ref().map_or(SILKSCREEN_COLOR.to_string(), hex);
            let (pads, _) = pad_look(finish_file.as_ref());
            let (bare_all, masked_all) = (f.bare.union(), f.masked.union());
            let bare = bare_all.difference(&drilled);
            let masked = masked_all.difference(&drilled).difference(&bare);
            let opening_all = f.opening.union();
            // Silkscreen is not printed over holes, via caps, or the mask openings (the bare
            // pads and the ring of laminate round them).
            let silk = f
                .silk
                .union()
                .difference(&drilled)
                .difference(&via_drilled)
                .difference(&opening_all)
                .difference(&bare);
            let plain = |name: String,
                         kind,
                         finish,
                         lift,
                         flat_lift,
                         height,
                         shape,
                         (color, finish_look): (String, Finish)| Built {
                name,
                kind,
                finish,
                side,
                z: at(lift),
                flat_z: at(flat_lift),
                thickness: Some(height),
                shape,
                color,
                finish_look,
                opacity: None,
            };
            // The mask fills only between the copper: not over the bare pads (grown by the mask
            // expansion), the masked copper or the drills, so copper stands proud of it. One cut from the united shapes (the
            // copper before its drills are taken out), so no sliver is left where edges meet.
            let mask = face.as_ref().map(|face| {
                face.difference(
                    &drilled
                        .union(&bare_all)
                        .union(&opening_all)
                        .union(&masked_all),
                )
            });
            built.push(plain(
                format!("Copper {side}"),
                "copper",
                Some("masked"),
                0.0,
                FLAT_MASKED_COPPER_LIFT,
                COPPER_THICKNESS,
                masked,
                (copper_color, MASK_FINISH),
            ));
            built.push(plain(
                format!("Copper {side} pads"),
                "copper",
                Some("bare"),
                0.0,
                FLAT_PADS_LIFT,
                COPPER_THICKNESS,
                bare,
                (pads.color.to_string(), pads.finish),
            ));
            built.push(plain(
                format!("Silkscreen {side}"),
                "silkscreen",
                None,
                SILK_LIFT,
                FLAT_SILK_LIFT,
                SILK_THICKNESS,
                silk,
                (silk_color, SILKSCREEN_FINISH),
            ));
            if let Some(mask) = mask {
                built.push(Built {
                    opacity: Some(MASK_OPACITY),
                    ..plain(
                        format!("Solder mask {side}"),
                        "mask",
                        None,
                        0.0,
                        FLAT_MASK_LIFT,
                        MASK_THICKNESS,
                        mask,
                        (mask_color, MASK_FINISH),
                    )
                });
            }
        }
        // Each inner layer at its height in the stack, drills and via holes cut out.
        for (&i, pile) in inner_layers.iter().zip(&inner_piles) {
            let (Some(layer), Some(place)) = (
                board.layers.get(i),
                board.layers.get(i).and_then(|l| l.position),
            ) else {
                continue;
            };
            let t = place.thickness.clamp(0.0, thickness);
            let z = (thickness - place.depth - t).clamp(0.0, (thickness - t).max(0.0));
            built.push(Built {
                name: format!("Copper {}", layer.name),
                kind: "copper",
                finish: None,
                side: "inner",
                z,
                flat_z: z + t / 2.0,
                thickness: Some(t),
                shape: pile.union().difference(&drilled).difference(&via_drilled),
                color: COPPER_COLOR.to_string(),
                finish_look: COPPER_FINISH,
                opacity: None,
            });
        }
        built
    }));
    match built {
        Ok(mut built) => {
            built.retain(|b| !b.shape.0.is_empty());
            for name in fit_faces(&mut built, face_budget) {
                notes.push(json!({
                    "severity": "Degraded",
                    "message": format!(
                        "{name} is drawn flat, without thickness: the solids of all overlays \
                         would pass {face_budget} faces"
                    ),
                }));
            }
            for b in built {
                let mut o = json!({
                    "name": b.name,
                    "part": board_part,
                    "kind": b.kind,
                    "side": b.side,
                    "z": (b.z * 1e6).round() / 1e6,
                    "color": b.color,
                    "metallic": b.finish_look.metallic,
                    "roughness": b.finish_look.roughness,
                    "areas": areas(&b.shape),
                });
                if let Some(map) = o.as_object_mut() {
                    if let Some(finish) = b.finish {
                        map.insert("finish".into(), json!(finish));
                    }
                    if let Some(height) = b.thickness {
                        map.insert("thickness".into(), json!((height * 1e6).round() / 1e6));
                    }
                    if let Some(opacity) = b.opacity {
                        map.insert("opacity".into(), json!(opacity));
                    }
                }
                out.push(o);
            }
        }
        Err(_) => notes.push(json!({
            "severity": "Dropped",
            "message": "the copper, silkscreen and solder mask overlays could not be drawn",
        })),
    }
    for (count, what) in [
        (
            skipped,
            "copper, silkscreen or text item(s) past the first 200000 were not drawn",
        ),
        (
            hole_counts.undrawn,
            "hole(s) of copper regions could not be drawn and were left filled",
        ),
        (
            hole_counts.over,
            "hole(s) past the first 10000 of a copper region were not cut from it",
        ),
        (
            hole_counts.solid,
            "copper region(s) are drawn solid: the board's holes went past 200000 vertices",
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
        (
            texts.other_layers,
            "text(s) on layers other than copper and silkscreen are not drawn yet",
        ),
        (
            texts.cut,
            "character(s) past the first 1000 of a text were not drawn",
        ),
        (
            texts.over_budget,
            "text(s) past the drawing limit were not drawn",
        ),
        (
            texts.glyphs_over_budget,
            "glyph(s) past the drawing limit were not drawn",
        ),
        (
            texts.glyphs_failed,
            "glyph(s) were too complex, too far out or could not be built and were not drawn",
        ),
    ] {
        if count > 0 {
            notes.push(json!({ "severity": "Dropped", "message": format!("{count} {what}") }));
        }
    }
    if texts.drawn > 0 {
        notes.push(json!({
            "severity": "Degraded",
            "message": format!(
                "{} text(s) are drawn in the Hershey Roman Simplex stroke font at their height \
                 and pen width, not in the board's own font, so letter shapes and spacing differ \
                 a little",
                texts.drawn
            ),
        }));
    }
    for ((font, unreadable), n) in texts.truetype.iter().take(MAX_FONT_NOTES) {
        let why = if *unreadable {
            "the board's copy of the font cannot be read"
        } else {
            "the board does not embed the font"
        };
        notes.push(json!({
            "severity": "Degraded",
            "message": format!(
                "{n} text(s) in the TrueType font \"{font}\" are drawn as single strokes, a \
                 tenth of their height wide, because {why}"
            ),
        }));
    }
    let more_fonts = texts.truetype.len().saturating_sub(MAX_FONT_NOTES);
    if more_fonts > 0 {
        notes.push(json!({
            "severity": "Degraded",
            "message": format!("and {more_fonts} more TrueType font(s) used by drawn texts"),
        }));
    }
    if texts.no_glyph > 0 {
        notes.push(json!({
            "severity": "Degraded",
            "message": format!(
                "{} character(s) the text's font has no glyph for are drawn as the font's \
                 missing-glyph shape, or as nothing if it has none",
                texts.no_glyph
            ),
        }));
    }
    if texts.missing > 0 {
        notes.push(json!({
            "severity": "Degraded",
            "message": format!(
                "{} character(s) the stroke font does not hold are drawn as \"?\"",
                texts.missing
            ),
        }));
    }
    (out, notes)
}

mod text;

#[cfg(test)]
mod tests;
