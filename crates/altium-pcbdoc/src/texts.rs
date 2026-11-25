//! Roadmap step 4: texts on the overlay (silkscreen) and copper layers.
//!
//! Worked out from our own CC0 test boards `one-text`, `one-rotated-text` and `one-bottom-text`
//! (one stroke-font text each, 2.000 mm high, stroke 0.200) and the two component texts of
//! `one-resistor`, compared with `empty-outline` and the numbers in their README.txt (see
//! LEGAL.md). The record framing, the component index and the flags at bytes 40 and 41 are
//! described in `components.rs`. In the record body:
//!
//! - byte 0 is the layer (33 Top Overlay, 34 Bottom Overlay; `/Board6/Data` names them in
//!   `LAYER33NAME` and `LAYER34NAME`),
//! - bytes 13..21 are the position (x, y), as `i32` in 1/10000 mil: the text's reference point
//!   as Altium shows it in the text's properties, (10, 10) on `one-text`,
//! - bytes 21..25 are the height (2.000 on all three boards),
//! - bytes 27..35 are the rotation as an `f64` in degrees (90.0 on `one-rotated-text`, 0.0 on the
//!   others),
//! - byte 35 is 1 when the text is mirrored (`one-bottom-text`) and 0 otherwise,
//! - bytes 36..40 are the stroke width (0.200 on all three),
//! - byte 43 is the font type: 0 for the stroke font, 1 for a TrueType font (`one-ttf-text`),
//! - bytes 44 and 45 are 1 when the TrueType font is bold (`one-dejavu-bold-text`) or italic
//!   (`one-dejavu-italic-text`), and 0 otherwise,
//! - bytes 46..110 are the TrueType font's name in UTF-16, ending at the first 0 ("Arial" on
//!   `one-ttf-text`; left-over bytes on the stroke boards, so only read when byte 43 is 1).
//!
//! A TrueType text keeps its position, height and rotation in the same bytes as a stroke text
//! (`one-ttf-text`: (10, 10), 2.000, 0, as Altium shows them). Its letter shapes come from the
//! font file, which Altium embeds in `/EmbeddedFonts6` (`fonts.rs`; `one-ttf-text` has it
//! removed, see its README.txt).
//!
//! The content is the UTF-16 entry in `/WideStrings6/Data` that bytes 115..119 point to, or the
//! 8-bit copy in the block after the body. A component's designator text is shown only when its
//! component has `NAMEON=TRUE`, its comment text only with `COMMENTON=TRUE` (`one-resistor`
//! shows "R1" and hides "10k").

use std::collections::BTreeMap;

use board_model::{Note, Point, Severity, Text, TextFont};

use crate::components::{
    short_string, span, Components, COMMENT_FLAG_AT, COMPONENT_AT, DESIGNATOR_FLAG_AT,
    NO_COMPONENT, TEXT_RECORD, WIDE_STRING_AT,
};
use crate::copper::{read_u16, read_u32, CopperLayers};

/// Millimetres per internal unit (1/10000 mil).
const MM_PER_UNIT: f64 = 0.0254 / 10_000.0;
/// The most records read from `/Texts6/Data` before the file is treated as hostile.
const MAX_RECORDS: usize = 1_000_000;
const MIRROR_AT: usize = 35;
const STROKE_WIDTH_AT: usize = 36;
const FONT_TYPE_AT: usize = 43;
const TRUETYPE: u8 = 1;
const BOLD_AT: usize = 44;
const ITALIC_AT: usize = 45;
const FONT_NAME: std::ops::Range<usize> = 46..110;
/// Rotations beyond this many degrees (either way) are refused as not real.
const MAX_ROTATION_DEG: f64 = 360.0;

/// The texts of `/Texts6/Data`, and each placed component's comment.
pub(crate) struct Texts {
    /// The visible texts on copper and overlay layers.
    pub texts: Vec<Text>,
    /// (index into the placements, comment): the string of each component's comment text, also
    /// when the component hides it (`COMMENTON=FALSE`), for `.Comment` (`special_strings.rs`).
    pub comments: Vec<(usize, String)>,
}

/// Reads the visible texts of `/Texts6/Data` on copper and overlay layers. A text that cannot be
/// read, or sits on another layer, is left out with a note.
pub(crate) fn read(
    data: &[u8],
    wide: &BTreeMap<usize, String>,
    components: &Components,
    layers: &mut CopperLayers,
    notes: &mut Vec<Note>,
) -> Result<Texts, String> {
    let mut texts = Vec::new();
    let mut comments = Vec::new();
    let mut other_layers: BTreeMap<String, usize> = BTreeMap::new();
    let mut unreadable = 0usize;
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
                    "/Texts6/Data: unknown record type {kind:#04x} at byte {at}; the texts after \
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

        let content = read_u32(body, WIDE_STRING_AT)
            .and_then(|i| wide.get(&i))
            .cloned()
            .or_else(|| short_string(block));
        let component = read_u16(body, COMPONENT_AT)
            .filter(|&c| c != NO_COMPONENT)
            .and_then(|c| {
                components
                    .placement_of
                    .get(usize::from(c))
                    .copied()
                    .flatten()
            });
        if body.get(COMMENT_FLAG_AT) == Some(&1) {
            if let (Some(p), Some(c)) = (component, &content) {
                comments.push((p, c.clone()));
            }
        }
        if !visible(body, components) {
            continue;
        }
        let Some(&layer_number) = body.first() else {
            unreadable += 1;
            continue;
        };
        let Some(layer) = layers.text_layer(layer_number) else {
            *other_layers
                .entry(layers.layer_name(layer_number))
                .or_default() += 1;
            continue;
        };
        match (content, geometry(body)) {
            (Some(content), Some((at_point, height, rotation_deg, mirrored, stroke_width))) => {
                let font = if body.get(FONT_TYPE_AT) == Some(&TRUETYPE) {
                    TextFont::TrueType {
                        name: font_name(body),
                        bold: body.get(BOLD_AT) == Some(&1),
                        italic: body.get(ITALIC_AT) == Some(&1),
                    }
                } else {
                    TextFont::Stroke {
                        width: stroke_width,
                    }
                };
                texts.push(Text {
                    layer,
                    at: at_point,
                    content,
                    height,
                    rotation_deg,
                    mirrored,
                    font,
                    component,
                });
            }
            _ => unreadable += 1,
        }
    }
    for (layer, n) in other_layers {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!("{n} text(s) on {layer} (not a copper or overlay layer) not read yet"),
        });
    }
    if unreadable > 0 {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!("{unreadable} text(s) could not be read"),
        });
    }
    Ok(Texts { texts, comments })
}

/// False for a component's designator or comment text that its component hides.
fn visible(body: &[u8], components: &Components) -> bool {
    let Some(index) = read_u16(body, COMPONENT_AT).filter(|&c| c != NO_COMPONENT) else {
        return true;
    };
    let index = usize::from(index);
    if body.get(DESIGNATOR_FLAG_AT) == Some(&1) {
        return components.name_on.get(index).copied().unwrap_or(true);
    }
    if body.get(COMMENT_FLAG_AT) == Some(&1) {
        return components.comment_on.get(index).copied().unwrap_or(true);
    }
    true
}

/// The TrueType font's name (UTF-16 up to the first 0), or "?" when it is missing or not text.
fn font_name(body: &[u8]) -> String {
    let units: Vec<u16> = body
        .get(FONT_NAME)
        .unwrap_or_default()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .take_while(|&u| u != 0)
        .collect();
    match String::from_utf16(&units) {
        Ok(name) if !name.trim().is_empty() => name,
        _ => "?".to_string(),
    }
}

/// Position, height, rotation, mirroring and stroke width, or `None` when the body is too short
/// or the rotation is not a real angle.
fn geometry(body: &[u8]) -> Option<(Point, f64, f64, bool, f64)> {
    let i32_at = |at: usize| -> Option<f64> {
        let bytes: [u8; 4] = body.get(at..at.checked_add(4)?)?.try_into().ok()?;
        Some(f64::from(i32::from_le_bytes(bytes)) * MM_PER_UNIT)
    };
    let rotation_bytes: [u8; 8] = body.get(27..35)?.try_into().ok()?;
    let rotation = f64::from_le_bytes(rotation_bytes);
    if !rotation.is_finite() || rotation.abs() > MAX_ROTATION_DEG {
        return None;
    }
    Some((
        Point {
            x: i32_at(13)?,
            y: i32_at(17)?,
        },
        i32_at(21)?,
        rotation,
        *body.get(MIRROR_AT)? == 1,
        i32_at(STROKE_WIDTH_AT)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_streams_are_errors_or_notes_not_panics() {
        let mut layers = CopperLayers::new(&crate::board::Properties::parse(
            b"|LAYER33NAME=Top Overlay",
        ));
        let none = Components::default();
        let wide = BTreeMap::new();
        let mut notes = Vec::new();
        assert!(read(&[TEXT_RECORD], &wide, &none, &mut layers, &mut notes).is_err());
        assert!(read(
            &[TEXT_RECORD, 0xff, 0xff, 0xff, 0x7f],
            &wide,
            &none,
            &mut layers,
            &mut notes
        )
        .is_err());
        // A short body on Top Overlay: reported as unreadable, not a panic.
        let mut short = vec![TEXT_RECORD, 1, 0, 0, 0, 33];
        short.extend_from_slice(&0u32.to_le_bytes());
        let texts = read(&short, &wide, &none, &mut layers, &mut notes).unwrap();
        assert!(texts.texts.is_empty());
        assert!(notes
            .iter()
            .any(|n| n.message.contains("could not be read")));
        assert!(read(&[0x42], &wide, &none, &mut layers, &mut notes)
            .unwrap()
            .texts
            .is_empty());
    }
}
