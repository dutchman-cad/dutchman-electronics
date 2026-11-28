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
//! - byte 25 is the stroke font: 1 Default, 2 Sans Serif, 3 Serif (`stroke-fonts`, the same
//!   texts in each, Altium's own names and numbers; 1 on every other board),
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
//! 8-bit copy in the block after the body.
//!
//! Frames and inverted texts (`text-kinds`, twelve DejaVu Sans Mono texts): a text frame stores
//! its location as the frame's lower-left corner (LB at (3, 4), CM at (18, 10.488) of a 10 x 4
//! frame), and Altium sets the letters inside it by its justification, so the text is placed at
//! the frame's point for that justification (the corner, the middle of a side or the center) and
//! anchored there by the same part of its own box. Where inside the frame Altium puts the letters
//! exactly is not measured; the frame and its point are the file's. An inverted text (its
//! letters cut out of a filled box) is drawn as plain letters, with a note: the board model has
//! filled areas, but the letters' shapes come from the font only when the package draws them. A text with no characters (`empty-texts`) shows
//! nothing in Altium and is left out without a note. A component's designator text is shown only when its
//! component has `NAMEON=TRUE`, its comment text only with `COMMENTON=TRUE` (`one-resistor`
//! shows "R1" and hides "10k").

use std::collections::BTreeMap;

use board_model::{
    HorizontalAnchor, Note, Point, Severity, StrokeFont, Text, TextAnchor, TextFont, VerticalAnchor,
};

use crate::components::{
    short_string, span, Components, COMMENT_FLAG_AT, COMPONENT_AT, DESIGNATOR_FLAG_AT,
    NO_COMPONENT, TEXT_RECORD, WIDE_STRING_AT,
};
use crate::copper::{read_u16, read_u32, CopperLayers};

/// Millimeters per internal unit (1/10000 mil).
const MM_PER_UNIT: f64 = 0.0254 / 10_000.0;
/// The most records read from `/Texts6/Data` before the file is treated as hostile.
const MAX_RECORDS: usize = 1_000_000;
const MIRROR_AT: usize = 35;
const STROKE_WIDTH_AT: usize = 36;
const FONT_TYPE_AT: usize = 43;
/// The byte naming a stroke text's font (see [`stroke_font`]).
const STROKE_FONT_AT: usize = 25;
const TRUETYPE: u8 = 1;
const BOLD_AT: usize = 44;
const ITALIC_AT: usize = 45;
const FONT_NAME: std::ops::Range<usize> = 46..110;
/// Rotations beyond this many degrees (either way) are refused as not real.
const MAX_ROTATION_DEG: f64 = 360.0;
/// `text-kinds`: byte 110 is 1 on an inverted text, whose letters are cut from a filled box; the
/// box reaches the margin at bytes 111..115 (0.500 on INV) past the text box, or, when byte 231
/// is 1 (Border Mode Offset), the offset at bytes 133..137 (0.300 on OFS).
const INVERTED_AT: usize = 110;
const INVERT_MARGIN_AT: usize = 111;
const INVERT_OFFSET_AT: usize = 133;
const INVERT_BY_OFFSET_AT: usize = 231;
/// `text-kinds`: byte 230 is 1 on a text frame, whose width and height are at bytes 124..128 and
/// 128..132 (10.000 and 4.000; on a plain string they hold Altium's own text box), and byte 132
/// is the justification: 1, 2, 3 left top, middle, bottom; 4, 5, 6 centre; 7, 8, 9 right (3 on
/// every plain string).
const FRAME_AT: usize = 230;
const FRAME_WIDTH_AT: usize = 124;
const FRAME_HEIGHT_AT: usize = 128;
const JUSTIFICATION_AT: usize = 132;

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
    let mut inverted: Vec<String> = Vec::new();
    let mut unreadable = 0usize;
    let mut unknown_fonts = 0usize;
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
        // An empty text shows nothing (`empty-texts`), so it is left out, wherever it lies.
        if !visible(body, components) || content.as_deref() == Some("") {
            continue;
        }
        let Some(&layer_number) = body.first() else {
            unreadable += 1;
            continue;
        };
        let Some(layer) = layers.text_layer(layer_number) else {
            *other_layers
                .entry(layers.off_copper_label(layer_number))
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
                        font: stroke_font(body.get(STROKE_FONT_AT).copied(), &mut unknown_fonts),
                    }
                };
                if body.get(INVERTED_AT) == Some(&1) {
                    let by_offset = body.get(INVERT_BY_OFFSET_AT) == Some(&1);
                    let border = i32_mm(
                        body,
                        if by_offset {
                            INVERT_OFFSET_AT
                        } else {
                            INVERT_MARGIN_AT
                        },
                    );
                    inverted.push(format!(
                        "\"{content}\" at ({:.3}, {:.3}), {} {:.3}",
                        at_point.x,
                        at_point.y,
                        if by_offset { "offset" } else { "margin" },
                        border.unwrap_or_default()
                    ));
                }
                let (at_point, anchor) = framed(body, at_point, rotation_deg, mirrored)
                    .unwrap_or((at_point, TextAnchor::default()));
                texts.push(Text {
                    layer,
                    at: at_point,
                    content,
                    height,
                    rotation_deg,
                    mirrored,
                    anchor,
                    line_pitch: None,
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
            message: format!("{n} text(s) on {layer} not read yet"),
        });
    }
    if unreadable > 0 {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!("{unreadable} text(s) could not be read"),
        });
    }
    for what in &inverted {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "the inverted text {what} is drawn as plain letters: its letters cut out of a \
                 filled box are not drawn yet"
            ),
        });
    }
    if unknown_fonts > 0 {
        notes.push(Note {
            severity: Severity::Info,
            message: format!(
                "{unknown_fonts} stroke text(s) name a stroke font other than Default, Sans Serif \
                 or Serif; drawn in Default"
            ),
        });
    }
    Ok(Texts { texts, comments })
}

/// The stroke font byte 25 names: 1 Default, 2 Sans Serif, 3 Serif. Any other value (or none)
/// is counted in `unknown` and read as Default.
fn stroke_font(byte: Option<u8>, unknown: &mut usize) -> StrokeFont {
    match byte {
        Some(1) => StrokeFont::Default,
        Some(2) => StrokeFont::SansSerif,
        Some(3) => StrokeFont::Serif,
        _ => {
            *unknown += 1;
            StrokeFont::Default
        }
    }
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
/// An `i32` length in 1/10000 mil at `at`, in mm.
fn i32_mm(body: &[u8], at: usize) -> Option<f64> {
    let bytes: [u8; 4] = body.get(at..at + 4)?.try_into().ok()?;
    Some(f64::from(i32::from_le_bytes(bytes)) * MM_PER_UNIT)
}

/// A text frame's point for its justification, and the anchor of the text there; `None` for a
/// text that is not a frame. The frame lies from `corner` (its lower-left corner, the text's
/// stored location) `width` across and `height` up, in the text's own frame, turned and
/// mirrored with it.
fn framed(
    body: &[u8],
    corner: Point,
    rotation_deg: f64,
    mirrored: bool,
) -> Option<(Point, TextAnchor)> {
    if body.get(FRAME_AT) != Some(&1) {
        return None;
    }
    let width = i32_mm(body, FRAME_WIDTH_AT).filter(|w| w.is_finite() && *w > 0.0)?;
    let height = i32_mm(body, FRAME_HEIGHT_AT).filter(|h| h.is_finite() && *h > 0.0)?;
    let code = *body.get(JUSTIFICATION_AT)?;
    if !(1..=9).contains(&code) {
        return None;
    }
    let (column, row) = ((code - 1) / 3, (code - 1) % 3);
    let (horizontal, dx) = match column {
        0 => (HorizontalAnchor::Left, 0.0),
        1 => (HorizontalAnchor::Center, width / 2.0),
        _ => (HorizontalAnchor::Right, width),
    };
    let (vertical, dy) = match row {
        0 => (VerticalAnchor::Top, height),
        1 => (VerticalAnchor::Center, height / 2.0),
        _ => (VerticalAnchor::Bottom, 0.0),
    };
    let dx = if mirrored { -dx } else { dx };
    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    let at = Point {
        x: corner.x + dx * cos - dy * sin,
        y: corner.y + dx * sin + dy * cos,
    };
    Some((
        at,
        TextAnchor {
            horizontal,
            vertical,
        },
    ))
}

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
