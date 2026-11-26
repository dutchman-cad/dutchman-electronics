//! Geometry the reader needs: KiCad's frame (Y down) to the neutral one (Y up), the turn of a
//! footprint, arcs given by three points, and contours.

use std::collections::HashMap;

use board_model::{Contour, Point, Segment};

use crate::sexpr::Node;

/// Two points closer than this (millimetres) are the same point when a contour is joined up.
/// Our own choice, 5 micrometres: KiCad writes nanometres but arcs are given by three rounded
/// points, which leave the ends about a micrometre off the circle.
pub const JOIN_TOLERANCE: f64 = 0.005;

/// Where a footprint sits: its origin in board coordinates (Y up) and its turn in degrees,
/// counter-clockwise as seen on the board. The identity places things as they are.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    x: f64,
    y: f64,
    cos: f64,
    sin: f64,
}

impl Frame {
    pub const IDENTITY: Frame = Frame {
        x: 0.0,
        y: 0.0,
        cos: 1.0,
        sin: 0.0,
    };

    /// A footprint at KiCad's `(at x y angle)`.
    pub fn new(x: f64, y: f64, angle_deg: f64) -> Frame {
        let r = angle_deg.to_radians();
        Frame {
            x,
            y: -y,
            cos: r.cos(),
            sin: r.sin(),
        }
    }

    /// A point written in KiCad's coordinates inside this frame, on the board in Y up.
    pub fn point(&self, kx: f64, ky: f64) -> Point {
        let (lx, ly) = (kx, -ky);
        Point {
            x: self.x + lx * self.cos - ly * self.sin,
            y: self.y + lx * self.sin + ly * self.cos,
        }
    }
}

/// `(x y)` of a `(xy X Y)` or `(start X Y)` style list, placed with `frame`.
pub fn place(frame: &Frame, node: &Node) -> Option<Point> {
    let (x, y) = node.pair()?;
    Some(frame.point(x, y))
}

pub fn dist(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn deg(y: f64, x: f64) -> f64 {
    let d = y.atan2(x).to_degrees().rem_euclid(360.0);
    // A hair below zero rounds up to 360: that is zero.
    if d > 360.0 - 1e-9 {
        0.0
    } else {
        d
    }
}

/// An arc through three points, in Y-up coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Arc3 {
    pub center: Point,
    pub radius: f64,
    pub start_deg: f64,
    pub end_deg: f64,
    /// The arc runs counter-clockwise from start to end.
    pub ccw: bool,
}

/// The circle through `start`, `mid` and `end`, and which way the arc goes. `None` when the
/// three points are on one line (or the same point).
pub fn arc_through(start: Point, mid: Point, end: Point) -> Option<Arc3> {
    let (ax, ay) = (start.x, start.y);
    let (bx, by) = (mid.x - ax, mid.y - ay);
    let (cx, cy) = (end.x - ax, end.y - ay);
    let d = 2.0 * (bx * cy - by * cx);
    let scale = bx.hypot(by).max(cx.hypot(cy));
    if !d.is_finite() || d.abs() <= 1e-9 * scale * scale || scale == 0.0 {
        return None;
    }
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;
    let ux = (cy * b2 - by * c2) / d;
    let uy = (bx * c2 - cx * b2) / d;
    let center = Point {
        x: ax + ux,
        y: ay + uy,
    };
    let radius = ux.hypot(uy);
    if !radius.is_finite() || radius <= 0.0 {
        return None;
    }
    let cross = (mid.x - start.x) * (end.y - mid.y) - (mid.y - start.y) * (end.x - mid.x);
    Some(Arc3 {
        center,
        radius,
        start_deg: deg(start.y - center.y, start.x - center.x),
        end_deg: deg(end.y - center.y, end.x - center.x),
        ccw: cross > 0.0,
    })
}

impl Arc3 {
    /// As a contour piece running from the arc's start to its end.
    pub fn segment(&self) -> Segment {
        Segment::Arc {
            center: self.center,
            radius: self.radius,
            start_deg: self.start_deg,
            end_deg: self.end_deg,
            clockwise: !self.ccw,
        }
    }

    /// The same arc as counter-clockwise from one angle to the other (for items that have no
    /// direction): start and end swap when it runs clockwise.
    pub fn ccw_angles(&self) -> (f64, f64) {
        if self.ccw {
            (self.start_deg, self.end_deg)
        } else {
            (self.end_deg, self.start_deg)
        }
    }
}

/// A full circle as one contour piece.
pub fn circle_segment(center: Point, radius: f64) -> Segment {
    Segment::Arc {
        center,
        radius,
        start_deg: 0.0,
        end_deg: 0.0,
        clockwise: false,
    }
}

/// The same piece walked the other way.
pub fn reversed(s: &Segment) -> Segment {
    match *s {
        Segment::Line { from, to } => Segment::Line { from: to, to: from },
        Segment::Arc {
            center,
            radius,
            start_deg,
            end_deg,
            clockwise,
        } => Segment::Arc {
            center,
            radius,
            start_deg: end_deg,
            end_deg: start_deg,
            clockwise: !clockwise,
        },
    }
}

fn arc_point(center: Point, radius: f64, deg: f64) -> Point {
    let r = deg.to_radians();
    Point {
        x: center.x + radius * r.cos(),
        y: center.y + radius * r.sin(),
    }
}

/// Where a piece starts and ends.
pub fn ends(s: &Segment) -> (Point, Point) {
    match *s {
        Segment::Line { from, to } => (from, to),
        Segment::Arc {
            center,
            radius,
            start_deg,
            end_deg,
            ..
        } => (
            arc_point(center, radius, start_deg),
            arc_point(center, radius, end_deg),
        ),
    }
}

/// The box a piece lies in: (min x, min y, max x, max y). An arc counts as its whole circle,
/// which is only used to compare sizes and to test containment roughly.
pub fn bounds(s: &Segment) -> (f64, f64, f64, f64) {
    match *s {
        Segment::Line { from, to } => (
            from.x.min(to.x),
            from.y.min(to.y),
            from.x.max(to.x),
            from.y.max(to.y),
        ),
        Segment::Arc { center, radius, .. } => (
            center.x - radius,
            center.y - radius,
            center.x + radius,
            center.y + radius,
        ),
    }
}

pub fn contour_bounds(c: &Contour) -> Option<(f64, f64, f64, f64)> {
    let mut it = c.segments.iter().map(bounds);
    let first = it.next()?;
    Some(it.fold(first, |a, b| {
        (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
    }))
}

/// The points of a `(pts ...)` list as a closed contour, placed with `frame`. Besides `(xy X Y)`
/// the list may hold `(arc (start) (mid) (end))`, which replaces the line to its end. `None`
/// when fewer than three corners are readable.
pub fn pts_contour(frame: &Frame, pts: &Node) -> Option<Contour> {
    let mut segments = Vec::new();
    let mut first: Option<Point> = None;
    let mut at: Option<Point> = None;
    let mut corners = 0usize;
    for item in pts.args() {
        let (start, piece, end) = match item.head() {
            Some("xy") => {
                let p = place(frame, item)?;
                (p, None, p)
            }
            Some("arc") => {
                let s = place(frame, item.child("start")?)?;
                let m = place(frame, item.child("mid")?)?;
                let e = place(frame, item.child("end")?)?;
                (s, arc_through(s, m, e).map(|a| a.segment()), e)
            }
            _ => continue,
        };
        corners += 1;
        if let Some(prev) = at {
            if dist(prev, start) > 1e-9 {
                segments.push(Segment::Line {
                    from: prev,
                    to: start,
                });
            }
        }
        match piece {
            Some(seg) => segments.push(seg),
            None if item.head() == Some("arc") => segments.push(Segment::Line {
                from: start,
                to: end,
            }),
            None => {}
        }
        first.get_or_insert(start);
        at = Some(end);
    }
    if corners < 3 {
        return None;
    }
    if let (Some(a), Some(f)) = (at, first) {
        if dist(a, f) > 1e-9 {
            segments.push(Segment::Line { from: a, to: f });
        }
    }
    Some(Contour { segments })
}

/// One piece of a zone's fill: the copper's outer contour and the holes inside it.
#[derive(Debug)]
pub struct FillPiece {
    pub outer: Contour,
    pub holes: Vec<Contour>,
}

/// The most loops one fill polygon is split into. More is left as the single contour KiCad wrote.
const MAX_FILL_LOOPS: usize = 4096;

fn signed_area(points: &[Point]) -> f64 {
    let twice: f64 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum();
    twice / 2.0
}

/// True when `p` is inside the polygon (even-odd rule).
fn inside(points: &[Point], p: Point) -> bool {
    let mut odd = false;
    for (a, b) in points.iter().zip(points.iter().cycle().skip(1)) {
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            odd = !odd;
        }
    }
    odd
}

/// A vertex as a hashable key: KiCad writes nanometres, so six decimals tell vertices apart.
fn vertex_key(p: Point) -> (i64, i64) {
    ((p.x * 1e6).round() as i64, (p.y * 1e6).round() as i64)
}

/// What [`split_fill`] made of one fill.
#[derive(Debug)]
pub struct FillSplit {
    pub pieces: Vec<FillPiece>,
    /// Why the fill was kept as the one contour KiCad wrote, with its holes not separated.
    pub kept_whole: Option<&'static str>,
    /// Holes that lie inside none of the copper pieces and were left out.
    pub orphan_holes: usize,
}

/// The most corners a fill may have to be split (the work grows with corners times holes).
const MAX_FILL_POINTS: usize = 400_000;
/// The most polygon edges tested spent putting holes into pieces when a fill has several
/// pieces (one piece needs none).
const MAX_FILL_TESTS: usize = 40_000_000;

fn point_bounds(points: &[Point]) -> (Point, Point) {
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

/// A zone's `filled_polygon` as outer contours with their holes.
///
/// KiCad writes one closed ring for a whole fill: each hole is reached from the outside by a
/// zero-width bridge (the ring runs to a vertex, round the hole, and back to the same vertex).
/// The ring is cut into simple loops wherever it comes back to a vertex it already passed; the
/// bridges collapse to loops without area and are dropped. Loops that wind like the biggest one
/// are copper (several when the ring pinches), the others are holes. A fill with one piece gives
/// it all its holes; with several, each hole goes to the smallest piece whose bounding box holds
/// it and that holds one of three of its corners, and a hole no piece holds is counted in
/// [`FillSplit::orphan_holes`] and left out. A fill with arcs, with more than [`MAX_FILL_LOOPS`]
/// loops or [`MAX_FILL_POINTS`] corners, or one that would cost more than [`MAX_FILL_TESTS`]
/// tests, is returned as the one contour that was written, with the reason in
/// [`FillSplit::kept_whole`]. `None` when fewer than three corners are readable.
pub fn split_fill(frame: &Frame, pts: &Node) -> Option<FillSplit> {
    let kept = |outer: Contour, why: &'static str| {
        Some(FillSplit {
            pieces: vec![FillPiece {
                outer,
                holes: Vec::new(),
            }],
            kept_whole: Some(why),
            orphan_holes: 0,
        })
    };
    if pts.args().iter().any(|n| n.head() == Some("arc")) {
        return kept(
            pts_contour(frame, pts)?,
            "has arcs, which are not split into holes",
        );
    }
    let mut ring: Vec<Point> = Vec::new();
    for item in pts.args() {
        if item.head() == Some("xy") {
            let p = place(frame, item)?;
            if ring.last().is_none_or(|q| dist(*q, p) > 1e-9) {
                ring.push(p);
            }
        }
    }
    while let (Some(&a), Some(&b)) = (ring.first(), ring.last()) {
        if ring.len() > 1 && dist(a, b) <= 1e-9 {
            ring.pop();
        } else {
            break;
        }
    }
    let first = ring.first().copied()?;
    if ring.len() < 3 {
        return None;
    }
    if ring.len() > MAX_FILL_POINTS {
        return kept(
            crate::padshape::polygon(&ring),
            "has too many corners to split into holes",
        );
    }
    let too_many_loops = "has too many holes to split";
    // Cut the ring into loops at every vertex it passes twice.
    let mut stack: Vec<Point> = Vec::new();
    let mut at: HashMap<(i64, i64), usize> = HashMap::new();
    let mut loops: Vec<Vec<Point>> = Vec::new();
    for &p in ring.iter().chain(std::iter::once(&first)) {
        let key = vertex_key(p);
        match at.get(&key).copied() {
            Some(pos) => {
                let tail: Vec<Point> = stack.drain(pos + 1..).collect();
                for q in &tail {
                    at.remove(&vertex_key(*q));
                }
                let mut l: Vec<Point> = stack.get(pos).copied().into_iter().collect();
                l.extend(tail);
                loops.push(l);
                if loops.len() > MAX_FILL_LOOPS {
                    return kept(crate::padshape::polygon(&ring), too_many_loops);
                }
            }
            None => {
                at.insert(key, stack.len());
                stack.push(p);
            }
        }
    }
    let mut loops: Vec<(Vec<Point>, f64)> = loops
        .into_iter()
        .filter(|l| l.len() >= 3)
        .map(|l| {
            let a = signed_area(&l);
            (l, a)
        })
        .filter(|(_, a)| a.abs() > 1e-9)
        .collect();
    let Some(big) = loops
        .iter()
        .map(|(_, a)| *a)
        .max_by(|a, b| a.abs().total_cmp(&b.abs()))
    else {
        return kept(crate::padshape::polygon(&ring), "has no area to split");
    };
    loops.sort_by(|a, b| a.1.abs().total_cmp(&b.1.abs()));
    let (outers, holes): (Vec<_>, Vec<_>) = loops
        .into_iter()
        .partition(|(_, a)| (*a > 0.0) == (big > 0.0));
    // `outers` is sorted smallest first, so the first one to hold a hole is its smallest parent.
    let mut pieces: Vec<(Vec<Point>, (Point, Point), Vec<Contour>)> = outers
        .into_iter()
        .map(|(l, _)| {
            let b = point_bounds(&l);
            (l, b, Vec::new())
        })
        .collect();
    let mut orphans = 0usize;
    let mut tests = 0usize;
    for (hole, _) in holes {
        if pieces.len() == 1 {
            if let Some(piece) = pieces.first_mut() {
                piece.2.push(crate::padshape::polygon(&hole));
            }
            continue;
        }
        let (lo, hi) = point_bounds(&hole);
        // A corner from the middle of the hole, then two more in case one sits on an edge.
        let n = hole.len();
        let samples: Vec<Point> = [n / 2, n / 4, n * 3 / 4]
            .iter()
            .filter_map(|&i| hole.get(i).copied())
            .collect();
        let mut target = None;
        'pieces: for (i, (outer, (olo, ohi), _)) in pieces.iter().enumerate() {
            if lo.x < olo.x - 1e-6
                || lo.y < olo.y - 1e-6
                || hi.x > ohi.x + 1e-6
                || hi.y > ohi.y + 1e-6
            {
                continue;
            }
            for p in &samples {
                tests = tests.saturating_add(outer.len());
                if tests > MAX_FILL_TESTS {
                    return kept(
                        crate::padshape::polygon(&ring),
                        "has too many pieces and holes to split",
                    );
                }
                if inside(outer, *p) {
                    target = Some(i);
                    break 'pieces;
                }
            }
        }
        match target.and_then(|i| pieces.get_mut(i)) {
            Some(piece) => piece.2.push(crate::padshape::polygon(&hole)),
            None => orphans = orphans.saturating_add(1),
        }
    }
    Some(FillSplit {
        pieces: pieces
            .into_iter()
            .map(|(outer, _, holes)| FillPiece {
                outer: crate::padshape::polygon(&outer),
                holes,
            })
            .collect(),
        kept_whole: None,
        orphan_holes: orphans,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    #[test]
    fn a_quarter_circle_runs_the_way_its_middle_point_says() {
        let r = 0.5f64.sqrt();
        let ccw = arc_through(pt(1.0, 0.0), pt(r, r), pt(0.0, 1.0)).unwrap();
        assert!(ccw.ccw);
        assert!((ccw.radius - 1.0).abs() < 1e-9);
        assert!((ccw.start_deg - 0.0).abs() < 1e-9 && (ccw.end_deg - 90.0).abs() < 1e-9);
        let cw = arc_through(pt(0.0, 1.0), pt(r, r), pt(1.0, 0.0)).unwrap();
        assert!(!cw.ccw);
        assert_eq!(cw.ccw_angles(), (cw.end_deg, cw.start_deg));
        assert!(arc_through(pt(0.0, 0.0), pt(1.0, 0.0), pt(2.0, 0.0)).is_none());
        assert!(arc_through(pt(0.0, 0.0), pt(0.0, 0.0), pt(0.0, 0.0)).is_none());
    }

    #[test]
    fn a_frame_turns_counter_clockwise_as_seen_on_the_board() {
        // KiCad's Y points down, so a point 0.8 to the left of a footprint turned 90 degrees
        // lands 0.8 below it on screen: in Y up that is 0.8 lower.
        let f = Frame::new(25.0, 15.0, 90.0);
        let p = f.point(-0.8, 0.0);
        assert!((p.x - 25.0).abs() < 1e-9);
        assert!((p.y - (-15.0 - 0.8)).abs() < 1e-9);
    }
}
