//! Fonts embedded in a board: `(embedded_files (file (name ..) (type font) (data |..|)))`.
//!
//! Worked out from the file-format documentation (which says only that a `data` token holds the
//! file) and from boards KiCad 9 wrote with a font embedded (`tests/boards/embedded-font`): the
//! data is the font file compressed with zstd, then written in base64 between two `|` characters
//! over several lines. A font's family, style and bold and italic marks are not in the board
//! file, so they are read from the font file's own `name` and `head` tables.
//!
//! The data is untrusted. Every step is bounded: at most [`MAX_FILES`] files, [`MAX_PACKED`]
//! bytes of base64 each, and [`MAX_TOTAL`] bytes unpacked for all files together, with the zstd
//! window capped at [`MAX_WINDOW`]. A file that cannot be read gets a note and is left out.

use std::io::Read;

use board_model::{EmbeddedFont, Note, Severity};

/// The most `file` entries looked at.
const MAX_FILES: usize = 64;
/// The most base64 characters of one file.
const MAX_PACKED: usize = 48 << 20;
/// The most bytes all fonts together may unpack to.
const MAX_TOTAL: usize = 64 << 20;
/// The largest zstd window a font may ask for.
const MAX_WINDOW: u64 = 32 << 20;

/// One font file read from the board, with the names it carries.
pub struct Embedded {
    /// The file's name in the board, e.g. "DejaVuSansMono.ttf".
    pub file_name: String,
    /// Names the font answers to: the family (name 16, else 1), name 1 and the full name.
    pub names: Vec<String>,
    pub font: EmbeddedFont,
}

impl Embedded {
    /// True when `face` (a text's `face`) is one of this font's names.
    pub fn answers_to(&self, face: &str) -> bool {
        self.names.iter().any(|n| n.eq_ignore_ascii_case(face))
    }
}

/// Reads every embedded font of the board's `embedded_files` list.
pub fn read(root: &crate::sexpr::Node, notes: &mut Vec<Note>) -> Vec<Embedded> {
    let mut out = Vec::new();
    let mut budget = MAX_TOTAL;
    let Some(files) = root.child("embedded_files") else {
        return out;
    };
    let mut skipped_kind = 0usize;
    for (i, file) in files.children("file").enumerate() {
        if i >= MAX_FILES {
            notes.push(Note {
                severity: Severity::Dropped,
                message: format!("embedded file(s) past the first {MAX_FILES} were not read"),
            });
            break;
        }
        if file.child("type").and_then(|t| t.str_at(0)) != Some("font") {
            skipped_kind += 1;
            continue;
        }
        let name = file.child("name").and_then(|n| n.str_at(0)).unwrap_or("?");
        let Some(data) = file.child("data").and_then(|d| d.str_at(0)) else {
            notes.push(warn(format!("the embedded font {name} has no data")));
            continue;
        };
        match unpack(data, budget) {
            Ok(bytes) => {
                budget = budget.saturating_sub(bytes.len());
                match describe(name, bytes) {
                    Ok(e) => out.push(e),
                    Err(why) => notes.push(warn(format!(
                        "the embedded font {name} was not used ({why})"
                    ))),
                }
            }
            Err(why) => notes.push(warn(format!(
                "the embedded font {name} could not be unpacked ({why}); texts in it keep the stroke font"
            ))),
        }
    }
    if skipped_kind > 0 {
        notes.push(Note {
            severity: Severity::Info,
            message: format!("{skipped_kind} embedded file(s) that are not fonts were not read"),
        });
    }
    out
}

fn warn(message: String) -> Note {
    Note {
        severity: Severity::Warning,
        message,
    }
}

/// base64, then zstd, within `limit` unpacked bytes.
fn unpack(data: &str, limit: usize) -> Result<Vec<u8>, String> {
    if data.len() > MAX_PACKED {
        return Err(format!("its data is longer than {MAX_PACKED} characters"));
    }
    let packed = base64(data).ok_or("the data is not base64")?;
    check_frame(&packed, limit)?;
    let mut decoder = ruzstd::decoding::StreamingDecoder::new_with_max_window_size(
        packed.as_slice(),
        MAX_WINDOW,
    )
    .map_err(|e| format!("not zstd data ({e})"))?;
    let mut out = Vec::new();
    let cap = limit as u64;
    let n = decoder
        .by_ref()
        .take(cap.saturating_add(1))
        .read_to_end(&mut out)
        .map_err(|e| format!("damaged zstd data ({e})"))?;
    if n > limit {
        return Err(format!("it unpacks to more than the {limit} bytes left for fonts"));
    }
    Ok(out)
}

/// Refuses a zstd frame whose header declares more than `limit` bytes of content.
fn check_frame(packed: &[u8], limit: usize) -> Result<(), String> {
    if packed.get(..4) != Some(&[0x28, 0xB5, 0x2F, 0xFD]) {
        return Err("not zstd data (wrong magic number)".into());
    }
    let d = *packed.get(4).ok_or("a zstd frame cut short")?;
    let single = d & 0x20 != 0;
    let dict_id = [0usize, 1, 2, 4][usize::from(d & 3)];
    let at = 5 + usize::from(!single) + dict_id;
    let size_bytes = match d >> 6 {
        0 => usize::from(single),
        1 => 2,
        2 => 4,
        _ => 8,
    };
    if size_bytes > 0 {
        let bytes = packed
            .get(at..at + size_bytes)
            .ok_or("a zstd frame cut short")?;
        let mut v = 0u64;
        for (i, b) in bytes.iter().enumerate() {
            v |= u64::from(*b) << (8 * i);
        }
        if size_bytes == 2 {
            v += 256;
        }
        if v > limit as u64 {
            return Err(format!("it declares {v} bytes, more than the {limit} left for fonts"));
        }
    }
    Ok(())
}

/// Decodes base64, ignoring white space; `None` for any other stray character.
fn base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    let mut padding = 0usize;
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => {
                padding += 1;
                continue;
            }
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        if padding > 0 {
            return None;
        }
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn u16_at(d: &[u8], at: usize) -> Option<usize> {
    let b = d.get(at..at.checked_add(2)?)?;
    Some(usize::from(u16::from_be_bytes([b[0], b[1]])))
}

fn u32_at(d: &[u8], at: usize) -> Option<usize> {
    let b = d.get(at..at.checked_add(4)?)?;
    usize::try_from(u32::from_be_bytes([b[0], b[1], b[2], b[3]])).ok()
}

/// The table `tag` of the font file, as its bytes.
fn table<'a>(d: &'a [u8], start: usize, tag: &[u8; 4]) -> Option<&'a [u8]> {
    let n = u16_at(d, start.checked_add(4)?)?;
    for i in 0..n.min(256) {
        let rec = start.checked_add(12)?.checked_add(i.checked_mul(16)?)?;
        if d.get(rec..rec.checked_add(4)?)? == tag {
            let off = u32_at(d, rec + 8)?;
            let len = u32_at(d, rec + 12)?;
            return d.get(off..off.checked_add(len)?);
        }
    }
    None
}

/// A name from the `name` table: the first record with this id, in UTF-16 (Windows, Unicode) or
/// single-byte (Macintosh) form.
fn name_of(t: &[u8], id: usize) -> Option<String> {
    let count = u16_at(t, 2)?;
    let strings = u16_at(t, 4)?;
    let mut best: Option<(u8, String)> = None;
    for i in 0..count.min(512) {
        let rec = 6 + i * 12;
        if u16_at(t, rec + 6)? != id {
            continue;
        }
        let (platform, len, off) = (u16_at(t, rec)?, u16_at(t, rec + 8)?, u16_at(t, rec + 10)?);
        let raw = t.get(strings.checked_add(off)?..strings.checked_add(off)?.checked_add(len)?)?;
        let (rank, text) = match platform {
            3 | 0 => (
                0,
                String::from_utf16_lossy(
                    &raw.chunks_exact(2)
                        .map(|c| u16::from_be_bytes([c[0], c[1]]))
                        .collect::<Vec<_>>(),
                ),
            ),
            1 => (1, raw.iter().map(|b| char::from(*b)).collect()),
            _ => continue,
        };
        if text.is_empty() {
            continue;
        }
        if best.as_ref().is_none_or(|(r, _)| rank < *r) {
            best = Some((rank, text));
        }
    }
    best.map(|(_, s)| s)
}

/// Reads the names and the bold and italic marks of a TrueType or OpenType file.
fn describe(file_name: &str, bytes: Vec<u8>) -> Result<Embedded, String> {
    let start = if bytes.get(..4) == Some(b"ttcf") {
        u32_at(&bytes, 12).ok_or("a font collection with no fonts")?
    } else {
        0
    };
    let magic = bytes.get(start..start + 4).ok_or("not a font file")?;
    if !(magic == [0, 1, 0, 0] || magic == b"true" || magic == b"OTTO") {
        return Err("not a TrueType or OpenType file".into());
    }
    let names = table(&bytes, start, b"name").ok_or("the font has no name table")?;
    let family1 = name_of(names, 1);
    let family = name_of(names, 16).or_else(|| family1.clone());
    let family = family.ok_or("the font names no family")?;
    let full = name_of(names, 4).unwrap_or_else(|| family.clone());
    let style = name_of(names, 17)
        .or_else(|| name_of(names, 2))
        .unwrap_or_default();
    let mac = table(&bytes, start, b"head").and_then(|h| u16_at(h, 44)).unwrap_or(0);
    let (bold, italic) = (mac & 1 != 0, mac & 2 != 0);
    let mut answers = vec![family.clone()];
    answers.extend(family1);
    answers.push(full.clone());
    Ok(Embedded {
        file_name: file_name.to_string(),
        names: answers,
        font: EmbeddedFont {
            family,
            full_name: full,
            style,
            bold,
            italic,
            data: bytes,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_ignores_white_space_and_refuses_strays() {
        assert_eq!(base64("aGVs\n\t bG8=").as_deref(), Some(&b"hello"[..]));
        assert_eq!(base64("aGVsbG8").as_deref(), Some(&b"hello"[..]));
        assert_eq!(base64("a!b"), None);
        assert_eq!(base64("aG=s"), None);
    }

    #[test]
    fn hostile_data_is_an_error_not_a_panic() {
        assert!(unpack("", 100).is_err());
        assert!(unpack("AAAA", 100).is_err());
        // zstd magic only, then a frame header cut short.
        assert!(unpack("KLUv/Q==", 100).is_err());
        // A frame declaring 4 GB of content (single segment, 4-byte size field).
        let frame = [0x28, 0xB5, 0x2F, 0xFD, 0xA0, 0xFF, 0xFF, 0xFF, 0xFF];
        assert!(check_frame(&frame, 1 << 20).unwrap_err().contains("declares"));
        assert!(describe("x.ttf", vec![0; 3]).is_err());
        assert!(describe("x.ttf", vec![0, 1, 0, 0, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0]).is_err());
    }
}
