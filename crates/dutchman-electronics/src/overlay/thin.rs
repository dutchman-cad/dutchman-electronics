//! Thinning an overlay's outlines before they are written: a point whose removal moves its ring by
//! at most [`THIN`] is left out (Douglas–Peucker on each closed ring).
//!
//! Unions of letter strokes and arcs leave runs of almost-collinear points: on `all-features` the
//! silkscreen's median edge was 0.006 mm long. Every point costs Dutchman a side face when it
//! extrudes the area, so the silkscreen alone took about half the time of a conversion (#126).
//!
//! An area smaller than [`MIN_AREA`] is left out: the overlays' unions and differences leave a
//! few slivers with no surface at all (six in `all-features`'s top copper), which Dutchman would
//! refuse as areas whose outer loop has no area or crosses itself.
//!
//! A thinned area is kept only when it is still a valid polygon (no ring crossing itself or
//! another, every hole inside the outer ring) and touches no other area of the same overlay;
//! otherwise the area keeps its original outline. Rings keep at least three points.
use geo::{
    Area, BoundingRect, Coord, Intersects, LineString, MultiPolygon, Polygon, Rect, Validation,
};

/// The most a point's removal may move an outline, in mm.
pub const THIN: f64 = 0.005;
/// Areas smaller than this, in mm², are left out (about 0.03 x 0.03 mm, far below anything a
/// board maker can make).
pub const MIN_AREA: f64 = 0.001;

/// The distance from `p` to the segment `a`–`b`.
fn distance(p: Coord, a: Coord, b: Coord) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
}

/// Which points of the open polyline `pts` Douglas–Peucker keeps at `tolerance` (the two ends
/// always).
fn keep(pts: &[Coord], tolerance: f64) -> Vec<bool> {
    let mut kept = vec![false; pts.len()];
    let Some(last) = pts.len().checked_sub(1) else {
        return kept;
    };
    if let Some(k) = kept.first_mut() {
        *k = true;
    }
    if let Some(k) = kept.get_mut(last) {
        *k = true;
    }
    let mut stack = vec![(0usize, last)];
    while let Some((from, to)) = stack.pop() {
        let (Some(&a), Some(&b)) = (pts.get(from), pts.get(to)) else {
            continue;
        };
        let mut far = (0.0, None);
        for (i, &p) in pts.iter().enumerate().take(to).skip(from.saturating_add(1)) {
            let d = distance(p, a, b);
            if d > far.0 {
                far = (d, Some(i));
            }
        }
        if let (d, Some(i)) = far {
            if d > tolerance {
                if let Some(k) = kept.get_mut(i) {
                    *k = true;
                }
                stack.push((from, i));
                stack.push((i, to));
            }
        }
    }
    kept
}

/// A closed ring thinned at `tolerance`, or `None` when fewer than three points would be left.
/// The ring is cut at its first point and at the point farthest from it, so both halves are open
/// polylines with fixed ends.
fn ring(l: &LineString, tolerance: f64) -> Option<LineString> {
    let mut pts: Vec<Coord> = l.0.clone();
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    if pts.len() < 4 {
        return None;
    }
    let first = *pts.first()?;
    let far = pts
        .iter()
        .enumerate()
        .max_by(|a, b| {
            let da = (a.1.x - first.x).hypot(a.1.y - first.y);
            let db = (b.1.x - first.x).hypot(b.1.y - first.y);
            da.total_cmp(&db)
        })
        .map(|(i, _)| i)?;
    let mut closed = pts.clone();
    closed.push(first);
    let (one, two) = (closed.get(..=far)?, closed.get(far..)?);
    let mut out: Vec<Coord> = one
        .iter()
        .zip(keep(one, tolerance))
        .filter_map(|(p, k)| k.then_some(*p))
        .collect();
    out.pop();
    out.extend(
        two.iter()
            .zip(keep(two, tolerance))
            .filter_map(|(p, k)| k.then_some(*p)),
    );
    // `out` ends with the first point again: three distinct points and the closing one.
    (out.len() >= 4).then(|| LineString::new(out))
}

/// One area thinned, or `None` when it would not stay a valid polygon, when nothing could be
/// left out, or when it has more than [`MAX_HOLES`] holes (checking it would take too long: the
/// validity check compares the holes pairwise).
fn polygon(p: &Polygon, tolerance: f64) -> Option<Polygon> {
    if p.interiors().len() > MAX_HOLES {
        return None;
    }
    let exterior = ring(p.exterior(), tolerance)?;
    let interiors = p
        .interiors()
        .iter()
        .map(|h| ring(h, tolerance))
        .collect::<Option<Vec<_>>>()?;
    let points = |q: &Polygon| {
        q.interiors()
            .iter()
            .fold(q.exterior().0.len(), |n, h| n.saturating_add(h.0.len()))
    };
    let thinned = Polygon::new(exterior, interiors);
    (points(&thinned) < points(p) && thinned.is_valid()).then_some(thinned)
}

/// The most holes an area may have and still be thinned.
const MAX_HOLES: usize = 1_000;

/// `r` grown by `by` on every side.
fn grown(r: Rect, by: f64) -> Rect {
    Rect::new(
        Coord {
            x: r.min().x - by,
            y: r.min().y - by,
        },
        Coord {
            x: r.max().x + by,
            y: r.max().y + by,
        },
    )
}

/// The areas of `shape` thinned at `tolerance`, and how many areas under [`MIN_AREA`] were left
/// out; each area that would turn invalid, or would touch or overlap another area, keeps its
/// original outline.
pub fn thin(shape: &MultiPolygon, tolerance: f64) -> (MultiPolygon, usize) {
    let kept: Vec<Polygon> = shape
        .iter()
        .filter(|p| p.unsigned_area() >= MIN_AREA)
        .cloned()
        .collect();
    let slivers = shape.0.len().saturating_sub(kept.len());
    let shape = MultiPolygon::new(kept);
    let mut out: Vec<Polygon> = shape
        .iter()
        .map(|p| polygon(p, tolerance).unwrap_or_else(|| p.clone()))
        .collect();
    let boxes: Vec<Option<Rect>> = shape
        .iter()
        .map(|p| p.bounding_rect().map(|r| grown(r, 2.0 * tolerance)))
        .collect();
    // The pairs of areas whose grown boxes meet, found by sweeping along x: only these can come
    // to touch. An area without a box is paired with every other.
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    let left = |i: &usize| {
        boxes
            .get(*i)
            .copied()
            .flatten()
            .map_or(f64::NEG_INFINITY, |r| r.min().x)
    };
    order.sort_by(|a, b| left(a).total_cmp(&left(b)));
    let mut pairs = Vec::new();
    for (k, &i) in order.iter().enumerate() {
        let a = boxes.get(i).copied().flatten();
        for &j in order.iter().skip(k.saturating_add(1)) {
            let b = boxes.get(j).copied().flatten();
            match (a, b) {
                (Some(x), Some(y)) => {
                    if y.min().x > x.max().x {
                        break;
                    }
                    if x.intersects(&y) {
                        pairs.push((i, j));
                    }
                }
                _ => pairs.push((i, j)),
            }
        }
    }
    // Restoring an area can make it touch a thinned neighbour, so repeat until nothing changes;
    // each pass restores at least one area, and restored areas never touch each other.
    loop {
        let mut restore = Vec::new();
        for &(i, j) in &pairs {
            if let (Some(a), Some(b)) = (out.get(i), out.get(j)) {
                if a.intersects(b) {
                    restore.extend([i, j]);
                }
            }
        }
        let mut changed = false;
        for i in restore {
            if let (Some(o), Some(original)) = (out.get_mut(i), shape.0.get(i)) {
                if o != original {
                    *o = original.clone();
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    (MultiPolygon::new(out), slivers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Area;

    fn c(x: f64, y: f64) -> Coord {
        Coord { x, y }
    }

    fn closed(points: &[(f64, f64)]) -> LineString {
        let mut v: Vec<Coord> = points.iter().map(|&(x, y)| c(x, y)).collect();
        if let Some(&first) = v.first() {
            v.push(first);
        }
        LineString::new(v)
    }

    /// A 10 x 1 rectangle whose long edges carry a point every 0.01 mm, each off the line by
    /// `wobble`.
    fn wobbly(x0: f64, wobble: f64) -> Polygon {
        let mut pts = Vec::new();
        for i in 0..=1000 {
            let x = x0 + f64::from(i) * 0.01;
            pts.push((x, if i % 2 == 0 { 0.0 } else { wobble }));
        }
        for i in (0..=1000).rev() {
            let x = x0 + f64::from(i) * 0.01;
            pts.push((x, 1.0 + if i % 2 == 0 { 0.0 } else { wobble }));
        }
        Polygon::new(closed(&pts), vec![])
    }

    #[test]
    fn points_within_the_tolerance_are_left_out() {
        let shape = MultiPolygon::new(vec![wobbly(0.0, 0.002)]);
        let (out, _) = thin(&shape, THIN);
        let [p] = out.0.as_slice() else {
            panic!("{out:?}");
        };
        assert!(p.exterior().0.len() <= 6, "{}", p.exterior().0.len());
        assert!(p.is_valid());
        assert!((p.unsigned_area() - shape.unsigned_area()).abs() < 10.0 * 2.0 * THIN);
    }

    #[test]
    fn points_beyond_the_tolerance_are_kept() {
        let shape = MultiPolygon::new(vec![wobbly(0.0, 0.02)]);
        let (out, _) = thin(&shape, THIN);
        assert_eq!(out, shape);
    }

    #[test]
    fn a_triangle_is_kept_as_it_is() {
        let shape = MultiPolygon::new(vec![Polygon::new(
            closed(&[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]),
            vec![],
        )]);
        assert_eq!(thin(&shape, THIN), (shape, 0));
    }

    #[test]
    fn slivers_without_surface_are_left_out_and_counted() {
        // A 1 x 1 square, and a spike of no area: 0 to 2 and back along the same line.
        let square = Polygon::new(
            closed(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]),
            vec![],
        );
        let spike = Polygon::new(closed(&[(3.0, 0.0), (5.0, 0.0), (4.0, 0.0)]), vec![]);
        let (out, slivers) = thin(&MultiPolygon::new(vec![square.clone(), spike]), THIN);
        assert_eq!((out, slivers), (MultiPolygon::new(vec![square]), 1));
    }

    #[test]
    fn areas_that_would_touch_keep_their_outlines() {
        // Two rectangles 0.004 apart; thinning would let each move up to THIN towards the other,
        // so a wobble pointing into the gap would make them touch. Both keep their outlines.
        let a = Polygon::new(
            closed(&[
                (0.0, 0.0),
                (1.0, 0.0),
                (1.0, 0.5),
                (1.003, 0.5005),
                (1.0, 0.501),
                (1.0, 1.0),
                (0.0, 1.0),
            ]),
            vec![],
        );
        let b = Polygon::new(
            closed(&[(1.004, 0.0), (2.0, 0.0), (2.0, 1.0), (1.004, 1.0)]),
            vec![],
        );
        let shape = MultiPolygon::new(vec![a, b]);
        let (out, _) = thin(&shape, THIN);
        assert!(!out.0[0].intersects(&out.0[1]));
        assert!(out.is_valid(), "{:?}", out.validation_errors());
    }
}
