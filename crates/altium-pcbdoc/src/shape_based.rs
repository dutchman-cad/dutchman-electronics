//! The "shape-based" copies of the body and region streams.
//!
//! Worked out from our own CC0 test boards (see LEGAL.md): on every board that has them, each
//! record of `/ShapeBasedComponentBodies6/Data` (15 records on 13 boards) and of
//! `/ShapeBasedRegions6/Data` (`one-region`, `one-pour`, `outline-with-hole`) repeats the record
//! at the same place in `/ComponentBodies6/Data` or `/Regions6/Data`:
//!
//! - the same type byte, the same 18 bytes and the same text, byte for byte;
//! - then the same outline in another form: a `u32` count n and n + 1 vertices of 37 bytes each
//!   (a byte 0, x and y as `i32` in 1/10000 mil, then 28 bytes 0), the last vertex repeating the
//!   first, where the plain record has n pairs of `f64`.
//!
//! The leading byte and the zeros look like room for arcs, which none of our outlines has. So
//! the reader reads the plain streams and only checks that the copy matches, with a warning when
//! it does not.

use crate::copper::read_u32;

/// Bytes at the start of a record's body that come before its text length.
const SKIPPED: usize = 18;
/// Bytes per outline vertex in a shape-based record.
const VERTEX: usize = 37;
/// The most records or vertices compared before the stream is treated as hostile.
const MAX_ITEMS: usize = 1_000_000;

/// True when `shape` only repeats `plain`, both runs of records of type `kind`: as many records,
/// each with the same bytes up to the end of its text and the same outline corners, with no
/// arcs. False for anything else, including a stream that cannot be split.
pub(crate) fn repeats(shape: &[u8], plain: &[u8], kind: u8) -> bool {
    let (Some(plain), Some(shaped)) = (split(plain, kind), split(shape, kind)) else {
        return false;
    };
    plain.len() == shaped.len()
        && plain
            .iter()
            .zip(&shaped)
            .all(|(p, s)| same_record(p, s).unwrap_or(false))
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

/// One record of each stream: see [`repeats`]. `None` when either is cut short.
fn same_record(plain: &[u8], shaped: &[u8]) -> Option<bool> {
    let text_end = (SKIPPED + 4).checked_add(read_u32(plain, SKIPPED)?)?;
    if plain.get(..text_end)? != shaped.get(..text_end)? {
        return Some(false);
    }
    let n = read_u32(plain, text_end)?;
    if read_u32(shaped, text_end)? != n || n > MAX_ITEMS {
        return Some(false);
    }
    let first = text_end.checked_add(4)?;
    if shaped.len() != first.checked_add(VERTEX.checked_mul(n.checked_add(1)?)?)? {
        return Some(false);
    }
    let vertex = |k: usize| shaped.get(first + VERTEX * k..first + VERTEX * (k + 1));
    for k in 0..=n {
        let v = vertex(k)?;
        let straight = v.first() == Some(&0) && v.get(9..)?.iter().all(|&b| b == 0);
        if !straight || (k == n && v != vertex(0)?) {
            return Some(false);
        }
        if k < n {
            let at = first + 16 * k;
            for (offset, coord_at) in [(1usize, at), (5, at + 8)] {
                let int: [u8; 4] = v.get(offset..offset + 4)?.try_into().ok()?;
                let float: [u8; 8] = plain.get(coord_at..coord_at + 8)?.try_into().ok()?;
                let diff = f64::from_le_bytes(float) - f64::from(i32::from_le_bytes(int));
                if diff.abs() >= 0.5 {
                    return Some(false);
                }
            }
        }
    }
    Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIND: u8 = 0x0b;

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
}
