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
    Area, BoundingRect, Contains, Coord, Intersects, LineString, MultiPolygon, Polygon, Rect,
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
fn polygon_with_validity(
    p: &Polygon,
    tolerance: f64,
    validity: &mut super::validity::Validity,
) -> Option<Polygon> {
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
    (points(&thinned) < points(p) && validity.valid(&thinned)).then_some(thinned)
}

#[cfg(test)]
fn polygon(p: &Polygon, tolerance: f64) -> Option<Polygon> {
    polygon_with_validity(p, tolerance, &mut super::validity::Validity::default())
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

fn envelope(r: Rect) -> rstar::AABB<[f64; 2]> {
    rstar::AABB::from_corners([r.min().x, r.min().y], [r.max().x, r.max().y])
}

struct IndexedBox {
    bounds: rstar::AABB<[f64; 2]>,
    data: usize,
}

impl rstar::RTreeObject for IndexedBox {
    type Envelope = rstar::AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.bounds
    }
}

fn boxed(r: Rect, data: usize) -> IndexedBox {
    IndexedBox {
        bounds: envelope(r),
        data,
    }
}

/// We filter in both axes and stream candidate pairs, keeping temporary memory linear
/// even for nested polygons whose boxes all overlap.
fn candidate_index(boxes: &[Option<Rect>]) -> rstar::RTree<IndexedBox> {
    rstar::RTree::bulk_load(
        boxes
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.map(|r| boxed(r, i)))
            .collect(),
    )
}

fn visit_candidates(
    boxes: &[Option<Rect>],
    index: &rstar::RTree<IndexedBox>,
    mut visit: impl FnMut(usize, usize),
) {
    let missing: Vec<usize> = boxes
        .iter()
        .enumerate()
        .filter_map(|(i, b)| b.is_none().then_some(i))
        .collect();
    for (i, b) in boxes.iter().enumerate() {
        if let Some(b) = b {
            for k in index.locate_in_envelope_intersecting(&envelope(*b)) {
                if k.data > i {
                    visit(i, k.data);
                }
            }
            for &j in missing.iter().filter(|&&j| j > i) {
                visit(i, j);
            }
        } else {
            for j in i.saturating_add(1)..boxes.len() {
                visit(i, j);
            }
        }
    }
}

#[cfg(test)]
fn candidate_pairs(boxes: &[Option<Rect>]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    visit_candidates(boxes, &candidate_index(boxes), |i, j| pairs.push((i, j)));
    pairs
}

/// The areas of `shape` thinned at `tolerance`, and how many areas under [`MIN_AREA`] were left
/// out; each area that would turn invalid, or would touch or overlap another area, keeps its
/// original outline.
#[cfg(test)]
pub fn thin(shape: &MultiPolygon, tolerance: f64) -> (MultiPolygon, usize) {
    thin_keeping(shape, tolerance, &MultiPolygon::new(Vec::new()))
}

/// [`thin`], except that an area touching `keep` (letter outlines, which are followed to a
/// fraction of their height and must stay as fine as that) keeps its outline.
pub fn thin_keeping(
    shape: &MultiPolygon,
    tolerance: f64,
    keep: &MultiPolygon,
) -> (MultiPolygon, usize) {
    let keep_index = rstar::RTree::bulk_load(
        keep.iter()
            .enumerate()
            .filter_map(|(i, p)| p.bounding_rect().map(|b| boxed(b, i)))
            .collect(),
    );
    let fixed = |p: &Polygon| {
        p.bounding_rect().is_some_and(|b| {
            keep_index
                .locate_in_envelope_intersecting(&envelope(b))
                .any(|k| keep.0.get(k.data).is_some_and(|q| p.intersects(q)))
        })
    };
    let kept: Vec<Polygon> = shape
        .iter()
        .filter(|p| p.unsigned_area() >= MIN_AREA)
        .cloned()
        .collect();
    let slivers = shape.0.len().saturating_sub(kept.len());
    let shape = MultiPolygon::new(kept);
    let mut validity = super::validity::Validity::default();
    let mut out: Vec<Polygon> = shape
        .iter()
        .map(|p| {
            if fixed(p) {
                return p.clone();
            }
            polygon_with_validity(p, tolerance, &mut validity).unwrap_or_else(|| p.clone())
        })
        .collect();
    // A pair of unchanged originals cannot cause a restoration. We record this once,
    // avoiding repeated full-ring comparisons (or intersection tests through distant holes).
    let mut simplified: Vec<bool> = out.iter().zip(&shape).map(|(a, b)| a != b).collect();
    if !simplified.iter().any(|&changed| changed) {
        return (MultiPolygon::new(out), slivers);
    }
    let boxes: Vec<Option<Rect>> = shape
        .iter()
        .map(|p| p.bounding_rect().map(|r| grown(r, 2.0 * tolerance)))
        .collect();
    let index = candidate_index(&boxes);
    // A fixed polygon can surround many separate islands inside its holes. Its exterior
    // box reaches all of them, but a local hole query can prove they do not touch copper.
    let hole_indices: Vec<_> = shape
        .iter()
        .map(|p| {
            rstar::RTree::bulk_load(if p.interiors().len() > MAX_HOLES {
                p.interiors()
                    .iter()
                    .enumerate()
                    .filter_map(|(i, h)| h.bounding_rect().map(|r| boxed(r, i)))
                    .collect()
            } else {
                Vec::new()
            })
        })
        .collect();
    let inside_hole = |i: usize, other: &Polygon| {
        use rstar::Envelope;
        let (Some(index), Some(original), Some(bounds)) =
            (hole_indices.get(i), shape.0.get(i), other.bounding_rect())
        else {
            return false;
        };
        index
            .locate_in_envelope_intersecting(&envelope(bounds))
            .any(|entry| {
                if !entry.bounds.contains_envelope(&envelope(bounds)) {
                    return false;
                }
                let Some(hole) = original.interiors().get(entry.data) else {
                    return false;
                };
                !hole.intersects(other.exterior())
                    && Polygon::new(hole.clone(), vec![]).contains(other)
            })
    };
    // Restoring an area can make it touch a thinned neighbour, so repeat until nothing changes;
    // each pass restores at least one area, and restored areas never touch each other.
    loop {
        let mut restore = vec![false; out.len()];
        visit_candidates(&boxes, &index, |i, j| {
            if !simplified.get(i).copied().unwrap_or(false)
                && !simplified.get(j).copied().unwrap_or(false)
            {
                return;
            }
            if let (Some(a), Some(b)) = (out.get(i), out.get(j)) {
                if (!simplified.get(i).copied().unwrap_or(true) && inside_hole(i, b))
                    || (!simplified.get(j).copied().unwrap_or(true) && inside_hole(j, a))
                {
                    return;
                }
                if a.intersects(b) {
                    if let Some(flag) = restore.get_mut(i) {
                        *flag = true;
                    }
                    if let Some(flag) = restore.get_mut(j) {
                        *flag = true;
                    }
                }
            }
        });
        let mut changed = false;
        for (i, &restore) in restore.iter().enumerate() {
            if !restore {
                continue;
            }
            if let (Some(o), Some(original)) = (out.get_mut(i), shape.0.get(i)) {
                if o != original {
                    *o = original.clone();
                    if let Some(flag) = simplified.get_mut(i) {
                        *flag = false;
                    }
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
    use geo::{Area, Validation};

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
    fn an_area_touching_the_kept_shape_is_not_thinned_and_the_others_are() {
        // Two thinnable rectangles far apart; the second is also in `keep`.
        let (a, b) = (wobbly(0.0, 0.002), wobbly(50.0, 0.002));
        let shape = MultiPolygon::new(vec![a, b.clone()]);
        let (out, _) = thin_keeping(&shape, THIN, &MultiPolygon::new(vec![b.clone()]));
        assert_eq!(out.0.len(), 2);
        assert!(out.0.contains(&b), "the kept area keeps every point");
        assert!(
            out.0.iter().any(|p| p.exterior().0.len() <= 6),
            "the other is thinned"
        );
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

#[cfg(test)]
mod spatial_tests {
    use super::*;

    #[test]
    fn indexed_candidates_match_all_pairs_including_missing_boxes() {
        let mut boxes: Vec<Option<Rect>> = (0..1200)
            .map(|i| {
                Some(Rect::new(
                    (f64::from(i % 30), f64::from(i / 30)),
                    (f64::from(i % 30) + 1.0, f64::from(i / 30) + 0.75),
                ))
            })
            .collect();
        boxes.extend([
            None,
            Some(Rect::new((-100.0, -100.0), (100.0, 100.0))),
            None,
        ]);
        let mut expected = Vec::new();
        for (i, a) in boxes.iter().enumerate() {
            for (j, b) in boxes.iter().enumerate().skip(i + 1) {
                if a.zip(*b).is_none_or(|(a, b)| a.intersects(&b)) {
                    expected.push((i, j));
                }
            }
        }
        let mut actual = candidate_pairs(&boxes);
        actual.sort_unstable();
        assert_eq!(actual, expected);
    }

    #[test]
    fn many_protected_text_areas_keep_exact_outlines() {
        let areas: Vec<Polygon> = (0..1200)
            .map(|i| {
                let x = f64::from(i % 30) * 2.0;
                let y = f64::from(i / 30) * 2.0;
                Polygon::new(
                    LineString::from(vec![
                        (x, y),
                        (x + 0.5, y + 0.001),
                        (x + 1.0, y),
                        (x + 1.0, y + 1.0),
                        (x, y + 1.0),
                        (x, y),
                    ]),
                    vec![],
                )
            })
            .collect();
        let shape = MultiPolygon::new(areas);
        assert_eq!(thin_keeping(&shape, THIN, &shape), (shape, 0));
    }
}

#[cfg(test)]
mod restoration_tests {
    use super::*;
    #[test]
    fn a_simplified_island_stays_in_one_of_many_holes() {
        let rect = |x: f64, y: f64| {
            LineString::from(vec![
                (x, y),
                (x + 1.0, y),
                (x + 1.0, y + 1.0),
                (x, y + 1.0),
                (x, y),
            ])
        };
        let holes: Vec<_> = (0..1200)
            .map(|i| rect(f64::from(i % 40) * 2.0, f64::from(i / 40) * 2.0))
            .collect();
        let outer = Polygon::new(
            LineString::from(vec![
                (-1.0, -1.0),
                (100.0, -1.0),
                (100.0, 100.0),
                (-1.0, 100.0),
                (-1.0, -1.0),
            ]),
            holes,
        );
        let island = Polygon::new(
            LineString::from(vec![
                (0.2, 0.2),
                (0.8, 0.2),
                (0.8, 0.5),
                (0.798, 0.501),
                (0.8, 0.502),
                (0.8, 0.8),
                (0.2, 0.8),
                (0.2, 0.2),
            ]),
            vec![],
        );
        let before = island.exterior().0.len();
        let (out, slivers) = thin(&MultiPolygon::new(vec![outer.clone(), island]), THIN);
        assert_eq!(slivers, 0);
        assert_eq!(out.0.first(), Some(&outer));
        assert!(out.0.get(1).unwrap().exterior().0.len() < before);
    }

    #[test]
    fn a_simplified_island_crossing_a_hole_notch_is_restored() {
        let rect = |x: f64, y: f64| {
            LineString::from(vec![
                (x, y),
                (x + 1.0, y),
                (x + 1.0, y + 1.0),
                (x, y + 1.0),
                (x, y),
            ])
        };
        let mut holes: Vec<_> = (0..1200)
            .map(|i| rect(f64::from(i % 40) * 2.0, f64::from(i / 40) * 2.0))
            .collect();
        let notch = LineString::from(vec![
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 1.0),
            (0.51, 1.0),
            (0.5, 0.996),
            (0.49, 1.0),
            (0.0, 1.0),
            (0.0, 0.0),
        ]);
        *holes.first_mut().unwrap() = notch;
        let island = Polygon::new(
            LineString::from(vec![
                (0.0005, 0.0005),
                (0.9995, 0.0005),
                (0.9995, 0.9995),
                (0.51, 0.9995),
                (0.5, 0.995),
                (0.49, 0.9995),
                (0.0005, 0.9995),
                (0.0005, 0.0005),
            ]),
            vec![],
        );
        assert!(
            polygon(&island, THIN).is_some(),
            "the notch would be simplified without the collision check"
        );
        let outer = Polygon::new(
            LineString::from(vec![
                (-1.0, -1.0),
                (100.0, -1.0),
                (100.0, 100.0),
                (-1.0, 100.0),
                (-1.0, -1.0),
            ]),
            holes,
        );
        let original = MultiPolygon::new(vec![outer, island]);
        assert_eq!(thin(&original, THIN), (original, 0));
    }

    #[test]
    fn unchanged_islands_inside_many_holes_keep_every_coordinate() {
        let rect = |x: f64, y: f64, side: f64| {
            LineString::from(vec![
                (x, y),
                (x + side, y),
                (x + side, y + side),
                (x, y + side),
                (x, y),
            ])
        };
        let holes: Vec<_> = (0..1200)
            .map(|i| rect(f64::from(i % 40) * 2.0, f64::from(i / 40) * 2.0, 1.0))
            .collect();
        let mut pieces = vec![Polygon::new(rect(-1.0, -1.0, 100.0), holes)];
        pieces.extend((0..1200).map(|i| {
            Polygon::new(
                rect(
                    f64::from(i % 40) * 2.0 + 0.25,
                    f64::from(i / 40) * 2.0 + 0.25,
                    0.5,
                ),
                vec![],
            )
        }));
        let original = MultiPolygon::new(pieces);
        assert_eq!(thin(&original, THIN), (original, 0));
    }
}
