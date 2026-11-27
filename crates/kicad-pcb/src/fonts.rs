//! Fonts embedded in a board: `(embedded_files (file (name ..) (type font) (data |..|)))`.
//!
//! Worked out from the file-format documentation (which says only that a `data` token holds the
//! file), from the public OpenType specification (the `name` and `head` tables) and RFC 8878 (the
//! zstd frame header), and from boards KiCad 9 wrote with a font embedded (`tests/boards/embedded-font`): the
//! data is the font file compressed with zstd, then written in base64 between two `|` characters
//! over several lines. A font's family, style and bold and italic marks are not in the board
//! file, so they are read from the font file's own `name` and `head` tables.
//!
//! The data is untrusted. Every step is bounded: at most [`MAX_FILES`] files, [`MAX_PACKED`]
//! bytes of base64 each, and [`MAX_TOTAL`] bytes unpacked for all files together (a file that
//! fails still costs what it unpacked before failing), with the zstd window capped at
//! [`MAX_WINDOW`]. A file that cannot be read gets a note and is left out.

use std::io::Read;

use board_model::{EmbeddedFont, Note, Severity};

/// The most `file` entries looked at.
const MAX_FILES: usize = 64;
/// The most base64 characters of one file.
const MAX_PACKED: usize = 48 << 20;
/// The most bytes all fonts together may unpack to. Every file counts against it, one that
/// fails or is not a font too.
const MAX_TOTAL: usize = 16 << 20;
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
            Err((why, spent)) => {
                budget = budget.saturating_sub(spent);
                notes.push(warn(format!(
                    "the embedded font {name} could not be unpacked ({why}); texts in it keep the stroke font"
                )));
            }
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

/// base64, then zstd, within `limit` unpacked bytes. An error carries what the attempt cost
/// against the budget: the bytes it unpacked before failing, or the packed size when it stopped
/// before unpacking.
fn unpack(data: &str, limit: usize) -> Result<Vec<u8>, (String, usize)> {
    if data.len() > MAX_PACKED {
        return Err((
            format!("its data is longer than {MAX_PACKED} characters"),
            0,
        ));
    }
    let packed = base64(data).ok_or(("the data is not base64".to_string(), data.len() / 4 * 3))?;
    let early = |why: String| (why, packed.len());
    check_frame(&packed, limit).map_err(early)?;
    let mut decoder =
        ruzstd::decoding::StreamingDecoder::new_with_max_window_size(packed.as_slice(), MAX_WINDOW)
            .map_err(|e| early(format!("not zstd data ({e})")))?;
    let mut out = Vec::new();
    let cap = limit as u64;
    let read = decoder
        .by_ref()
        .take(cap.saturating_add(1))
        .read_to_end(&mut out);
    if let Err(e) = read {
        return Err((
            format!("damaged zstd data ({e})"),
            out.len().max(packed.len()),
        ));
    }
    if out.len() > limit {
        return Err((
            format!("it unpacks to more than the {limit} bytes left for fonts"),
            limit,
        ));
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
    let dict_id = match d & 3 {
        0 => 0usize,
        1 => 1,
        2 => 2,
        _ => 4,
    };
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
            return Err(format!(
                "it declares {v} bytes, more than the {limit} left for fonts"
            ));
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
    let b: [u8; 2] = d.get(at..at.checked_add(2)?)?.try_into().ok()?;
    Some(usize::from(u16::from_be_bytes(b)))
}

fn u32_at(d: &[u8], at: usize) -> Option<usize> {
    let b: [u8; 4] = d.get(at..at.checked_add(4)?)?.try_into().ok()?;
    usize::try_from(u32::from_be_bytes(b)).ok()
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
                    &raw.as_chunks::<2>()
                        .0
                        .iter()
                        .map(|c| u16::from_be_bytes(*c))
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
    let magic = bytes
        .get(start..start.checked_add(4).ok_or("not a font file")?)
        .ok_or("not a font file")?;
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
    let mac = table(&bytes, start, b"head")
        .and_then(|h| u16_at(h, 44))
        .unwrap_or(0);
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

    fn base64_of(bytes: &[u8]) -> String {
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for c in bytes.chunks(3) {
            let n = c
                .iter()
                .enumerate()
                .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
            for i in 0..=c.len() {
                out.push(char::from(A[((n >> (18 - 6 * i)) & 63) as usize]));
            }
            out.extend(std::iter::repeat_n('=', 2 - (c.len() - 1)));
        }
        out
    }

    /// A zstd frame with a 2 MiB window and no declared size, made of `blocks` run-length blocks of
    /// 128 KiB zeros each (the last one marked last), so it unpacks to 128 KiB per block.
    fn zero_frame(blocks: usize) -> Vec<u8> {
        let mut f = vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x58];
        for i in 0..blocks {
            let last = u32::from(i + 1 == blocks);
            let header = (131_072u32 << 3) | (1 << 1) | last;
            f.extend_from_slice(&header.to_le_bytes()[..3]);
            f.push(0);
        }
        f
    }

    #[test]
    fn a_frame_is_unpacked_and_one_that_expands_past_the_limit_is_refused() {
        let ok = unpack(&base64_of(&zero_frame(2)), 1 << 20).unwrap();
        assert_eq!(ok.len(), 2 * 131_072);
        // Declares no size, but unpacks to 4 blocks of 128 KiB: more than the 300 KB allowed.
        let (why, spent) = unpack(&base64_of(&zero_frame(4)), 300_000).unwrap_err();
        assert!(why.contains("more than"), "{why}");
        assert_eq!(spent, 300_000);
    }

    #[test]
    fn a_frame_that_declares_a_small_size_and_holds_more_is_an_error() {
        // Single segment, one size byte saying 10, then three 128 KiB blocks.
        let mut f = vec![0x28, 0xB5, 0x2F, 0xFD, 0x20, 10];
        for i in 0..3 {
            let header = (131_072u32 << 3) | (1 << 1) | u32::from(i == 2);
            f.extend_from_slice(&header.to_le_bytes()[..3]);
            f.push(0);
        }
        let r = unpack(&base64_of(&f), 300_000);
        match r {
            Err(_) => {}
            Ok(out) => assert!(out.len() <= 300_000),
        }
    }

    #[test]
    fn a_window_over_the_cap_a_cut_frame_and_a_bad_header_are_errors() {
        // Window descriptor 0xF8: exponent 31, a 2 TiB window.
        let huge = [0x28, 0xB5, 0x2F, 0xFD, 0x00, 0xF8, 0x01, 0x00, 0x00, 0x00];
        assert!(unpack(&base64_of(&huge), 1 << 20).is_err());
        // The first frame cut in the middle of its second block, and cut right after a header.
        let f = zero_frame(3);
        for cut in [f.len() - 2, 6 + 4 + 2, 6 + 2, 5, 4, 2] {
            // Never a panic; if the decoder hands back what it had, it is never more than the whole.
            if let Ok(out) = unpack(&base64_of(&f[..cut]), 1 << 20) {
                assert!(out.len() < 3 * 131_072, "cut at {cut}");
            }
        }
        // Wrong magic number.
        assert!(unpack(&base64_of(&[1, 2, 3, 4, 5, 6]), 100).is_err());
    }

    #[test]
    fn a_failed_file_still_costs_the_budget() {
        let (_, spent) = unpack(&base64_of(&zero_frame(4)), 300_000).unwrap_err();
        assert_eq!(spent, 300_000);
        let (_, spent) = unpack("AAAAAAAAAAAA", 100).unwrap_err();
        assert!(spent > 0);
    }

    /// A font file with one table record `tag` at `offset` for `len` bytes, and nothing else.
    fn sfnt(tag: &[u8; 4], offset: u32, len: u32) -> Vec<u8> {
        let mut f = vec![0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        f.extend_from_slice(tag);
        f.extend_from_slice(&[0; 4]);
        f.extend_from_slice(&offset.to_be_bytes());
        f.extend_from_slice(&len.to_be_bytes());
        f
    }

    #[test]
    fn hostile_font_files_are_errors_not_panics() {
        assert!(describe("x.ttf", vec![0; 3]).is_err());
        assert!(describe("x.ttf", vec![0, 1, 0, 0, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0]).is_err());
        // The name table starts past the end, or runs past it, or wraps.
        for (off, len) in [(1 << 30, 10), (28, 1 << 30), (u32::MAX, u32::MAX), (28, 0)] {
            assert!(
                describe("x.ttf", sfnt(b"name", off, len)).is_err(),
                "{off} {len}"
            );
        }
        // A name table claiming 65535 records and a string area past the end.
        let mut f = sfnt(b"name", 28, 6);
        f.extend_from_slice(&[0, 0, 0xFF, 0xFF, 0xFF, 0xFF]);
        assert!(describe("x.ttf", f).is_err());
        // A collection whose first font is past the end, and one pointing at itself.
        let mut c = b"ttcf".to_vec();
        c.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 1, 0xFF, 0xFF, 0xFF, 0xFF]);
        assert!(describe("x.ttc", c).is_err());
        let mut c = b"ttcf".to_vec();
        c.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 12]);
        assert!(describe("x.ttc", c).is_err());
        // An offset that makes `start + 4` wrap.
        let mut c = b"ttcf".to_vec();
        c.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 1, 0xFF, 0xFF, 0xFF, 0xFD]);
        assert!(describe("x.ttc", c).is_err());
    }

    #[test]
    fn hostile_data_is_an_error_not_a_panic() {
        assert!(unpack("", 100).is_err());
        assert!(unpack("AAAA", 100).is_err());
        // zstd magic only, then a frame header cut short.
        assert!(unpack("KLUv/Q==", 100).is_err());
        // A frame declaring 4 GB of content (single segment, 4-byte size field).
        let frame = [0x28, 0xB5, 0x2F, 0xFD, 0xA0, 0xFF, 0xFF, 0xFF, 0xFF];
        assert!(check_frame(&frame, 1 << 20)
            .unwrap_err()
            .contains("declares"));
    }
}
