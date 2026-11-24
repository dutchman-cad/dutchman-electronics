//! Roadmap step 3, first part: tracks and pads on copper layers.
//!
//! Worked out from our own CC0 test boards (`one-track`, `outline-with-hole`, `one-resistor`) and
//! the numbers in their README.txt (see LEGAL.md). Coordinates and sizes are little-endian `i32`
//! in 1/10000 mil, absolute (the boards' origin is at 0, 0).
//!
//! - A layer is one byte `n`; `/Board6/Data` names it in a `LAYERnNAME` key (`LAYER1NAME=Top
//!   Layer`, `LAYER74NAME=Multi-Layer` on our boards). A stack layer with a `COPTHICK` is copper.
//! - `/Tracks6/Data`: records of type `0x04`, a `u32` length, then a body whose byte 0 is the
//!   layer, bytes 3..5 the net index (`ff ff` for none), and from byte 13 the start x, start y,
//!   end x, end y and width.
//! - `/Pads6/Data`: records of type `0x02` followed by six blocks, each a `u32` length and its
//!   bytes. Block 4 holds the geometry: byte 0 the layer, bytes 3..5 the net index, from byte 13
//!   the centre x and y, then three (x, y) size pairs (top, middle, bottom), from byte 45 the hole
//!   diameter (0 for a pad without a hole), and at bytes 49..52 the shapes of the three size pairs
//!   (1 round, 2 rectangle). Bytes 52..60 are 0 on every pad we have; they are taken to be the
//!   rotation as an `f64` in degrees, which no board of ours confirms yet, so a non-zero value
//!   gets a warning. Bytes 7..9 are the index of the component the pad belongs to (see
//!   `components.rs`); its designator becomes the pad's owner.
//!
//! The file stores whole units, so a typed length can come out a fraction of a micrometre off
//! (50 mm is 19685039.37 units, stored as 19685039, read as 49.99999906 mm). That is the file, not
//! a rounding bug; tests allow 0.001 mm.

use board_model::{CopperItem, Layer, LayerKind, Note, PadShape, Point, Severity, Side};

use crate::board::Properties;
use crate::components::Components;

/// Millimetres per internal unit (1/10000 mil).
const MM_PER_UNIT: f64 = 0.0254 / 10_000.0;
const TRACK_RECORD: u8 = 0x04;
const PAD_RECORD: u8 = 0x02;
const PAD_BLOCKS: usize = 6;
const PAD_GEOMETRY_BLOCK: usize = 4;
/// The most records read from one stream before the file is treated as hostile.
const MAX_RECORDS: usize = 10_000_000;
/// The most layers named in `/Board6/Data` that are looked at.
const MAX_LAYER_NUMBER: u8 = u8::MAX;
const NO_NET: u16 = 0xffff;

/// The copper layers of a board, built from `/Board6/Data`.
pub(crate) struct CopperLayers {
    /// The layers in `Board::layers` order.
    pub layers: Vec<Layer>,
    /// For each layer byte (0..=255), its index in `layers`, if it is copper.
    by_number: Vec<Option<usize>>,
    /// For each layer byte, its name, when `/Board6/Data` names it.
    names: Vec<Option<String>>,
}

impl CopperLayers {
    pub(crate) fn new(board: &Properties) -> Self {
        // Copper layer names in stack order, from the V9 stack (a layer with COPTHICK is copper).
        let mut copper_names = Vec::new();
        for n in 0..10_000usize {
            let Some(name) = board.get(&format!("V9_STACK_LAYER{n}_NAME")) else {
                break;
            };
            if board.get(&format!("V9_STACK_LAYER{n}_COPTHICK")).is_some() {
                copper_names.push(name.to_string());
            }
        }
        let side_of = |name: &str| -> Option<Side> {
            let pos = copper_names.iter().position(|c| c == name)?;
            if pos == 0 {
                Some(Side::Top)
            } else if pos + 1 == copper_names.len() {
                Some(Side::Bottom)
            } else {
                None
            }
        };
        let mut layers = Vec::new();
        let mut by_number = vec![None; usize::from(MAX_LAYER_NUMBER) + 1];
        let mut names = vec![None; usize::from(MAX_LAYER_NUMBER) + 1];
        for n in 0..=MAX_LAYER_NUMBER {
            let Some(name) = board.get(&format!("LAYER{n}NAME")) else {
                continue;
            };
            if let Some(slot) = names.get_mut(usize::from(n)) {
                *slot = Some(name.to_string());
            }
            let multi = name == "Multi-Layer";
            if multi || copper_names.iter().any(|c| c == name) {
                if let Some(slot) = by_number.get_mut(usize::from(n)) {
                    *slot = Some(layers.len());
                }
                layers.push(Layer {
                    name: name.to_string(),
                    kind: LayerKind::Copper,
                    side: if multi { None } else { side_of(name) },
                });
            }
        }
        Self {
            layers,
            by_number,
            names,
        }
    }

    fn copper_index(&self, number: u8) -> Option<usize> {
        self.by_number.get(usize::from(number)).copied().flatten()
    }

    fn name(&self, number: u8) -> String {
        self.names
            .get(usize::from(number))
            .cloned()
            .flatten()
            .unwrap_or_else(|| format!("layer {number}"))
    }
}

/// Counts of what was skipped, for the notes.
#[derive(Default)]
struct Skipped {
    off_copper: std::collections::BTreeMap<String, usize>,
    with_net: usize,
}

impl Skipped {
    fn into_notes(self, what: &str, notes: &mut Vec<Note>) {
        for (layer, count) in self.off_copper {
            notes.push(Note {
                severity: Severity::Dropped,
                message: format!("{count} {what} on {layer} (not a copper layer) not read yet"),
            });
        }
        if self.with_net > 0 {
            notes.push(Note {
                severity: Severity::Info,
                message: format!(
                    "{} {what} belong to a net; net names are not read yet",
                    self.with_net
                ),
            });
        }
    }
}

/// Reads the tracks on copper layers from `/Tracks6/Data`.
pub(crate) fn tracks(
    data: &[u8],
    layers: &CopperLayers,
    notes: &mut Vec<Note>,
) -> Result<Vec<CopperItem>, String> {
    let mut items = Vec::new();
    let mut skipped = Skipped::default();
    for body in records(data, TRACK_RECORD, "/Tracks6/Data", notes)? {
        let layer_number = *body.first().ok_or("track record is empty")?;
        let from = point(body, 13)?;
        let to = point(body, 21)?;
        let width = length(body, 29)?;
        if read_u16(body, 3) != Some(NO_NET) {
            skipped.with_net += 1;
        }
        match layers.copper_index(layer_number) {
            Some(layer) => items.push(CopperItem::Track {
                layer,
                from,
                to,
                width,
                net: None,
            }),
            None => {
                *skipped
                    .off_copper
                    .entry(layers.name(layer_number))
                    .or_default() += 1
            }
        }
    }
    skipped.into_notes("track(s)", notes);
    Ok(items)
}

/// Reads the pads from `/Pads6/Data`.
pub(crate) fn pads(
    data: &[u8],
    layers: &CopperLayers,
    components: &Components,
    notes: &mut Vec<Note>,
) -> Result<Vec<CopperItem>, String> {
    let mut items = Vec::new();
    let mut skipped = Skipped::default();
    let mut rotated = 0usize;
    let mut uneven = 0usize;
    let mut at = 0usize;
    let mut count = 0usize;
    while at < data.len() {
        count += 1;
        if count > MAX_RECORDS {
            return Err(format!("/Pads6/Data has more than {MAX_RECORDS} records"));
        }
        let kind = data.get(at).copied().ok_or("pad record cut short")?;
        if kind != PAD_RECORD {
            notes.push(Note {
                severity: Severity::Dropped,
                message: format!(
                    "/Pads6/Data: unknown record type {kind:#04x} at byte {at}; the rest of the \
                     stream was not read"
                ),
            });
            break;
        }
        let mut cursor = at + 1;
        let mut geometry: Option<&[u8]> = None;
        for block in 0..PAD_BLOCKS {
            let len = read_u32(data, cursor).ok_or("pad block length cut short")?;
            let start = cursor + 4;
            let end = start
                .checked_add(len)
                .filter(|&e| e <= data.len())
                .ok_or("pad block is longer than the stream")?;
            if block == PAD_GEOMETRY_BLOCK {
                geometry = data.get(start..end);
            }
            cursor = end;
        }
        at = cursor;
        let g = geometry.ok_or("pad record has no geometry block")?;

        let layer_number = *g.first().ok_or("pad geometry block is empty")?;
        if read_u16(g, 3) != Some(NO_NET) {
            skipped.with_net += 1;
        }
        let Some(layer) = layers.copper_index(layer_number) else {
            *skipped
                .off_copper
                .entry(layers.name(layer_number))
                .or_default() += 1;
            continue;
        };
        let at_point = point(g, 13)?;
        let sizes = [
            (length(g, 21)?, length(g, 25)?),
            (length(g, 29)?, length(g, 33)?),
            (length(g, 37)?, length(g, 41)?),
        ];
        let hole = length(g, 45)?;
        let shapes = g.get(49..52).ok_or("pad shapes cut short")?;
        let rotation = read_f64(g, 52).ok_or("pad rotation cut short")?;
        if !rotation.is_finite() || rotation.abs() > 360.0 {
            return Err(format!("pad rotation {rotation} is out of range"));
        }
        if rotation != 0.0 {
            rotated += 1;
        }
        let [top, middle, bottom] = sizes;
        let (size_x, size_y) = top;
        if middle != top || bottom != top || shapes.iter().any(|s| Some(s) != shapes.first()) {
            uneven += 1;
        }
        let shape = match shapes.first() {
            Some(1) if size_x == size_y => PadShape::Round,
            Some(1) => PadShape::Oval,
            Some(2) => PadShape::Rectangle,
            other => {
                notes.push(Note {
                    severity: Severity::Warning,
                    message: format!(
                        "a pad at ({:.3}, {:.3}) has shape code {other:?}, not read yet; read as a \
                         rectangle",
                        at_point.x, at_point.y
                    ),
                });
                PadShape::Rectangle
            }
        };
        items.push(CopperItem::Pad {
            layer,
            at: at_point,
            shape,
            size_x,
            size_y,
            rotation_deg: rotation,
            drill: (hole > 0.0).then_some(hole),
            owner: components.owner(g),
        });
    }
    if rotated > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{rotated} pad(s) have a rotation; where the rotation is stored is not confirmed by \
                 a test board yet"
            ),
        });
    }
    if uneven > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{uneven} pad(s) differ between top, middle and bottom; read with the top size and \
                 shape"
            ),
        });
    }
    skipped.into_notes("pad(s)", notes);
    Ok(items)
}

/// Splits a stream of `[type byte][u32 length][body]` records into bodies.
fn records<'a>(
    data: &'a [u8],
    expected: u8,
    stream: &str,
    notes: &mut Vec<Note>,
) -> Result<Vec<&'a [u8]>, String> {
    let mut bodies = Vec::new();
    let mut at = 0usize;
    while at < data.len() {
        if bodies.len() >= MAX_RECORDS {
            return Err(format!("{stream} has more than {MAX_RECORDS} records"));
        }
        let kind = data.get(at).copied().ok_or("record cut short")?;
        if kind != expected {
            notes.push(Note {
                severity: Severity::Dropped,
                message: format!(
                    "{stream}: unknown record type {kind:#04x} at byte {at}; the rest of the \
                     stream was not read"
                ),
            });
            break;
        }
        let len = read_u32(data, at + 1).ok_or_else(|| format!("{stream}: length cut short"))?;
        let start = at + 5;
        let end = start
            .checked_add(len)
            .filter(|&e| e <= data.len())
            .ok_or_else(|| format!("{stream}: record is longer than the stream"))?;
        bodies.push(data.get(start..end).ok_or("record out of range")?);
        at = end;
    }
    Ok(bodies)
}

pub(crate) fn read_u16(data: &[u8], at: usize) -> Option<u16> {
    let bytes: [u8; 2] = data.get(at..at.checked_add(2)?)?.try_into().ok()?;
    Some(u16::from_le_bytes(bytes))
}

pub(crate) fn read_u32(data: &[u8], at: usize) -> Option<usize> {
    let bytes: [u8; 4] = data.get(at..at.checked_add(4)?)?.try_into().ok()?;
    usize::try_from(u32::from_le_bytes(bytes)).ok()
}

fn read_f64(data: &[u8], at: usize) -> Option<f64> {
    let bytes: [u8; 8] = data.get(at..at.checked_add(8)?)?.try_into().ok()?;
    Some(f64::from_le_bytes(bytes))
}

/// An `i32` in 1/10000 mil at `at`, in millimetres.
fn length(data: &[u8], at: usize) -> Result<f64, String> {
    let bytes: [u8; 4] = data
        .get(at..at + 4)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| format!("record cut short at byte {at}"))?;
    Ok(f64::from(i32::from_le_bytes(bytes)) * MM_PER_UNIT)
}

fn point(data: &[u8], at: usize) -> Result<Point, String> {
    Ok(Point {
        x: length(data, at)?,
        y: length(data, at + 4)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board(text: &str) -> Properties {
        Properties::parse(text.as_bytes())
    }

    fn top_only() -> CopperLayers {
        CopperLayers::new(&board(
            "|V9_STACK_LAYER0_NAME=Top Layer|V9_STACK_LAYER0_COPTHICK=1.4mil\
             |V9_STACK_LAYER1_NAME=Bottom Layer|V9_STACK_LAYER1_COPTHICK=1.4mil\
             |LAYER1NAME=Top Layer|LAYER74NAME=Multi-Layer|LAYER33NAME=Top Overlay",
        ))
    }

    #[test]
    fn layers_come_from_the_named_layer_numbers() {
        let l = top_only();
        assert_eq!(l.layers.len(), 2);
        assert_eq!(l.copper_index(1), Some(0));
        assert_eq!(l.layers[0].side, Some(Side::Top));
        assert_eq!(l.copper_index(74), Some(1));
        assert_eq!(l.copper_index(33), None);
        assert_eq!(l.name(33), "Top Overlay");
        assert_eq!(l.name(99), "layer 99");
    }

    #[test]
    fn a_track_off_copper_is_a_note() {
        let mut body = vec![33u8, 0, 0, 0xff, 0xff];
        body.resize(33, 0);
        let mut data = vec![TRACK_RECORD];
        data.extend_from_slice(&(body.len() as u32).to_le_bytes());
        data.extend_from_slice(&body);
        let mut notes = Vec::new();
        assert!(tracks(&data, &top_only(), &mut notes).unwrap().is_empty());
        assert!(notes[0].message.contains("Top Overlay"));
    }

    #[test]
    fn hostile_streams_are_errors_not_panics() {
        let l = top_only();
        let none = Components::default();
        let mut notes = Vec::new();
        assert!(tracks(&[TRACK_RECORD, 0xff, 0xff, 0xff, 0x7f], &l, &mut notes).is_err());
        assert!(tracks(&[TRACK_RECORD, 2, 0, 0, 0, 1, 0], &l, &mut notes).is_err());
        assert!(pads(&[PAD_RECORD, 0xff, 0xff, 0xff, 0x7f], &l, &none, &mut notes).is_err());
        let mut few_blocks = vec![PAD_RECORD];
        few_blocks.extend_from_slice(&0u32.to_le_bytes());
        assert!(pads(&few_blocks, &l, &none, &mut notes).is_err());
        assert!(tracks(&[0x42], &l, &mut notes).unwrap().is_empty());
        assert_eq!(notes.last().map(|n| n.severity), Some(Severity::Dropped));
    }
}
