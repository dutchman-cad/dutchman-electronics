//! The "shape-based" copies of the body and region streams.
//!
//! Worked out from our own CC0 test boards (see LEGAL.md): on every board that has them, each
//! record of `/ShapeBasedComponentBodies6/Data` (15 records on 13 boards) and of
//! `/ShapeBasedRegions6/Data` (`one-region`, `one-pour`, `outline-with-hole`,
//! `outline-with-arcs`) repeats the record at the same place in `/ComponentBodies6/Data` or
//! `/Regions6/Data`:
//!
//! - the same type byte, the same 18 bytes and the same text, byte for byte;
//! - then the outline in another form: a `u32` count n and n + 1 vertices of 37 bytes each, the
//!   last vertex at the first one's place. A vertex is a byte (0 for a straight edge to the next
//!   vertex, 1 for an arc), x, y, the arc's centre x and y and its radius as `i32` in 1/10000 mil,
//!   then its start and end angle as `f64` degrees; all of the arc's fields are 0 on a straight
//!   edge. The arc runs counter-clockwise from its start to its end angle, like an outline arc of
//!   `/Board6/Data` (see `board.rs`).
//!
//! Where every edge is straight, the vertices are the n pairs of `f64` of the plain record. The
//! round board cutout of `outline-with-arcs` is two half circles here (centre (40, 15) mm,
//! radius 2, 0 to 180 and 180 to 360 degrees, the last vertex with byte 0) but a polygon of 35
//! corners in `/Regions6/Data`. So the reader reads the plain streams, takes the arcs from here
//! where a record has them, and checks that the rest only repeats the plain records.

use board_model::Point;

use crate::board::{unit_to_mm, Arc, Vertex};
use crate::copper::read_u32;

/// Bytes at the start of a record's body that come before its text length.
const SKIPPED: usize = 18;
/// Bytes per outline vertex in a shape-based record.
const VERTEX: usize = 37;
/// The most records or vertices compared before the stream is treated as hostile.
const MAX_ITEMS: usize = 1_000_000;

/// Compares `shape` with `plain`, both runs of records of type `kind`. `None` unless they have
/// as many records, each with the same bytes up to the end of its text. Otherwise, by record:
/// `Some(vertices)` where the copy's outline has arcs, `None` where it repeats the plain corners.
/// A record that has neither (other corners, or arcs that cannot be read) makes it all `None`.
pub(crate) fn compare(shape: &[u8], plain: &[u8], kind: u8) -> Option<Vec<Option<Vec<Vertex>>>> {
    let (plain, shaped) = (split(plain, kind)?, split(shape, kind)?);
    if plain.len() != shaped.len() {
        return None;
    }
    plain
        .iter()
        .zip(&shaped)
        .map(|(p, s)| same_record(p, s))
        .collect()
}

/// The bodies of a run of records of type `kind`, or `None` if the stream is not one.
fn split(data: &[u8], kind: u8) -> Option<Vec<&[u8]>> {
    let mut bodies = Vec::new();
    let mut at = 0usize;
    while at < data.len() {
        if bodies.len() >= MAX_ITEMS || *data.get(at)? != kind {
            return None;
        }
        let len = read_u32(data, at.checked_add(1)?)?;
        let start = at.checked_add(5)?;
        let end = start.checked_add(len)?;
        bodies.push(data.get(start..end)?);
        at = end;
    }
    Some(bodies)
}

/// One record of each stream: see [`compare`]. `None` when they differ or either is cut short;
/// `Some(None)` when the copy only repeats the plain corners; `Some(Some(vertices))` when its
/// outline has arcs.
fn same_record(plain: &[u8], shaped: &[u8]) -> Option<Option<Vec<Vertex>>> {
    let text_end = (SKIPPED + 4).checked_add(read_u32(plain, SKIPPED)?)?;
    if plain.get(..text_end)? != shaped.get(..text_end)? {
        return None;
    }
    let n = read_u32(shaped, text_end)?;
    if n > MAX_ITEMS {
        return None;
    }
    let first = text_end.checked_add(4)?;
    if shaped.len() != first.checked_add(VERTEX.checked_mul(n.checked_add(1)?)?)? {
        return None;
    }
    let vertices = (0..=n)
        .map(|k| shaped.get(first + VERTEX * k..first + VERTEX * (k + 1)))
        .collect::<Option<Vec<_>>>()?;
    let (head, tail) = (vertices.first()?, vertices.last()?);
    if vertices.iter().any(|v| v.first() != Some(&0)) {
        // The last vertex is at the first one's place; the rest of it may differ.
        if head.get(1..9)? != tail.get(1..9)? {
            return None;
        }
        return vertices
            .iter()
            .map(|v| read_vertex(v))
            .collect::<Option<Vec<_>>>()
            .map(Some);
    }
    if read_u32(plain, text_end)? != n || tail != head {
        return None;
    }
    for (k, v) in vertices.iter().take(n).enumerate() {
        if v.get(9..)?.iter().any(|&b| b != 0) {
            return None;
        }
        let at = first + 16 * k;
        for (offset, coord_at) in [(1usize, at), (5, at + 8)] {
            let int: [u8; 4] = v.get(offset..offset + 4)?.try_into().ok()?;
            let float: [u8; 8] = plain.get(coord_at..coord_at + 8)?.try_into().ok()?;
            let diff = f64::from_le_bytes(float) - f64::from(i32::from_le_bytes(int));
            if diff.abs() >= 0.5 {
                return None;
            }
        }
    }
    Some(None)
}

/// One 37-byte vertex of a shape-based outline, or `None` for an unknown kind byte or a
/// number out of range.
fn read_vertex(v: &[u8]) -> Option<Vertex> {
    let length = |at: usize| -> Option<f64> {
        let bytes: [u8; 4] = v.get(at..at + 4)?.try_into().ok()?;
        unit_to_mm(f64::from(i32::from_le_bytes(bytes))).ok()
    };
    let angle = |at: usize| -> Option<f64> {
        let bytes: [u8; 8] = v.get(at..at + 8)?.try_into().ok()?;
        Some(f64::from_le_bytes(bytes)).filter(|a| a.is_finite() && a.abs() <= 3600.0)
    };
    let arc = match v.first()? {
        0 => None,
        1 => Some(Arc {
            center: Point {
                x: length(9)?,
                y: length(13)?,
            },
            radius: length(17)?,
            start_deg: angle(21)?,
            end_deg: angle(29)?,
        }),
        _ => return None,
    };
    Some(Vertex {
        at: Point {
            x: length(1)?,
            y: length(5)?,
        },
        arc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIND: u8 = 0x0b;

    /// True when `shape` only repeats `plain`, with no arcs.
    fn repeats(shape: &[u8], plain: &[u8], kind: u8) -> bool {
        compare(shape, plain, kind).is_some_and(|r| r.iter().all(Option::is_none))
    }

    /// A record with the given outline, plain (`f64` pairs) or shape-based (37-byte vertices,
    /// the first repeated), framed as in the streams.
    fn record(points: &[(i32, i32)], shaped: bool) -> Vec<u8> {
        let text = b"|KIND=0|ISBOARDCUTOUT=FALSE";
        let mut body = vec![0u8; SKIPPED];
        body.extend_from_slice(&(text.len() as u32).to_le_bytes());
        body.extend_from_slice(text);
        body.extend_from_slice(&(points.len() as u32).to_le_bytes());
        if shaped {
            for (x, y) in points.iter().chain(points.first()) {
                body.push(0);
                body.extend_from_slice(&x.to_le_bytes());
                body.extend_from_slice(&y.to_le_bytes());
                body.extend_from_slice(&[0u8; 28]);
            }
        } else {
            for (x, y) in points {
                body.extend_from_slice(&f64::from(*x).to_le_bytes());
                body.extend_from_slice(&f64::from(*y).to_le_bytes());
            }
        }
        let mut data = vec![KIND];
        data.extend_from_slice(&(body.len() as u32).to_le_bytes());
        data.extend_from_slice(&body);
        data
    }

    #[test]
    fn a_shape_based_copy_must_repeat_the_records() {
        let square = [(0, 0), (1000, 0), (1000, 1000), (0, 1000)];
        let plain = record(&square, false);
        let shaped = record(&square, true);
        assert!(repeats(&shaped, &plain, KIND));
        assert!(repeats(&[], &[], KIND));
        // Another record type, a moved corner, an arc flag, a missing record, a cut-short
        // stream and garbage all count as different.
        assert!(!repeats(&shaped, &plain, 0x0c));
        let moved = record(&[(0, 0), (1000, 0), (1000, 2000), (0, 1000)], true);
        assert!(!repeats(&moved, &plain, KIND));
        let mut arc = shaped.clone();
        let first_vertex = arc.len() - 5 * VERTEX;
        arc[first_vertex] = 1;
        assert!(!repeats(&arc, &plain, KIND));
        assert!(!repeats(&[], &plain, KIND));
        assert!(!repeats(&shaped[..shaped.len() - 3], &plain, KIND));
        assert!(!repeats(&[0x42, 1, 2], &plain, KIND));
        assert!(!repeats(&[KIND, 0xff, 0xff, 0xff, 0x7f], &plain, KIND));
    }

    #[test]
    fn a_shape_based_circle_comes_back_as_two_arcs() {
        // A polygon in the plain record, two half circles (centre (1000, 0), radius 500, in
        // 1/10000 mil) in the copy, listed like `outline-with-arcs`'s cutout.
        let plain = record(&[(500, 0), (1000, 500), (1500, 0)], false);
        let text_end = plain.len() - 4 - 3 * 16;
        let mut shaped = plain[..text_end].to_vec();
        shaped.extend_from_slice(&2u32.to_le_bytes());
        for (flag, x, start, end) in [
            (1u8, 500, 0.0, 180.0),
            (1, 1500, 180.0, 360.0),
            (0, 500, 0.0, 180.0),
        ] {
            shaped.push(flag);
            for v in [x, 0i32, 1000, 0, 500] {
                shaped.extend_from_slice(&v.to_le_bytes());
            }
            shaped.extend_from_slice(&f64::to_le_bytes(start));
            shaped.extend_from_slice(&f64::to_le_bytes(end));
        }
        let body_len = (shaped.len() - 5) as u32;
        shaped[1..5].copy_from_slice(&body_len.to_le_bytes());
        let records = compare(&shaped, &plain, KIND).unwrap();
        let [Some(vertices)] = records.as_slice() else {
            panic!("{records:?}");
        };
        assert_eq!(vertices.len(), 3);
        let arc = vertices[1].arc.unwrap();
        assert_eq!((arc.start_deg, arc.end_deg), (180.0, 360.0));
        assert!((arc.radius - unit_to_mm(500.0).unwrap()).abs() < 1e-12);
        assert!((vertices[1].at.x - unit_to_mm(1500.0).unwrap()).abs() < 1e-12);
        // An unknown kind byte, or a last vertex somewhere else, is not a copy.
        let first = text_end + 4;
        let mut odd = shaped.clone();
        odd[first] = 7;
        assert!(compare(&odd, &plain, KIND).is_none());
        let mut open = shaped.clone();
        open[first + 2 * VERTEX + 1] = 9;
        assert!(compare(&open, &plain, KIND).is_none());
    }
}
