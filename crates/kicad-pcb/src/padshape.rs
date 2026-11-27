//! The outlines of pads that are not a plain circle, rectangle or oval: rectangles with chamfered
//! corners, trapezoids and custom pads. Everything here works in a pad's own frame (center at the
//! origin, Y up, not turned); [`placed`] moves a contour onto the board.
//!
//! The corner and delta conventions come from KiCad's own Gerber export of our `pad-shapes`
//! board (see its README.txt), not from any source code.

use crate::geom::arc_through;
use crate::sexpr::Node;
use board_model::{Contour, Point, Segment};

/// Which corners of a rectangle are chamfered.
#[derive(Clone, Copy, Debug, Default)]
pub struct Corners {
    pub top_left: bool,
    pub top_right: bool,
    pub bottom_left: bool,
    pub bottom_right: bool,
}

impl Corners {
    /// Reads KiCad's corner names (`top_left`, `top_right`, `bottom_left`, `bottom_right`).
    pub fn from_names<'a>(names: impl Iterator<Item = &'a str>) -> Corners {
        let mut c = Corners::default();
        for n in names {
            match n {
                "top_left" => c.top_left = true,
                "top_right" => c.top_right = true,
                "bottom_left" => c.bottom_left = true,
                "bottom_right" => c.bottom_right = true,
                _ => {}
            }
        }
        c
    }

    pub fn any(self) -> bool {
        self.top_left || self.top_right || self.bottom_left || self.bottom_right
    }
}

fn pt(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// Joins corner pieces (a start point, an optional arc, an end point) into a closed contour with
/// straight lines between the end of one piece and the start of the next.
fn join(pieces: &[(Point, Option<Segment>, Point)]) -> Contour {
    let mut segments = Vec::new();
    let mut prev_end = pieces.last().map(|p| p.2);
    for (start, arc, end) in pieces {
        if let Some(from) = prev_end {
            if (from.x - start.x).hypot(from.y - start.y) > 1e-9 {
                segments.push(Segment::Line { from, to: *start });
            }
        }
        match arc {
            Some(a) => segments.push(a.clone()),
            // A chamfer, or a sharp corner (no length).
            None if (start.x - end.x).hypot(start.y - end.y) > 1e-9 => {
                segments.push(Segment::Line {
                    from: *start,
                    to: *end,
                })
            }
            None => {}
        }
        prev_end = Some(*end);
    }
    Contour { segments }
}

/// A `w` by `h` rectangle with corners rounded to radius `r` and the `chamfered` corners cut
/// off by `c` instead (a chamfered corner is not also rounded). Counter-clockwise.
pub fn rounded_chamfered_rect(w: f64, h: f64, r: f64, c: f64, chamfered: Corners) -> Contour {
    let (hx, hy) = (w / 2.0, h / 2.0);
    let limit = (w.min(h) / 2.0).max(0.0);
    let (r, c) = (r.clamp(0.0, limit), c.clamp(0.0, limit));
    // (corner x sign, corner y sign, chamfered, angle the rounding starts at), going
    // counter-clockwise from the bottom left.
    let corners = [
        (-1.0, -1.0, chamfered.bottom_left, 180.0),
        (1.0, -1.0, chamfered.bottom_right, 270.0),
        (1.0, 1.0, chamfered.top_right, 0.0),
        (-1.0, 1.0, chamfered.top_left, 90.0),
    ];
    let mut pieces = Vec::new();
    for (sx, sy, cut, a0) in corners {
        let (x, y) = (sx * hx, sy * hy);
        let d = if cut { c } else { r };
        // Start: back along the edge we arrive on; end: forward along the edge we leave on.
        // The walk goes bottom-left, bottom-right, top-right, top-left, so at every corner the
        // arriving edge is the one towards the previous corner.
        let (start, end) = match (sx < 0.0, sy < 0.0) {
            (true, true) => (pt(x, y + d), pt(x + d, y)),
            (false, true) => (pt(x - d, y), pt(x, y + d)),
            (false, false) => (pt(x, y - d), pt(x - d, y)),
            (true, false) => (pt(x + d, y), pt(x, y - d)),
        };
        let arc = (!cut && r > 1e-9).then(|| Segment::Arc {
            center: pt(x - sx * r, y - sy * r),
            radius: r,
            start_deg: a0,
            end_deg: a0 + 90.0,
            clockwise: false,
        });
        pieces.push((start, arc, end));
    }
    join(&pieces)
}

/// A trapezoid: a `w` by `h` rectangle with KiCad's `rect_delta` `(dx, dy)` applied. A positive
/// `dy` narrows the top edge to `w - dy` and widens the bottom one to `w + dy`; a positive `dx`
/// lengthens the left edge to `h + dx` and shortens the right one to `h - dx` (measured on KiCad's
/// own Gerber output). `None` when the deltas leave no shape.
pub fn trapezoid(w: f64, h: f64, dx: f64, dy: f64) -> Option<Contour> {
    let (top, bottom) = (w / 2.0 - dy / 2.0, w / 2.0 + dy / 2.0);
    let (left, right) = (h / 2.0 + dx / 2.0, h / 2.0 - dx / 2.0);
    if top < 0.0 || bottom < 0.0 || left < 0.0 || right < 0.0 {
        return None;
    }
    // Counter-clockwise from the bottom left (Y up).
    Some(polygon(&[
        pt(-bottom, -left),
        pt(bottom, -right),
        pt(top, right),
        pt(-top, left),
    ]))
}

/// A closed polygon through `points`.
pub fn polygon(points: &[Point]) -> Contour {
    let mut segments = Vec::new();
    for (i, &from) in points.iter().enumerate() {
        let to = points
            .get(i + 1)
            .or(points.first())
            .copied()
            .unwrap_or(from);
        if (from.x - to.x).hypot(from.y - to.y) > 1e-9 {
            segments.push(Segment::Line { from, to });
        }
    }
    Contour { segments }
}

/// A full circle.
pub fn circle(center: Point, radius: f64) -> Contour {
    Contour {
        segments: vec![Segment::Arc {
            center,
            radius,
            start_deg: 0.0,
            end_deg: 0.0,
            clockwise: false,
        }],
    }
}

/// A stroke of `width` from `a` to `b` with round ends.
pub fn capsule(a: Point, b: Point, width: f64) -> Contour {
    let r = width / 2.0;
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = dx.hypot(dy);
    if len < 1e-9 {
        return circle(a, r);
    }
    let heading = dy.atan2(dx).to_degrees();
    let (nx, ny) = (-dy / len * r, dx / len * r);
    Contour {
        segments: vec![
            Segment::Line {
                from: pt(a.x - nx, a.y - ny),
                to: pt(b.x - nx, b.y - ny),
            },
            Segment::Arc {
                center: b,
                radius: r,
                start_deg: heading - 90.0,
                end_deg: heading + 90.0,
                clockwise: false,
            },
            Segment::Line {
                from: pt(b.x + nx, b.y + ny),
                to: pt(a.x + nx, a.y + ny),
            },
            Segment::Arc {
                center: a,
                radius: r,
                start_deg: heading + 90.0,
                end_deg: heading + 270.0,
                clockwise: false,
            },
        ],
    }
}

/// A contour of the pad's own frame turned `rotation_deg` counter-clockwise and moved so the
/// pad's center is at `at`.
pub fn placed(c: &Contour, at: Point, rotation_deg: f64) -> Contour {
    let (s, k) = rotation_deg.to_radians().sin_cos();
    let map = |p: Point| pt(at.x + p.x * k - p.y * s, at.y + p.x * s + p.y * k);
    Contour {
        segments: c
            .segments
            .iter()
            .map(|seg| match *seg {
                Segment::Line { from, to } => Segment::Line {
                    from: map(from),
                    to: map(to),
                },
                Segment::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                    clockwise,
                } => Segment::Arc {
                    center: map(center),
                    radius,
                    start_deg: start_deg + rotation_deg,
                    end_deg: end_deg + rotation_deg,
                    clockwise,
                },
            })
            .collect(),
    }
}

/// The point list of a custom pad's `gr_poly` as a closed contour in the pad's frame (Y negated
/// from the file), with `(arc (start) (mid) (end))` items kept as real arcs. `None` when fewer
/// than three corners (and no arc) can be read.
pub fn pts_contour(pts: &Node) -> Option<Contour> {
    let local = |n: &Node| n.pair().map(|(x, y)| pt(x, -y));
    let mut segments = Vec::new();
    let (mut first, mut at): (Option<Point>, Option<Point>) = (None, None);
    let mut corners = 0usize;
    for item in pts.args() {
        let (start, piece, end) = match item.head() {
            Some("xy") => {
                let Some(p) = local(item) else { continue };
                (p, None, p)
            }
            Some("arc") => {
                let (Some(s), Some(m), Some(e)) = (
                    item.child("start").and_then(local),
                    item.child("mid").and_then(local),
                    item.child("end").and_then(local),
                ) else {
                    continue;
                };
                (s, arc_through(s, m, e).map(|a| a.segment()), e)
            }
            _ => continue,
        };
        corners += 1;
        if let Some(prev) = at {
            if (prev.x - start.x).hypot(prev.y - start.y) > 1e-9 {
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
    // An arc alone is a shape: the closing edge joins its two ends.
    if corners < 3 && !segments.iter().any(|s| matches!(s, Segment::Arc { .. })) {
        return None;
    }
    if let (Some(a), Some(f)) = (at, first) {
        if (a.x - f.x).hypot(a.y - f.y) > 1e-9 {
            segments.push(Segment::Line { from: a, to: f });
        }
    }
    Some(Contour { segments })
}

/// A stroke of `width` along a circular arc (counter-clockwise from `start_deg` to `end_deg`, or
/// clockwise), with round ends. A sweep over 180 degrees is cut in two so each piece is a simple
/// contour; the pieces overlap at their ends and are drawn together.
pub fn arc_stroke(
    center: Point,
    radius: f64,
    start_deg: f64,
    end_deg: f64,
    clockwise: bool,
    width: f64,
) -> Vec<Contour> {
    let probe = Segment::Arc {
        center,
        radius,
        start_deg,
        end_deg,
        clockwise,
    };
    let sweep = probe.sweep_deg().unwrap_or(360.0).abs();
    // Counter-clockwise from `a0` for `sweep` degrees.
    let a0 = if clockwise { end_deg } else { start_deg };
    if sweep > 180.0 {
        let half = sweep / 2.0;
        return vec![
            arc_band(center, radius, a0, a0 + half, width),
            arc_band(center, radius, a0 + half, a0 + sweep, width),
        ];
    }
    vec![arc_band(center, radius, a0, a0 + sweep, width)]
}

/// The outline of a stroke of `width` along a counter-clockwise arc from `a0` to `a1` degrees.
fn arc_band(center: Point, radius: f64, a0: f64, a1: f64, width: f64) -> Contour {
    let h = width / 2.0;
    let on = |r: f64, a: f64| {
        let (s, c) = a.to_radians().sin_cos();
        pt(center.x + r * c, center.y + r * s)
    };
    let inner = (radius - h).max(0.0);
    let ring = |r: f64, from: f64, to: f64, clockwise: bool| Segment::Arc {
        center,
        radius: r,
        start_deg: from,
        end_deg: to,
        clockwise,
    };
    let cap = |at_deg: f64, from: f64| Segment::Arc {
        center: on(radius, at_deg),
        radius: h,
        start_deg: from,
        end_deg: from + 180.0,
        clockwise: false,
    };
    let pieces = [
        (on(radius + h, a0), Some(ring(radius + h, a0, a1, false)), on(radius + h, a1)),
        (on(radius + h, a1), Some(cap(a1, a1)), on(inner, a1)),
        (on(inner, a1), (inner > 1e-9).then(|| ring(inner, a1, a0, true)), on(inner, a0)),
        (on(inner, a0), Some(cap(a0, a0 + 180.0)), on(radius + h, a0)),
    ];
    join(&pieces)
}

/// Round-ended strokes of `width` along every piece of `c`.
pub fn stroke(c: &Contour, width: f64) -> Vec<Contour> {
    let mut out = Vec::new();
    for seg in &c.segments {
        match *seg {
            Segment::Line { from, to } => out.push(capsule(from, to, width)),
            Segment::Arc {
                center,
                radius,
                start_deg,
                end_deg,
                clockwise,
            } => out.extend(arc_stroke(center, radius, start_deg, end_deg, clockwise, width)),
        }
    }
    out
}
