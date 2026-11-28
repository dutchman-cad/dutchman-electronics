//! Local boolean operations for large, already united collections of polygons.
//! We keep small collections on the original path, including their vertex ordering.
use geo::{BooleanOps, BoundingRect, MapCoords, MultiPolygon, Polygon, Rect};
use rstar::{RTree, RTreeObject, AABB};

const LOCAL_THRESHOLD: usize = 4096;

fn envelope(r: Rect) -> AABB<[f64; 2]> {
    AABB::from_corners([r.min().x, r.min().y], [r.max().x, r.max().y])
}

struct Entry {
    bounds: AABB<[f64; 2]>,
    index: usize,
}
impl RTreeObject for Entry {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.bounds
    }
}

/// A polygon index also indexes interior rings: a small clip need not visit remote holes.
struct Index<'a> {
    shape: &'a MultiPolygon,
    polygons: RTree<Entry>,
    holes: Vec<RTree<Entry>>,
}
impl<'a> Index<'a> {
    fn new(shape: &'a MultiPolygon) -> Self {
        Self {
            shape,
            polygons: RTree::bulk_load(
                shape
                    .iter()
                    .enumerate()
                    .filter_map(|(index, p)| {
                        p.bounding_rect().map(|r| Entry {
                            bounds: envelope(r),
                            index,
                        })
                    })
                    .collect(),
            ),
            holes: shape
                .iter()
                .map(|p| {
                    RTree::bulk_load(
                        p.interiors()
                            .iter()
                            .enumerate()
                            .filter_map(|(index, h)| {
                                h.bounding_rect().map(|r| Entry {
                                    bounds: envelope(r),
                                    index,
                                })
                            })
                            .collect(),
                    )
                })
                .collect(),
        }
    }

    fn nearby(&self, bounds: Rect, trim_holes: bool) -> MultiPolygon {
        let mut entries: Vec<_> = self
            .polygons
            .locate_in_envelope_intersecting(&envelope(bounds))
            .collect();
        entries.sort_unstable_by_key(|e| e.index);
        MultiPolygon::new(
            entries
                .into_iter()
                .filter_map(|e| {
                    let p = self.shape.0.get(e.index)?;
                    if !trim_holes || p.interiors().len() < LOCAL_THRESHOLD {
                        return Some(p.clone());
                    }
                    let index = self.holes.get(e.index)?;
                    let mut holes: Vec<_> = index
                        .locate_in_envelope_intersecting(&envelope(bounds))
                        .map(|e| e.index)
                        .collect();
                    holes.sort_unstable();
                    Some(Polygon::new(
                        p.exterior().clone(),
                        holes
                            .into_iter()
                            .filter_map(|i| p.interiors().get(i).cloned())
                            .collect(),
                    ))
                })
                .collect(),
        )
    }
}

fn complexity(shape: &MultiPolygon) -> usize {
    shape
        .iter()
        .fold(shape.0.len(), |n, p| n.saturating_add(p.interiors().len()))
}

/// Batch nearby pieces rather than invoking the boolean solver once per tiny via.
/// The batches partition polygons, not their geometry: no artificial tile edges are added.
fn batches(shape: &MultiPolygon) -> Vec<MultiPolygon> {
    if shape.0.len() < LOCAL_THRESHOLD {
        return vec![shape.clone()];
    }
    let mut cells = std::collections::BTreeMap::<(i64, i64), Vec<Polygon>>::new();
    for p in shape {
        let key = p.bounding_rect().map_or((0, 0), |r| {
            (
                (r.min().x / 32.0).floor() as i64,
                (r.min().y / 32.0).floor() as i64,
            )
        });
        cells.entry(key).or_default().push(p.clone());
    }
    cells
        .into_values()
        .flat_map(|v| {
            v.chunks(512)
                .map(|c| MultiPolygon::new(c.to_vec()))
                .collect::<Vec<_>>()
        })
        .collect()
}

// Strict containment in a small convex exterior lets us attach complete, disjoint holes
// directly. Near-touching rings and complex outlines always use the boolean solver.
fn contains_ring(outer: &Polygon, inner: &geo::LineString) -> bool {
    let ring = outer.exterior();
    if ring.0.len() < 4 || ring.0.len() > 64 || inner.0.len() < 4 || inner.0.len() > 64 {
        return false;
    }
    use geo::Area;
    let sign = outer.signed_area().signum();
    if sign == 0.0 {
        return false;
    }
    use geo::kernels::{Kernel, Orientation, RobustKernel};
    let wanted = if sign > 0.0 {
        Orientation::CounterClockwise
    } else {
        Orientation::Clockwise
    };
    ring.lines().all(|edge| {
        let side = |p: &geo::Coord| {
            sign * ((edge.end.x - edge.start.x) * (p.y - edge.start.y)
                - (edge.end.y - edge.start.y) * (p.x - edge.start.x))
        };
        ring.coords().all(|p| {
            let turn = RobustKernel::orient2d(edge.start, edge.end, *p);
            turn == wanted || turn == Orientation::Collinear
        }) && inner
            .coords()
            .all(|p| side(p) > 1e-8 && RobustKernel::orient2d(edge.start, edge.end, *p) == wanted)
    })
}

fn simple_difference(p: &Polygon, clip: &MultiPolygon) -> Option<Polygon> {
    if !p.interiors().is_empty() {
        return None;
    }
    if clip
        .iter()
        .all(|q| q.interiors().is_empty() && contains_ring(p, q.exterior()))
    {
        Some(Polygon::new(
            p.exterior().clone(),
            clip.iter().map(|q| q.exterior().clone()).collect(),
        ))
    } else {
        None
    }
}

fn simple_intersection(p: &Polygon, clip: &MultiPolygon) -> Option<Polygon> {
    let [q] = clip.0.as_slice() else {
        return None;
    };
    if !contains_ring(q, p.exterior()) {
        return None;
    }
    if q.interiors().is_empty() {
        return Some(p.clone());
    }
    if !p.interiors().is_empty() {
        return None;
    }
    if q.interiors().iter().all(|h| contains_ring(p, h)) {
        Some(Polygon::new(p.exterior().clone(), q.interiors().to_vec()))
    } else {
        None
    }
}

fn union_group(g: MultiPolygon) -> Vec<Polygon> {
    if g.0.len() == 1 {
        return g.0;
    }
    if let [a, b] = g.0.as_slice() {
        if a.interiors().is_empty() && contains_ring(a, b.exterior()) {
            return vec![a.clone()];
        }
        if b.interiors().is_empty() && contains_ring(b, a.exterior()) {
            return vec![b.clone()];
        }
    }
    geo::unary_union(&g).0
}

#[derive(Default)]
struct UnionMemo {
    entries: std::collections::HashMap<Vec<u64>, (geo::Coord, Vec<Polygon>)>,
    #[cfg(test)]
    hits: usize,
}

fn coords(p: &Polygon) -> impl Iterator<Item = &geo::Coord> {
    p.exterior()
        .coords()
        .chain(p.interiors().iter().flat_map(|r| r.coords()))
}

impl UnionMemo {
    fn union(&mut self, shape: MultiPolygon) -> Vec<Polygon> {
        if shape.0.len() < 2 {
            return shape.0;
        }
        let signature = super::validity::group(&shape);
        if let Some(ref signature) = signature {
            if let Some((origin, polygons)) = self.entries.get(&signature.key) {
                if let Some(delta) = super::validity::offset(signature.origin, *origin) {
                    let translate = |p: geo::Coord| geo::Coord {
                        x: p.x + delta.x,
                        y: p.y + delta.y,
                    };
                    if polygons
                        .iter()
                        .flat_map(coords)
                        .all(|p| super::validity::in_band(translate(*p), signature.bands))
                    {
                        #[cfg(test)]
                        {
                            self.hits = self.hits.saturating_add(1);
                        }
                        return polygons.iter().map(|p| p.map_coords(translate)).collect();
                    }
                }
            }
        }
        let polygons = union_group(shape);
        if let Some(signature) = signature {
            let points = polygons.iter().flat_map(coords).take(513).count();
            if points <= 512
                && polygons
                    .iter()
                    .flat_map(coords)
                    .all(|p| super::validity::in_band(*p, signature.bands))
                && (self.entries.len() < 128 || self.entries.contains_key(&signature.key))
            {
                self.entries
                    .insert(signature.key, (signature.origin, polygons.clone()));
            }
        }
        polygons
    }
}

pub trait LocalOps {
    fn union_fast(&self, other: &Self) -> Self;
    fn difference_fast(&self, other: &Self) -> Self;
    fn intersection_fast(&self, other: &Self) -> Self;
}
impl LocalOps for MultiPolygon {
    fn union_fast(&self, other: &Self) -> Self {
        if complexity(self).max(complexity(other)) < LOCAL_THRESHOLD {
            return self.union(other);
        }
        if self.0.is_empty() {
            return other.clone();
        }
        if other.0.is_empty() {
            return self.clone();
        }
        let all = MultiPolygon::new(self.iter().chain(other).cloned().collect());
        match components(&all, 0.0) {
            None => self.union(other),
            Some(groups) => {
                let mut memo = UnionMemo::default();
                MultiPolygon::new(groups.into_iter().flat_map(|g| memo.union(g)).collect())
            }
        }
    }

    fn difference_fast(&self, other: &Self) -> Self {
        if complexity(self).max(complexity(other)) < LOCAL_THRESHOLD {
            return self.difference(other);
        }
        let index = Index::new(other);
        let mut ready = Vec::new();
        let mut pending = Vec::new();
        for p in self {
            let Some(bounds) = p.bounding_rect() else {
                continue;
            };
            let clip = index.nearby(bounds, true);
            if let Some(result) = simple_difference(p, &clip) {
                ready.push(result);
            } else {
                pending.push(p.clone());
            }
        }
        let pending = MultiPolygon::new(pending);
        ready.extend(
            MultiPolygon::new(
                batches(&pending)
                    .iter()
                    .flat_map(|p| {
                        let Some(bounds) = p.bounding_rect() else {
                            return vec![];
                        };
                        let clip = index.nearby(bounds, true);
                        if clip.0.is_empty() {
                            p.0.clone()
                        } else {
                            p.difference(&clip).0
                        }
                    })
                    .collect(),
            )
            .0,
        );
        MultiPolygon::new(ready)
    }

    fn intersection_fast(&self, other: &Self) -> Self {
        if complexity(self).max(complexity(other)) < LOCAL_THRESHOLD {
            return self.intersection(other);
        }
        // Iterate the smaller pieces; query the larger ones and only their relevant holes.
        let (pieces, indexed) = if self.0.len() >= other.0.len() {
            (self, other)
        } else {
            (other, self)
        };
        let index = Index::new(indexed);
        let mut ready = Vec::new();
        let mut pending = Vec::new();
        for p in pieces {
            let Some(bounds) = p.bounding_rect() else {
                continue;
            };
            let clip = index.nearby(bounds, true);
            if clip.0.is_empty() {
                continue;
            }
            if let Some(result) = simple_intersection(p, &clip) {
                ready.push(result);
            } else {
                pending.push(p.clone());
            }
        }
        let pending = MultiPolygon::new(pending);
        ready.extend(
            MultiPolygon::new(
                batches(&pending)
                    .iter()
                    .flat_map(|p| {
                        let Some(bounds) = p.bounding_rect() else {
                            return vec![];
                        };
                        let clip = index.nearby(bounds, true);
                        if clip.0.is_empty() {
                            vec![]
                        } else {
                            p.intersection(&clip).0
                        }
                    })
                    .collect(),
            )
            .0,
        );
        MultiPolygon::new(ready)
    }
}

/// Separate independent groups before expensive unions or buffers. A dense collection falls
/// back to the global operation after a bounded number of candidate visits.
pub fn components(shape: &MultiPolygon, margin: f64) -> Option<Vec<MultiPolygon>> {
    if shape.0.len() < LOCAL_THRESHOLD {
        return None;
    }
    let boxes: Vec<_> = shape
        .iter()
        .map(|p| {
            p.bounding_rect().map(|r| {
                envelope(Rect::new(
                    (r.min().x - margin, r.min().y - margin),
                    (r.max().x + margin, r.max().y + margin),
                ))
            })
        })
        .collect();
    let tree = RTree::bulk_load(
        boxes
            .iter()
            .enumerate()
            .filter_map(|(index, b)| b.map(|bounds| Entry { bounds, index }))
            .collect(),
    );
    let mut parent: Vec<_> = (0..shape.0.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while let Some(&p) = parent.get(i) {
            if p == i {
                break;
            }
            let up = parent.get(p).copied().unwrap_or(p);
            if let Some(slot) = parent.get_mut(i) {
                *slot = up;
            }
            i = up;
        }
        i
    }
    let mut visits = 0usize;
    for (i, b) in boxes.iter().enumerate() {
        let b = b.as_ref()?;
        for e in tree.locate_in_envelope_intersecting(b) {
            visits = visits.saturating_add(1);
            if visits > shape.0.len().saturating_mul(64) {
                return None;
            }
            if e.index > i {
                let (a, b) = (root(&mut parent, i), root(&mut parent, e.index));
                if let Some(slot) = parent.get_mut(b.max(a)) {
                    *slot = a.min(b);
                }
            }
        }
    }
    let mut groups = std::collections::BTreeMap::<usize, Vec<Polygon>>::new();
    for (i, p) in shape.iter().enumerate() {
        groups
            .entry(root(&mut parent, i))
            .or_default()
            .push(p.clone());
    }
    if groups.len() < 2 {
        return None;
    }
    Some(groups.into_values().map(MultiPolygon::new).collect())
}

pub fn union(shape: &MultiPolygon) -> MultiPolygon {
    match components(shape, 0.0) {
        None => geo::unary_union(shape),
        Some(groups) => {
            // Raw piles may contain self-crossing contours. We preserve the global fill rule
            // for those, and avoid expensive validity checks on complex raw polygons.
            let mut validity = super::validity::Validity::default();
            if groups.iter().flat_map(|g| g.iter()).any(|p| {
                p.exterior().0.len() > 256 || p.interiors().len() > 64 || !validity.valid(p)
            }) {
                return geo::unary_union(shape);
            }
            let mut memo = UnionMemo::default();
            MultiPolygon::new(groups.into_iter().flat_map(|g| memo.union(g)).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{Area, LineString, Validation};
    fn square(x: f64, y: f64, size: f64) -> Polygon {
        Polygon::new(
            LineString::from(vec![
                (x, y),
                (x + size, y),
                (x + size, y + size),
                (x, y + size),
                (x, y),
            ]),
            vec![],
        )
    }
    #[test]
    fn translated_unions_match_every_uncached_coordinate() {
        let mut memo = UnionMemo::default();
        for i in 1..30 {
            let x = 32.0 + f64::from(i) * 2.0;
            let y = 520.0 + f64::from(i) * 2.0;
            let shape = MultiPolygon::new(vec![
                square(x + 0.1, y + 0.1, 0.7),
                square(x + 0.45, y + 0.35, 0.6),
            ]);
            let expected = union_group(shape.clone());
            assert_eq!(memo.union(shape), expected);
        }
        assert!(memo.hits > 0, "we exercise cache reuse, not only fallback");
    }

    #[test]
    fn negative_coordinate_translations_match_uncached_unions() {
        let mut memo = UnionMemo::default();
        for i in 1..12 {
            let x = -32.0 - f64::from(i) * 2.0;
            let y = -520.0 - f64::from(i) * 2.0;
            let shape = MultiPolygon::new(vec![
                square(x + 0.1, y + 0.1, 0.7),
                square(x + 0.45, y + 0.35, 0.6),
            ]);
            assert_eq!(memo.union(shape.clone()), union_group(shape));
        }
        assert!(memo.hits > 0);
    }

    #[test]
    fn translated_capsule_loops_keep_every_coordinate_and_hole() {
        let mut memo = UnionMemo::default();
        for i in 1..12 {
            let x = 32.0 + f64::from(i) * 2.0;
            let y = 520.0 + f64::from(i) * 2.0;
            let point = |dx, dy| board_model::Point {
                x: x + dx,
                y: y + dy,
            };
            let pieces = [
                super::super::stroke(point(0.1, 0.1), point(0.1, 0.9), 0.12).unwrap(),
                super::super::stroke(point(0.1, 0.9), point(0.7, 0.6), 0.12).unwrap(),
                super::super::stroke(point(0.7, 0.6), point(0.1, 0.4), 0.12).unwrap(),
            ];
            let shape = MultiPolygon::new(pieces.to_vec());
            assert_eq!(memo.union(shape.clone()), union_group(shape));
        }
        assert!(memo.hits > 0);
    }

    #[test]
    fn an_odd_coordinate_ulp_uses_the_uncached_union() {
        let mut memo = UnionMemo::default();
        let shape = MultiPolygon::new(vec![square(40.1, 520.1, 0.7), square(40.45, 520.35, 0.6)]);
        memo.union(shape.clone());
        let hits = memo.hits;
        let ulp = f64::from_bits(40.1f64.to_bits() + 1) - 40.1;
        let shifted = shape.map_coords(|p| geo::Coord {
            x: p.x + ulp,
            y: p.y,
        });
        assert_eq!(memo.union(shifted.clone()), union_group(shifted));
        assert_eq!(memo.hits, hits);
    }

    #[test]
    fn translated_union_cache_has_a_fixed_size_limit() {
        let mut memo = UnionMemo::default();
        for i in 1..300 {
            let side = 0.1 + f64::from(i) * 0.001;
            let shape =
                MultiPolygon::new(vec![square(40.1, 520.1, side), square(40.15, 520.15, side)]);
            assert_eq!(memo.union(shape.clone()), union_group(shape));
        }
        assert!(memo.entries.len() <= 128);
    }

    #[test]
    fn isolated_self_crossing_input_is_normalized() {
        let mut pieces: Vec<_> = (0..4200)
            .map(|i| square(f64::from(i % 100) * 2.0, f64::from(i / 100) * 2.0, 0.8))
            .collect();
        pieces.push(Polygon::new(
            LineString::from(vec![
                (1000.0, 0.0),
                (1002.0, 2.0),
                (1000.0, 3.0),
                (1003.0, 0.0),
                (1000.0, 0.0),
            ]),
            vec![],
        ));
        let original = MultiPolygon::new(pieces);
        let actual = union(&original);
        assert!(actual.iter().all(Validation::is_valid));
        assert!(actual.xor(&geo::unary_union(&original)).unsigned_area() < 1e-6);
    }

    #[test]
    fn containment_shortcuts_keep_holes_and_reject_touching_edges() {
        let outer = square(0.0, 0.0, 10.0);
        let inner = square(2.0, 2.0, 1.0);
        let hole = MultiPolygon::new(vec![inner.clone()]);
        let cut = simple_difference(&outer, &hole).unwrap();
        assert!(cut.xor(&outer.difference(&hole)).unsigned_area() < 1e-6);
        let clipped =
            simple_intersection(&outer, &MultiPolygon::new(vec![square(-1.0, -1.0, 12.0)]))
                .unwrap();
        assert_eq!(clipped, outer);
        assert!(
            simple_difference(&outer, &MultiPolygon::new(vec![square(0.0, 0.0, 1.0)])).is_none()
        );
        let concave = Polygon::new(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (2.5, 2.5),
                (0.0, 10.0),
                (0.0, 0.0),
            ]),
            vec![],
        );
        assert!(!contains_ring(&concave, inner.exterior()));
    }

    #[test]
    fn independent_islands_and_growing_neighbors_are_grouped_safely() {
        let many = MultiPolygon::new(
            (0..4200)
                .map(|i| square(f64::from(i) * 2.0, 0.0, 0.8))
                .collect(),
        );
        assert_eq!(components(&many, 0.0).unwrap().len(), 4200);
        assert!(components(&many, 0.7).is_none());
        let united = union(&many);
        assert!(united.xor(&geo::unary_union(&many)).unsigned_area() < 1e-6);
    }

    #[test]
    fn distant_holes_do_not_change_a_local_clip() {
        let holes: Vec<_> = (0..4200)
            .map(|i| {
                square(f64::from(i % 100) * 2.0, f64::from(i / 100) * 2.0, 0.8)
                    .exterior()
                    .clone()
            })
            .collect();
        let face = MultiPolygon::new(vec![Polygon::new(
            square(-1.0, -1.0, 250.0).exterior().clone(),
            holes,
        )]);
        let clip = MultiPolygon::new(vec![square(-0.2, -0.2, 1.5)]);
        for (actual, expected) in [
            (clip.intersection_fast(&face), clip.intersection(&face)),
            (clip.difference_fast(&face), clip.difference(&face)),
        ] {
            assert!(actual.xor(&expected).unsigned_area() < 1e-6);
        }
    }

    #[test]
    fn local_booleans_match_global_area_and_holes() {
        let many = MultiPolygon::new(
            (0..4200)
                .map(|i| square(f64::from(i % 100) * 2.0, f64::from(i / 100) * 2.0, 0.8))
                .collect(),
        );
        let face = MultiPolygon::new(vec![Polygon::new(
            square(-1.0, -1.0, 250.0).exterior().clone(),
            many.iter()
                .step_by(3)
                .map(|p| p.exterior().clone())
                .collect(),
        )]);
        for (actual, expected) in [
            (many.difference_fast(&face), many.difference(&face)),
            (many.intersection_fast(&face), many.intersection(&face)),
            (face.difference_fast(&many), face.difference(&many)),
        ] {
            assert!(actual.xor(&expected).unsigned_area() < 1e-6);
        }
    }
}
