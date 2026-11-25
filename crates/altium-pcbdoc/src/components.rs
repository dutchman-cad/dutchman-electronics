//! Roadmap step 5, first part: component placements (designator, footprint, position, rotation,
//! side).
//!
//! Worked out from our own CC0 test board `one-resistor` (one component: designator R1, comment
//! "10k", footprint R0603-TEST, centre (25, 15) mm, rotation 0, top side) by comparing its streams
//! with `empty-outline`, and from the numbers in its README.txt (see LEGAL.md):
//!
//! - `/Components6/Data` is a run of records, each a little-endian `u32` length and then text of
//!   the form `|KEY=VALUE…` ending in a NUL byte (the framing of `/Board6/Data`). Our board has one
//!   record, with `X=984.252mil` and `Y=590.5512mil` (the centre, 25 x 15 mm),
//!   `ROTATION= 0.00000000000000E+0000` (degrees), `LAYER=TOP` and `PATTERN=R0603-TEST` (the
//!   footprint). The designator is not in this record. `one-rotated-resistor` has
//!   `ROTATION= 9.00000000000000E+0001` (90 degrees, counter-clockwise); `one-bottom-resistor`
//!   has `LAYER=BOTTOM` and `ROTATION= 1.80000000000000E+0002`, the 180 degrees Altium itself
//!   gives a part when it is flipped to the bottom.
//! - A component is referred to by its place in that run, counting from 0. In the record bodies of
//!   `/Tracks6/Data`, `/Pads6/Data` (geometry block) and `/Texts6/Data`, bytes 7..9 hold it as a
//!   `u16`, `ff ff` for none: the resistor's two pads and its two texts hold 0, the free pad in
//!   `outline-with-hole` and the track in `one-track` hold `ff ff`.
//! - `/Texts6/Data` is a run of records: a type byte `0x05`, a `u32` length and a body (252 bytes
//!   on our board), then a `u32` length and a block holding one length byte and the text in 8-bit
//!   characters. Body byte 0 is the layer (33, Top Overlay, on our board). Byte 40 is 1 on the
//!   comment text ("10k") and 0 on the designator; byte 41 is 1 on the designator text ("R1") and
//!   0 on the comment. Bytes 115..119 are a `u32` index into `/WideStrings6/Data` (0 for "10k",
//!   1 for "R1").
//! - `/WideStrings6/Data` is a run of entries: a `u32` index, a `u32` byte length, and that many
//!   bytes of UTF-16LE text ending in a NUL character. The designator is taken from there when
//!   its index is found (UTF-16 holds any character), otherwise from the 8-bit copy.

use std::collections::BTreeMap;

use board_model::{Note, Placement, Point, Severity, Side};

use crate::board::Properties;
use crate::copper::{read_u16, read_u32};

/// The most records or entries read from one stream before the file is treated as hostile.
const MAX_RECORDS: usize = 1_000_000;
pub(crate) const TEXT_RECORD: u8 = 0x05;
/// Where a record body holds the index of the component it belongs to.
pub(crate) const COMPONENT_AT: usize = 7;
pub(crate) const NO_COMPONENT: u16 = 0xffff;
/// The byte that is 1 on a component's designator text.
pub(crate) const DESIGNATOR_FLAG_AT: usize = 41;
/// Body byte that is 1 on a component's comment text ("10k" on `one-resistor`).
pub(crate) const COMMENT_FLAG_AT: usize = 40;
/// Where a text body holds its index into `/WideStrings6/Data`.
pub(crate) const WIDE_STRING_AT: usize = 115;
/// Rotations beyond this many degrees (either way) are refused as not real.
const MAX_ROTATION_DEG: f64 = 360.0;

/// The components of a board.
#[derive(Debug, Default)]
pub(crate) struct Components {
    pub placements: Vec<Placement>,
    /// The designator of each component, by its place in `/Components6/Data` (the index that
    /// pads, tracks and texts refer to). Also kept for a component that was not placed.
    pub designators: Vec<Option<String>>,
    /// Whether each component shows its designator text (`NAMEON=TRUE`) and its comment text
    /// (`COMMENTON=TRUE`), by the same index.
    pub name_on: Vec<bool>,
    pub comment_on: Vec<bool>,
    /// For each component (same index), its place in `placements`, if it was placed.
    pub placement_of: Vec<Option<usize>>,
}

impl Components {
    /// The designator of the component an object's body refers to at bytes 7..9, if any.
    pub(crate) fn owner(&self, body: &[u8]) -> Option<String> {
        let index = read_u16(body, COMPONENT_AT)?;
        if index == NO_COMPONENT {
            return None;
        }
        self.designators.get(usize::from(index)).cloned().flatten()
    }
}

/// Reads the components from `/Components6/Data`, with their designators from `/Texts6/Data` and
/// `/WideStrings6/Data`. A component that cannot be read is left out with a note; it does not stop
/// the board from being read.
pub(crate) fn read(
    components6: &[u8],
    texts6: Option<&[u8]>,
    wide_strings6: Option<&[u8]>,
    notes: &mut Vec<Note>,
) -> Result<Components, String> {
    let records = text_records(components6, "/Components6/Data")?;
    let wide = match wide_strings6 {
        Some(data) => wide_strings(data)?,
        None => BTreeMap::new(),
    };
    let mut designators = vec![None; records.len()];
    if let Some(data) = texts6 {
        designator_texts(data, &wide, &mut designators, notes)?;
    }
    let mut placements = Vec::new();
    let mut placement_of = vec![None; records.len()];
    let mut unnamed = 0usize;
    for (index, record) in records.iter().enumerate() {
        let designator = designators.get(index).cloned().flatten();
        let label = designator
            .clone()
            .unwrap_or_else(|| format!("component {}", index.saturating_add(1)));
        match placement(record, designator.clone().unwrap_or_default()) {
            Ok(p) => {
                if designator.is_none() {
                    unnamed += 1;
                }
                if let Some(slot) = placement_of.get_mut(index) {
                    *slot = Some(placements.len());
                }
                placements.push(p);
            }
            Err(why) => notes.push(Note {
                severity: Severity::Dropped,
                message: format!("{label} was not read: {why}"),
            }),
        }
    }
    if unnamed > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!("{unnamed} component(s) have no designator text; left blank"),
        });
    }
    let flag = |r: &Properties, key: &str| r.get(key).map(str::trim) == Some("TRUE");
    Ok(Components {
        placements,
        designators,
        name_on: records.iter().map(|r| flag(r, "NAMEON")).collect(),
        comment_on: records.iter().map(|r| flag(r, "COMMENTON")).collect(),
        placement_of,
    })
}

/// One component record. `Err` says why it was left out.
fn placement(record: &Properties, designator: String) -> Result<Placement, String> {
    let x = record.length_mm("X")?.ok_or("it has no X")?;
    let y = record.length_mm("Y")?.ok_or("it has no Y")?;
    let rotation_deg = match record.get("ROTATION") {
        Some(text) => text
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|r| r.is_finite() && r.abs() <= MAX_ROTATION_DEG)
            .ok_or_else(|| format!("ROTATION={text} is not an angle"))?,
        None => return Err("it has no ROTATION".into()),
    };
    let side = match record.get("LAYER").map(str::trim) {
        Some("TOP") => Side::Top,
        Some("BOTTOM") => Side::Bottom,
        Some(other) => return Err(format!("LAYER={other} is not a side this reader knows")),
        None => return Err("it has no LAYER".into()),
    };
    Ok(Placement {
        designator,
        footprint: record.get("PATTERN").unwrap_or_default().to_string(),
        at: Point { x, y },
        rotation_deg,
        side,
        model: None,
        comment: String::new(),
    })
}

/// Splits a run of `[u32 length][text]` records into their key-value pairs.
pub(crate) fn text_records(data: &[u8], stream: &str) -> Result<Vec<Properties>, String> {
    let mut records = Vec::new();
    let mut at = 0usize;
    while at < data.len() {
        if records.len() >= MAX_RECORDS {
            return Err(format!("{stream} has more than {MAX_RECORDS} records"));
        }
        let len = read_u32(data, at).ok_or_else(|| format!("{stream}: length cut short"))?;
        let (start, end) = span(at, 4, len, data.len())
            .ok_or_else(|| format!("{stream}: record is longer than the stream"))?;
        let text = data
            .get(start..end)
            .ok_or_else(|| format!("{stream}: record out of range"))?;
        records.push(Properties::parse(text));
        at = end;
    }
    Ok(records)
}

/// The UTF-16 texts of `/WideStrings6/Data`, by index.
pub(crate) fn wide_strings(data: &[u8]) -> Result<BTreeMap<usize, String>, String> {
    let mut strings = BTreeMap::new();
    let mut at = 0usize;
    let mut count = 0usize;
    while at < data.len() {
        count += 1;
        if count > MAX_RECORDS {
            return Err(format!(
                "/WideStrings6/Data has more than {MAX_RECORDS} entries"
            ));
        }
        let index = read_u32(data, at).ok_or("/WideStrings6/Data: index cut short")?;
        let len_at = at
            .checked_add(4)
            .ok_or("/WideStrings6/Data: out of range")?;
        let len = read_u32(data, len_at).ok_or("/WideStrings6/Data: length cut short")?;
        let (start, end) = span(len_at, 4, len, data.len())
            .ok_or("/WideStrings6/Data: entry is longer than the stream")?;
        let bytes = data
            .get(start..end)
            .ok_or("/WideStrings6/Data: entry out of range")?;
        let units = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .take_while(|&u| u != 0);
        let text: String = char::decode_utf16(units)
            .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect();
        strings.entry(index).or_insert(text);
        at = end;
    }
    Ok(strings)
}

/// Fills `designators` from the designator texts in `/Texts6/Data`.
fn designator_texts(
    data: &[u8],
    wide: &BTreeMap<usize, String>,
    designators: &mut [Option<String>],
    notes: &mut Vec<Note>,
) -> Result<(), String> {
    let mut at = 0usize;
    let mut count = 0usize;
    while at < data.len() {
        count += 1;
        if count > MAX_RECORDS {
            return Err(format!("/Texts6/Data has more than {MAX_RECORDS} records"));
        }
        let kind = data.get(at).copied().ok_or("text record cut short")?;
        if kind != TEXT_RECORD {
            notes.push(Note {
                severity: Severity::Dropped,
                message: format!(
                    "/Texts6/Data: unknown record type {kind:#04x} at byte {at}; designators after \
                     it were not read"
                ),
            });
            break;
        }
        let len_at = at.checked_add(1).ok_or("/Texts6/Data: out of range")?;
        let len = read_u32(data, len_at).ok_or("/Texts6/Data: length cut short")?;
        let (start, end) = span(len_at, 4, len, data.len())
            .ok_or("/Texts6/Data: record is longer than the stream")?;
        let body = data.get(start..end).ok_or("/Texts6/Data: out of range")?;
        let text_len = read_u32(data, end).ok_or("/Texts6/Data: text length cut short")?;
        let (text_start, text_end) = span(end, 4, text_len, data.len())
            .ok_or("/Texts6/Data: text is longer than the stream")?;
        let block = data
            .get(text_start..text_end)
            .ok_or("/Texts6/Data: text out of range")?;
        at = text_end;

        if body.get(DESIGNATOR_FLAG_AT) != Some(&1) {
            continue;
        }
        let Some(component) = read_u16(body, COMPONENT_AT).filter(|&c| c != NO_COMPONENT) else {
            continue;
        };
        let from_wide = read_u32(body, WIDE_STRING_AT)
            .and_then(|i| wide.get(&i))
            .cloned();
        let text = match from_wide {
            Some(t) => t,
            None => short_string(block).ok_or("/Texts6/Data: text block cut short")?,
        };
        if let Some(slot) = designators.get_mut(usize::from(component)) {
            slot.get_or_insert(text);
        }
    }
    Ok(())
}

/// A block holding one length byte and that many 8-bit (Latin-1) characters.
pub(crate) fn short_string(block: &[u8]) -> Option<String> {
    let (&len, rest) = block.split_first()?;
    let bytes = rest.get(..usize::from(len))?;
    Some(bytes.iter().map(|&b| char::from(b)).collect())
}

/// The start and end of `len` bytes that follow a `header`-byte field at `at`, if they fit in
/// `total` bytes.
pub(crate) fn span(at: usize, header: usize, len: usize, total: usize) -> Option<(usize, usize)> {
    let start = at.checked_add(header)?;
    let end = start.checked_add(len)?;
    (end <= total).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(text: &str) -> Vec<u8> {
        let mut bytes = (text.len() as u32 + 1).to_le_bytes().to_vec();
        bytes.extend_from_slice(text.as_bytes());
        bytes.push(0);
        bytes
    }

    /// A text record whose body says it is the designator of component `component`.
    fn designator_text(component: u16, wide: u32, text: &str) -> Vec<u8> {
        let mut body = vec![0u8; 252];
        body[COMPONENT_AT..COMPONENT_AT + 2].copy_from_slice(&component.to_le_bytes());
        body[DESIGNATOR_FLAG_AT] = 1;
        body[WIDE_STRING_AT..WIDE_STRING_AT + 4].copy_from_slice(&wide.to_le_bytes());
        let mut out = vec![TEXT_RECORD];
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out.extend_from_slice(&(text.len() as u32 + 1).to_le_bytes());
        out.push(text.len() as u8);
        out.extend_from_slice(text.as_bytes());
        out
    }

    const R1: &str = "|LAYER=TOP|X=984.252mil|Y=590.5512mil|PATTERN=R0603-TEST\
                      |ROTATION= 0.00000000000000E+0000";

    #[test]
    fn a_component_with_its_designator_text() {
        let mut notes = Vec::new();
        let texts = designator_text(0, 7, "R1");
        let c = read(&record(R1), Some(&texts), None, &mut notes).unwrap();
        assert_eq!(c.placements.len(), 1);
        let p = &c.placements[0];
        assert_eq!(p.designator, "R1");
        assert_eq!(p.footprint, "R0603-TEST");
        assert!((p.at.x - 25.0).abs() < 0.001 && (p.at.y - 15.0).abs() < 0.001);
        assert_eq!((p.rotation_deg, p.side), (0.0, Side::Top));
        assert!(notes.is_empty(), "{notes:?}");
    }

    #[test]
    fn the_wide_string_wins_over_the_8_bit_copy() {
        let mut wide = 7u32.to_le_bytes().to_vec();
        let utf16: Vec<u8> = "Ω1\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
        wide.extend_from_slice(&(utf16.len() as u32).to_le_bytes());
        wide.extend_from_slice(&utf16);
        let mut notes = Vec::new();
        let texts = designator_text(0, 7, "?1");
        let c = read(&record(R1), Some(&texts), Some(&wide), &mut notes).unwrap();
        assert_eq!(c.placements[0].designator, "Ω1");
        assert_eq!(c.owner(&[0, 0, 0, 0, 0, 0, 0, 0, 0]), Some("Ω1".into()));
        assert_eq!(c.owner(&[0, 0, 0, 0, 0, 0, 0, 0xff, 0xff]), None);
    }

    #[test]
    fn a_bad_component_costs_that_component_not_the_board() {
        let mut data = record("|LAYER=TOP|X=0mil|Y=0mil|ROTATION=NaN");
        data.extend(record("|LAYER=INNER|X=0mil|Y=0mil|ROTATION=0"));
        data.extend(record("|LAYER=TOP|Y=0mil|ROTATION=0"));
        data.extend(record("|LAYER=BOTTOM|X=0mil|Y=0mil|ROTATION=90"));
        let mut notes = Vec::new();
        let c = read(&data, None, None, &mut notes).unwrap();
        assert_eq!(c.placements.len(), 1);
        assert_eq!(c.placements[0].side, Side::Bottom);
        assert_eq!(c.designators.len(), 4);
        let dropped = notes
            .iter()
            .filter(|n| n.severity == Severity::Dropped)
            .count();
        assert_eq!(dropped, 3, "{notes:?}");
        // A bottom-side part is confirmed by one-bottom-resistor: no warning any more.
        assert!(!notes.iter().any(|n| n.message.contains("LAYER=BOTTOM")));
        assert!(notes.iter().any(|n| n.message.contains("no designator")));
    }

    #[test]
    fn hostile_streams_are_errors_not_panics() {
        let mut notes = Vec::new();
        assert!(read(&[0xff, 0xff, 0xff, 0x7f], None, None, &mut notes).is_err());
        assert!(read(&[1, 0], None, None, &mut notes).is_err());
        assert!(read(
            &[],
            Some(&[TEXT_RECORD, 0xff, 0xff, 0xff, 0x7f]),
            None,
            &mut notes
        )
        .is_err());
        // A text record whose 8-bit block is missing.
        let mut cut = designator_text(0, 9, "R1");
        cut.truncate(cut.len() - 4);
        assert!(read(&record(R1), Some(&cut), None, &mut notes).is_err());
        // A text block whose length byte runs past the block.
        let mut long = designator_text(0, 9, "R1");
        let n = long.len();
        long[n - 3] = 200;
        assert!(read(&record(R1), Some(&long), None, &mut notes).is_err());
        assert!(read(
            &[],
            None,
            Some(&[0, 0, 0, 0, 0xff, 0xff, 0xff, 0x7f]),
            &mut notes
        )
        .is_err());
        assert!(read(&[], None, Some(&[0, 0]), &mut notes).is_err());
        // An unknown text record type stops the designators, with a note.
        let c = read(&record(R1), Some(&[0x42, 1, 2]), None, &mut notes).unwrap();
        assert_eq!(c.placements.len(), 1);
        assert!(notes.iter().any(|n| n.severity == Severity::Dropped));
    }
}
