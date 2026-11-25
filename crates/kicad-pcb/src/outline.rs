//! The board outline: the pieces drawn on `Edge.Cuts`, joined into closed contours.

use board_model::{Contour, Outline, Point, Segment};

use crate::geom::{contour_bounds, dist, ends, reversed, JOIN_TOLERANCE};

/// The most loose pieces joined up one by one. More than this is refused as hostile.
pub const MAX_PIECES: usize = 5_000;

/// What joining the pieces gave.
#[derive(Debug, Default)]
pub struct Joined {
    pub outline: Outline,
    /// Chains of pieces that never came back to their start, and so were left out.
    pub open_chains: usize,
    /// Closed contours left out of the outline because they lie outside the biggest one.
    pub outside: usize,
    /// Pieces left unjoined because there were too many.
    pub over_limit: usize,
}

/// Joins `open` pieces end to end, adds the `closed` contours (circles, rectangles, polygons),
/// and takes the biggest contour as the outer one and the rest as cutouts.
pub fn join(open: Vec<Segment>, closed: Vec<Contour>) -> Joined {
    let mut out = Joined::default();
    let mut contours = closed;
    let mut pieces = open;
    if pieces.len() > MAX_PIECES {
        out.over_limit = pieces.len() - MAX_PIECES;
        pieces.truncate(MAX_PIECES);
    }
    let mut used = vec![false; pieces.len()];
    for i in 0..pieces.len() {
        if used.get(i).copied().unwrap_or(true) {
            continue;
        }
        if let Some(flag) = used.get_mut(i) {
            *flag = true;
        }
        let Some(first) = pieces.get(i) else { continue };
        let mut chain = vec![first.clone()];
        let start = ends(first).0;
        let mut tail = ends(first).1;
        loop {
            if dist(start, tail) <= JOIN_TOLERANCE {
                break;
            }
            let mut next: Option<(usize, Segment)> = None;
            for (j, p) in pieces.iter().enumerate() {
                if used.get(j).copied().unwrap_or(true) {
                    continue;
                }
                let (a, b) = ends(p);
                if dist(a, tail) <= JOIN_TOLERANCE {
                    next = Some((j, p.clone()));
                    break;
                }
                if dist(b, tail) <= JOIN_TOLERANCE {
                    next = Some((j, reversed(p)));
                    break;
                }
            }
            let Some((j, seg)) = next else { break };
            if let Some(flag) = used.get_mut(j) {
                *flag = true;
            }
            tail = ends(&seg).1;
            chain.push(seg);
        }
        if dist(start, tail) <= JOIN_TOLERANCE {
            contours.push(Contour { segments: chain });
        } else {
            out.open_chains += 1;
        }
    }
    let area = |c: &Contour| {
        contour_bounds(c).map_or(0.0, |(x0, y0, x1, y1)| (x1 - x0) * (y1 - y0))
    };
    let Some(biggest) = contours
        .iter()
        .enumerate()
        .max_by(|a, b| area(a.1).total_cmp(&area(b.1)))
        .map(|(i, _)| i)
    else {
        return out;
    };
    let outer = contours.swap_remove(biggest);
    let bounds = contour_bounds(&outer);
    let eps = JOIN_TOLERANCE;
    for c in contours {
        let inside = match (contour_bounds(&c), bounds) {
            (Some(b), Some(o)) => {
                b.0 >= o.0 - eps && b.1 >= o.1 - eps && b.2 <= o.2 + eps && b.3 <= o.3 + eps
            }
            _ => false,
        };
        if inside {
            out.outline.cutouts.push(c);
        } else {
            out.outside += 1;
        }
    }
    out.outline.outer = outer;
    out
}

/// A closed contour of straight lines through these points.
pub fn polygon(points: &[Point]) -> Contour {
    let mut segments = Vec::new();
    for (i, a) in points.iter().enumerate() {
        let b = points.get((i + 1) % points.len()).unwrap_or(a);
        segments.push(Segment::Line { from: *a, to: *b });
    }
    Contour { segments }
}
