//! Embedded TrueType fonts, `/EmbeddedFonts6/Data`.
//!
//! Worked out from our own CC0 test boards `one-dejavu-text`, `one-dejavu-bold-text` and
//! `one-dejavu-italic-text` (one text each in DejaVu Sans Mono: regular, bold and italic), compared
//! with `empty-outline` and with the public DejaVu Sans Mono 2.37 files, which they hold byte for
//! byte (see their README.txt and LEGAL.md). `/EmbeddedFonts6/Header` is the number of fonts as a
//! `u32` (1 on each, 0 on boards without a font). The data is one record per font, back to back:
//!
//! - three strings, each a `u32` byte length and that many bytes of UTF-16 ending in a 0: the
//!   face's full name ("DejaVu Sans Mono Bold"), its family ("DejaVu Sans Mono", which is how a
//!   text names its font) and its style ("Book", "Bold", "Oblique"),
//! - three bytes: bold (1 only on the bold board), italic (1 only on the italic board), and a
//!   byte that is 1 on all three boards, whose meaning is not known,
//! - a `u32` length and that many bytes: the font file, zlib-compressed.
//!
//! The reader hands the unpacked bytes over as they are: it does not check that they are a
//! TrueType file. Whoever draws with them has to treat them as untrusted input.
//!
//! A text marks its font bold and italic in two bytes of its own record (see `texts.rs`); a text
//! is drawn with the font whose family and both marks match.

use std::collections::BTreeMap;

use board_model::{EmbeddedFont, Note, Severity, Text, TextFont, MAX_INPUT_BYTES};

/// The most fonts read from the stream before it is treated as hostile.
const MAX_RECORDS: usize = 10_000;
/// The longest name string, in bytes.
const MAX_NAME_BYTES: usize = 4_096;
/// The most fonts listed one by one in notes; the rest are counted in one note.
const MAX_FONT_NOTES: usize = 20;

/// One font as the stream stores it, still packed.
#[derive(Debug, PartialEq)]
pub(crate) struct Record<'a> {
    pub full_name: String,
    pub family: String,
    pub style: String,
    pub bold: bool,
    pub italic: bool,
    /// The bytes of the three names, as stored (for the privacy check).
    pub names: &'a [u8],
    /// The zlib-compressed font file.
    pub packed: &'a [u8],
}

/// Splits `/EmbeddedFonts6/Data` into its fonts. Fails when the data does not end exactly after
/// the last font.
pub(crate) fn records(data: &[u8]) -> Result<Vec<Record<'_>>, String> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at < data.len() {
        if out.len() >= MAX_RECORDS {
            return Err(format!("more than {MAX_RECORDS} fonts"));
        }
        let start = at;
        let mut names = Vec::with_capacity(3);
        for _ in 0..3 {
            let (name, next) = utf16_string(data, at)
                .ok_or_else(|| format!("font {}: a name is cut short or not text", out.len()))?;
            names.push(name);
            at = next;
        }
        let names_bytes = data.get(start..at).unwrap_or_default();
        let flags = data
            .get(at..at.saturating_add(3))
            .ok_or_else(|| format!("font {}: cut short", out.len()))?;
        let (bold, italic) = (flags.first() == Some(&1), flags.get(1) == Some(&1));
        at = at.saturating_add(3);
        let (packed, next) =
            sized(data, at).ok_or_else(|| format!("font {}: the file is cut short", out.len()))?;
        at = next;
        let [full_name, family, style]: [String; 3] = names
            .try_into()
            .map_err(|_| "three names expected".to_string())?;
        out.push(Record {
            full_name,
            family,
            style,
            bold,
            italic,
            names: names_bytes,
            packed,
        });
    }
    Ok(out)
}

/// Why a font file could not be unpacked, and how many bytes were unpacked before that (they
/// count against a shared budget like the bytes of a font that could be unpacked).
#[derive(Debug, PartialEq)]
pub(crate) struct Unpacked {
    pub why: String,
    pub spent: usize,
}

/// Unpacks one font file to at most `limit` bytes.
pub(crate) fn unpack(record: &Record<'_>, limit: usize) -> Result<Vec<u8>, Unpacked> {
    miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(record.packed, limit).map_err(|e| {
        let why = if e.status == miniz_oxide::inflate::TINFLStatus::HasMoreOutput {
            format!("it unpacks to more than {limit} bytes")
        } else {
            format!("not zlib data ({:?})", e.status)
        };
        Unpacked {
            why,
            spent: e.output.len(),
        }
    })
}

/// Hands over the embedded font file of every TrueType font the texts use, each once. A font
/// that the board does not embed, or that cannot be unpacked, gets a note; its texts keep their
/// position, height and rotation.
pub(crate) fn read(
    header: Option<&[u8]>,
    data: Option<&[u8]>,
    texts: &[Text],
    notes: &mut Vec<Note>,
) -> Vec<EmbeddedFont> {
    read_within(header, data, texts, MAX_INPUT_BYTES, notes)
}

/// [`read`] with all fonts together allowed to unpack to at most `limit` bytes.
fn read_within(
    header: Option<&[u8]>,
    data: Option<&[u8]>,
    texts: &[Text],
    limit: usize,
    notes: &mut Vec<Note>,
) -> Vec<EmbeddedFont> {
    let records = match data.filter(|d| !d.is_empty()).map(records) {
        Some(Ok(records)) => {
            check_count(header, records.len(), notes);
            records
        }
        Some(Err(why)) => {
            notes.push(Note {
                severity: Severity::Warning,
                message: format!(
                    "/EmbeddedFonts6/Data could not be read ({why}); no embedded font was read"
                ),
            });
            Vec::new()
        }
        None => {
            check_count(header, 0, notes);
            Vec::new()
        }
    };
    // Texts that name a font in different case use the same font (`EmbeddedFont::serves`), so
    // they are counted together, under the spelling met first.
    let mut used: BTreeMap<(String, bool, bool), (String, usize)> = BTreeMap::new();
    for t in texts {
        if let TextFont::TrueType { name, bold, italic } = &t.font {
            let (_, n) = used
                .entry((name.to_ascii_lowercase(), *bold, *italic))
                .or_insert_with(|| (name.clone(), 0));
            *n = n.saturating_add(1);
        }
    }
    let mut budget = limit;
    let mut fonts: Vec<EmbeddedFont> = Vec::new();
    let mut missing = Vec::new();
    let mut taken = vec![false; records.len()];
    for ((_, bold, italic), (name, n)) in &used {
        let face = describe(name, *bold, *italic);
        let found = records.iter().enumerate().find(|(_, r)| {
            r.family.eq_ignore_ascii_case(name) && r.bold == *bold && r.italic == *italic
        });
        let Some((index, record)) = found else {
            missing.push(format!(
                "{n} text(s) use the TrueType font {face}, which the board does not embed; their \
                 position, height and rotation are read"
            ));
            continue;
        };
        if let Some(flag) = taken.get_mut(index) {
            *flag = true;
        }
        match unpack(record, budget) {
            Ok(file) => {
                budget = budget.saturating_sub(file.len());
                fonts.push(EmbeddedFont {
                    family: record.family.clone(),
                    full_name: record.full_name.clone(),
                    style: record.style.clone(),
                    bold: record.bold,
                    italic: record.italic,
                    data: file,
                });
            }
            Err(e) => {
                notes.push(Note {
                    severity: Severity::Warning,
                    message: format!(
                        "the embedded font {face} could not be unpacked ({}; {budget} of the \
                         {limit} bytes allowed for all fonts were left for it); {n} text(s) use \
                         it, and their position, height and rotation are read",
                        e.why
                    ),
                });
                budget = budget.saturating_sub(e.spent);
            }
        }
    }
    for message in missing.iter().take(MAX_FONT_NOTES) {
        notes.push(Note {
            severity: Severity::Info,
            message: message.clone(),
        });
    }
    let more = missing.len().saturating_sub(MAX_FONT_NOTES);
    if more > 0 {
        notes.push(Note {
            severity: Severity::Info,
            message: format!("and {more} more TrueType font(s) that the board does not embed"),
        });
    }
    let unused = taken.iter().filter(|t| !**t).count();
    if unused > 0 {
        notes.push(Note {
            severity: Severity::Info,
            message: format!("{unused} embedded font(s) that no text uses were not handed over"),
        });
    }
    fonts
}

/// Notes a `/EmbeddedFonts6/Header` whose font count differs from the fonts in the data.
fn check_count(header: Option<&[u8]>, found: usize, notes: &mut Vec<Note>) {
    let Some(header) = header else {
        return;
    };
    let count = <[u8; 4]>::try_from(header)
        .ok()
        .map(u32::from_le_bytes)
        .and_then(|c| usize::try_from(c).ok());
    let message = match count {
        Some(c) if c == found => return,
        Some(c) => format!(
            "/EmbeddedFonts6/Header says the board embeds {c} font(s), but \
             /EmbeddedFonts6/Data holds {found}; the fonts in the data are used"
        ),
        None => format!(
            "/EmbeddedFonts6/Header is {} bytes, not a 4-byte font count; the {found} font(s) \
             in /EmbeddedFonts6/Data are used",
            header.len()
        ),
    };
    notes.push(Note {
        severity: Severity::Warning,
        message,
    });
}

/// "\"DejaVu Sans Mono\"", with " bold", " italic" or " bold italic" after it.
fn describe(name: &str, bold: bool, italic: bool) -> String {
    let style = match (bold, italic) {
        (false, false) => "",
        (true, false) => " bold",
        (false, true) => " italic",
        (true, true) => " bold italic",
    };
    format!("\"{name}\"{style}")
}

/// A `u32` length and that many bytes, starting at `at`; and where the next item starts.
fn sized(data: &[u8], at: usize) -> Option<(&[u8], usize)> {
    let len = data.get(at..at.checked_add(4)?)?;
    let len = usize::try_from(u32::from_le_bytes(len.try_into().ok()?)).ok()?;
    let start = at.checked_add(4)?;
    let end = start.checked_add(len)?;
    Some((data.get(start..end)?, end))
}

/// A sized UTF-16 string ending in a 0 (the 0 and anything after it are dropped).
fn utf16_string(data: &[u8], at: usize) -> Option<(String, usize)> {
    let (bytes, next) = sized(data, at)?;
    if bytes.len() > MAX_NAME_BYTES || bytes.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .take_while(|&u| u != 0)
        .collect();
    Some((String::from_utf16(&units).ok()?, next))
}

#[cfg(test)]
mod tests {
    use super::*;
    use board_model::Point;

    fn name(s: &str) -> Vec<u8> {
        let bytes: Vec<u8> = s
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        let mut out = u32::try_from(bytes.len())
            .unwrap_or(0)
            .to_le_bytes()
            .to_vec();
        out.extend(bytes);
        out
    }

    fn zlib(data: &[u8]) -> Vec<u8> {
        miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
    }

    fn record(full: &str, style: &str, bold: bool, italic: bool, packed: &[u8]) -> Vec<u8> {
        let mut out = name(full);
        out.extend(name("Test Sans"));
        out.extend(name(style));
        out.extend([u8::from(bold), u8::from(italic), 1]);
        out.extend(u32::try_from(packed.len()).unwrap_or(0).to_le_bytes());
        out.extend(packed);
        out
    }

    fn text(bold: bool, italic: bool, name: &str) -> Text {
        Text {
            layer: 0,
            at: Point { x: 0.0, y: 0.0 },
            content: "A".into(),
            height: 2.0,
            rotation_deg: 0.0,
            mirrored: false,
            font: TextFont::TrueType {
                name: name.into(),
                bold,
                italic,
            },
            component: None,
        }
    }

    fn two_fonts() -> Vec<u8> {
        let mut data = record("Test Sans", "Book", false, false, &zlib(b"regular font"));
        data.extend(record(
            "Test Sans Bold",
            "Bold",
            true,
            false,
            &zlib(b"bold font"),
        ));
        data
    }

    #[test]
    fn splits_records_and_reads_their_names_and_marks() -> Result<(), String> {
        let data = two_fonts();
        let r = records(&data)?;
        let summary: Vec<_> = r
            .iter()
            .map(|r| {
                (
                    r.full_name.as_str(),
                    r.family.as_str(),
                    r.style.as_str(),
                    r.bold,
                    r.italic,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("Test Sans", "Test Sans", "Book", false, false),
                ("Test Sans Bold", "Test Sans", "Bold", true, false)
            ]
        );
        assert_eq!(
            r.get(1).map(|r| unpack(r, 100)),
            Some(Ok(b"bold font".to_vec()))
        );
        Ok(())
    }

    #[test]
    fn hands_over_only_the_fonts_texts_use_each_once() {
        let data = two_fonts();
        let texts = [
            text(true, false, "test sans"),
            text(true, false, "Test Sans"),
        ];
        let mut notes = Vec::new();
        let fonts = read(None, Some(&data), &texts, &mut notes);
        let got: Vec<_> = fonts
            .iter()
            .map(|f| (f.style.as_str(), f.data.as_slice()))
            .collect();
        assert_eq!(got, [("Bold", b"bold font".as_slice())]);
        let messages: Vec<_> = notes.iter().map(|n| n.message.as_str()).collect();
        assert_eq!(
            messages,
            ["1 embedded font(s) that no text uses were not handed over"]
        );
    }

    #[test]
    fn a_font_the_board_does_not_embed_gets_a_note() {
        let data = two_fonts();
        let texts = [text(false, true, "Test Sans"), text(false, false, "Arial")];
        let mut notes = Vec::new();
        let fonts = read(None, Some(&data), &texts, &mut notes);
        assert!(fonts.is_empty());
        let messages: Vec<_> = notes.iter().map(|n| n.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "1 text(s) use the TrueType font \"Arial\", which the board does not embed; \
                 their position, height and rotation are read",
                "1 text(s) use the TrueType font \"Test Sans\" italic, which the board does not \
                 embed; their position, height and rotation are read",
                "2 embedded font(s) that no text uses were not handed over",
            ]
        );
    }

    fn warnings(notes: &[Note]) -> Vec<&str> {
        notes
            .iter()
            .filter(|n| n.severity == Severity::Warning)
            .map(|n| n.message.as_str())
            .collect()
    }

    #[test]
    fn a_font_that_unpacks_too_big_gets_one_note_for_all_its_spellings() {
        let data = two_fonts();
        // "Test Sans" and "test sans" are the same font, so it is tried and noted once.
        let texts = [
            text(false, false, "Test Sans"),
            text(false, false, "test sans"),
        ];
        let mut notes = Vec::new();
        let fonts = read_within(None, Some(&data), &texts, 5, &mut notes);
        assert!(fonts.is_empty());
        assert_eq!(
            warnings(&notes),
            [
                "the embedded font \"Test Sans\" could not be unpacked (it unpacks to more than 5 \
                 bytes; 5 of the 5 bytes allowed for all fonts were left for it); 2 text(s) use \
                 it, and their position, height and rotation are read"
            ]
        );
    }

    #[test]
    fn the_fonts_share_one_budget_and_the_note_names_what_was_left() {
        let data = two_fonts();
        let texts = [
            text(false, false, "Test Sans"),
            text(true, false, "Test Sans"),
        ];
        let mut notes = Vec::new();
        // "regular font" takes 12 of the 15 bytes, so "bold font" (9) does not fit in the 3 left.
        let fonts = read_within(None, Some(&data), &texts, 15, &mut notes);
        let got: Vec<_> = fonts.iter().map(|f| f.style.as_str()).collect();
        assert_eq!(got, ["Book"]);
        let w = warnings(&notes);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(
            w.iter().all(|m| m.starts_with(
                "the embedded font \"Test Sans\" bold could not be unpacked (it unpacks to more \
                 than 3 bytes; 3 of the 15 bytes"
            )),
            "{w:?}"
        );
    }

    #[test]
    fn a_header_count_that_differs_from_the_data_gets_a_warning() {
        let data = two_fonts();
        let texts = [text(false, false, "Test Sans")];
        let count = |header: Option<&[u8]>, data: Option<&[u8]>| {
            let mut notes = Vec::new();
            read(header, data, &texts, &mut notes);
            warnings(&notes).join("; ")
        };
        assert_eq!(count(Some(&2u32.to_le_bytes()), Some(&data)), "");
        assert_eq!(count(Some(&0u32.to_le_bytes()), None), "");
        assert_eq!(count(None, Some(&data)), "");
        assert_eq!(
            count(Some(&3u32.to_le_bytes()), Some(&data)),
            "/EmbeddedFonts6/Header says the board embeds 3 font(s), but /EmbeddedFonts6/Data \
             holds 2; the fonts in the data are used"
        );
        assert_eq!(
            count(Some(&1u32.to_le_bytes()), None),
            "/EmbeddedFonts6/Header says the board embeds 1 font(s), but /EmbeddedFonts6/Data \
             holds 0; the fonts in the data are used"
        );
        assert_eq!(
            count(Some(&[2, 0]), Some(&data)),
            "/EmbeddedFonts6/Header is 2 bytes, not a 4-byte font count; the 2 font(s) in \
             /EmbeddedFonts6/Data are used"
        );
    }

    #[test]
    fn damaged_streams_are_refused_with_a_note() {
        let data = two_fonts();
        for cut in [1, 10, data.len().saturating_sub(1)] {
            let damaged = data.get(..cut).unwrap_or_default();
            assert!(records(damaged).is_err(), "cut at {cut}");
            let mut notes = Vec::new();
            let fonts = read(
                None,
                Some(damaged),
                &[text(false, false, "Test Sans")],
                &mut notes,
            );
            assert!(fonts.is_empty());
            assert!(notes
                .iter()
                .any(|n| n.message.contains("could not be read")));
        }
        let mut odd = data.clone();
        odd.splice(0..4, 3u32.to_le_bytes());
        assert!(records(&odd).is_err());
        assert!(records(&u32::MAX.to_le_bytes()).is_err());
    }

    #[test]
    fn describes_the_style() {
        assert_eq!(describe("A", false, false), "\"A\"");
        assert_eq!(describe("A", true, true), "\"A\" bold italic");
    }
}
