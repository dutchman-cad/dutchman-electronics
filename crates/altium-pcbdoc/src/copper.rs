//! Roadmap step 3: tracks, track arcs, pads, vias, fills and copper regions on copper layers.
//!
//! Worked out from our own CC0 test boards (`one-track`, `outline-with-hole`, `one-resistor`,
//! `one-via`, `one-plated-pad`, `one-rotated-pad`, `one-oval-pad`, `one-bottom-track`, `one-arc`,
//! `one-fill`, `one-region`, `one-pour`)
//! by comparing each with `empty-outline`, and from the numbers in their README.txt (see
//! LEGAL.md). Coordinates and sizes are little-endian `i32` in 1/10000 mil, absolute (the boards'
//! origin is at 0, 0).
//!
//! The file stores whole units, so a typed length can come out a fraction of a micrometre off
//! (50 mm is 19685039.37 units, stored as 19685039, read as 49.99999906 mm). That is the file, not
//! a rounding bug; tests allow 0.001 mm.
//!
//! - A layer is one byte `n`; `/Board6/Data` names it in a `LAYERnNAME` key (`LAYER1NAME=Top
//!   Layer`, `LAYER32NAME=Bottom Layer`, `LAYER74NAME=Multi-Layer` on our boards). A stack layer
//!   with a `COPTHICK` is copper.
//! - `/Tracks6/Data`: records of type `0x04`, a `u32` length, then a body whose byte 0 is the
//!   layer, bytes 3..5 the net index (`ff ff` for none), and from byte 13 the start x, start y,
//!   end x, end y and width. `one-bottom-track` differs from `one-track` in the layer byte (32
//!   instead of 1) and in bytes 41..43, which are not read.
//! - `/Arcs6/Data` (`one-arc`): records of type `0x01`, a `u32` length, then a body whose byte 0
//!   is the layer, bytes 3..5 the net index, from byte 13 the centre x and y, at byte 21 the
//!   radius, at bytes 25..33 and 33..41 two `f64` in degrees, and at byte 41 the width. Our arc
//!   runs from 0 to 90 degrees: bytes 33..41 hold 90.0 (the end angle) and bytes 25..33 hold 0.0.
//!   `one-overlay-arc` (30 to 300 degrees) confirms that bytes 25..33 are the start angle.
//! - Lines and arcs on the overlay layers (`one-overlay-line`, `one-overlay-arc`,
//!   `one-bottom-overlay-line`) are the same track and arc records on layer 33 (Top Overlay) or
//!   34 (Bottom Overlay). They become [`Graphic`]s on a silkscreen layer. Bytes 7..9 are the index
//!   of the component a line belongs to (0 for R1's line on `one-resistor-silkscreen`, `ff ff` for
//!   none), as for pads; its designator becomes the owner.
//! - `/Vias6/Data` (`one-via`): records of type `0x03`, a `u32` length, then a body whose byte 0
//!   is the layer (74, Multi-Layer), bytes 3..5 the net index, from byte 13 the centre x and y, at
//!   byte 21 the diameter and at byte 25 the hole diameter. Bytes 29 and 30 hold 1 and 32: the
//!   via runs from Top Layer to Bottom Layer, as typed in. A via whose two layer bytes are not the
//!   top and bottom copper layers is read as a through via, with a warning.
//! - `/Pads6/Data`: records of type `0x02` followed by six blocks, each a `u32` length and its
//!   bytes. Block 4 holds the geometry: byte 0 the layer, bytes 3..5 the net index, from byte 13
//!   the centre x and y, then three (x, y) size pairs (top, middle, bottom), from byte 45 the hole
//!   diameter (0 for a pad without a hole), and at bytes 49..52 the shapes of the three size pairs
//!   (1 round, 2 rectangle). Bytes 52..60 are the rotation as an `f64` in degrees: 30.0 on
//!   `one-rotated-pad`, 0.0 on every other pad. Its sizes are stored before rotation (x 2.000,
//!   y 1.000, as typed). Round with unequal sizes is the oval of `one-oval-pad` (2.000 x 1.000).
//!   Byte 60 is 1 on the plated hole of `one-plated-pad` and 0 on the unplated hole of
//!   `outline-with-hole` and on every SMD pad.
//!   Bytes 7..9 are the index of the component the pad belongs to (see `components.rs`);
//!   its designator becomes the pad's owner.
//! - Nets (`one-net`): `/Nets6/Data` is a run of records, each a `u32` length and text with
//!   `NAME=GND`; the n-th record (from 0) is net n. Bytes 3..5 of a track's body are its net
//!   index: 0 on `one-net`'s track, `ff ff` for no net.
//! - `/Fills6/Data` (`one-fill`): records of type `0x06` and a `u32` length; in the body, byte 0
//!   is the layer, bytes 3..5 the net index, from byte 13 two corners (x1, y1, x2, y2), then at
//!   byte 29 the rotation as an `f64` in degrees: 0 on `one-fill`, 30 on `one-rotated-fill`, whose
//!   bounding box (x 18.17 to 31.83) shows it is turned counter-clockwise about its centre.
//! - Copper regions (`one-region`) and the copper Altium poured for a polygon (`one-pour`) are
//!   region records in `/Regions6/Data` (see `board.rs`) with `KIND=0` and the layer in body
//!   byte 0; poured copper also has `SUBPOLYINDEX=0` (a free region has -1). The pour's own
//!   definition in `/Polygons6/Data` is not needed for its copper.

use board_model::{
    Contour, CopperItem, Graphic, Layer, LayerKind, Note, PadShape, Point, Segment, Severity, Side,
};

use crate::board::{OtherRegion, Properties};
use crate::components::Components;

/// Millimetres per internal unit (1/10000 mil).
const MM_PER_UNIT: f64 = 0.0254 / 10_000.0;
const TRACK_RECORD: u8 = 0x04;
const PAD_RECORD: u8 = 0x02;
const ARC_RECORD: u8 = 0x01;
const VIA_RECORD: u8 = 0x03;
const FILL_RECORD: u8 = 0x06;
/// Angles beyond this many degrees (either way) are refused as not real.
const MAX_ANGLE_DEG: f64 = 360.0;
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
    /// The net names of `/Nets6/Data`, by net index.
    pub nets: Vec<String>,
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
            nets: Vec::new(),
        }
    }

    /// Whether two layer bytes are the top and the bottom copper layer, in either order.
    fn are_top_and_bottom(&self, a: u8, b: u8) -> bool {
        let side = |n: u8| {
            self.copper_index(n)
                .and_then(|i| self.layers.get(i))
                .and_then(|l| l.side)
        };
        matches!(
            (side(a), side(b)),
            (Some(Side::Top), Some(Side::Bottom)) | (Some(Side::Bottom), Some(Side::Top))
        )
    }

    /// The index in `layers` for a text on layer byte `number`: a copper layer or an overlay layer
    /// (see [`Self::overlay_layer`]). `None` for any other layer.
    pub(crate) fn text_layer(&mut self, number: u8) -> Option<usize> {
        self.copper_index(number)
            .or_else(|| self.overlay_layer(number))
    }

    /// The index in `layers` for an overlay layer (named "… Overlay" in `/Board6/Data`), which is
    /// added as silkscreen the first time something on it is read. `None` for any other layer.
    pub(crate) fn overlay_layer(&mut self, number: u8) -> Option<usize> {
        let name = self.names.get(usize::from(number)).cloned().flatten()?;
        if !name.ends_with("Overlay") {
            return None;
        }
        if let Some(index) = self.layers.iter().position(|l| l.name == name) {
            return Some(index);
        }
        let side = if name.starts_with("Top") {
            Some(Side::Top)
        } else if name.starts_with("Bottom") {
            Some(Side::Bottom)
        } else {
            None
        };
        self.layers.push(Layer {
            name,
            kind: LayerKind::Silkscreen,
            side,
        });
        Some(self.layers.len() - 1)
    }

    pub(crate) fn layer_name(&self, number: u8) -> String {
        self.name(number)
    }

    /// The net of a record whose body holds the net index at bytes 3..5: `Ok(None)` for no net,
    /// `Err(())` for a net index with no name in `/Nets6/Data`.
    fn net(&self, body: &[u8]) -> Result<Option<String>, ()> {
        match read_u16(body, 3) {
            None | Some(NO_NET) => Ok(None),
            Some(i) => self.nets.get(usize::from(i)).cloned().map(Some).ok_or(()),
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
    /// Items on layers that are neither copper nor overlay, by layer name.
    off_copper: std::collections::BTreeMap<String, usize>,
    /// Items that belong to a net the board model has no place for (pads, vias, fills).
    with_net: usize,
    /// Items whose net index has no name in `/Nets6/Data`.
    unnamed_net: usize,
    /// Items left out because an angle in them is not a real angle.
    bad_angle: usize,
}

impl Skipped {
    fn into_notes(self, what: &str, notes: &mut Vec<Note>) {
        for (layer, count) in self.off_copper {
            notes.push(Note {
                severity: Severity::Dropped,
                message: format!(
                    "{count} {what} on {layer} (not a copper or overlay layer) not read yet"
                ),
            });
        }
        if self.bad_angle > 0 {
            notes.push(Note {
                severity: Severity::Dropped,
                message: format!(
                    "{} {what} left out: an angle in them is not a number of degrees",
                    self.bad_angle
                ),
            });
        }
        if self.with_net > 0 {
            notes.push(Note {
                severity: Severity::Info,
                message: format!(
                    "{} {what} belong to a net; the board model keeps no net for them",
                    self.with_net
                ),
            });
        }
        if self.unnamed_net > 0 {
            notes.push(Note {
                severity: Severity::Warning,
                message: format!(
                    "{} {what} name a net that /Nets6/Data does not have; read without a net",
                    self.unnamed_net
                ),
            });
        }
    }
}

/// Reads the tracks from `/Tracks6/Data`: those on copper layers as copper, those on the overlay
/// layers into `graphics`.
pub(crate) fn tracks(
    data: &[u8],
    layers: &mut CopperLayers,
    components: &Components,
    graphics: &mut Vec<Graphic>,
    notes: &mut Vec<Note>,
) -> Result<Vec<CopperItem>, String> {
    let mut items = Vec::new();
    let mut skipped = Skipped::default();
    for body in records(data, TRACK_RECORD, "/Tracks6/Data", notes)? {
        let layer_number = *body.first().ok_or("track record is empty")?;
        let from = point(body, 13)?;
        let to = point(body, 21)?;
        let width = length(body, 29)?;
        let net = layers.net(body).unwrap_or_else(|()| {
            skipped.unnamed_net += 1;
            None
        });
        if let Some(layer) = layers.copper_index(layer_number) {
            items.push(CopperItem::Track {
                layer,
                from,
                to,
                width,
                net,
            });
        } else if let Some(layer) = layers.overlay_layer(layer_number) {
            graphics.push(Graphic::Line {
                layer,
                from,
                to,
                width,
                owner: components.owner(body),
            });
        } else {
            *skipped
                .off_copper
                .entry(layers.name(layer_number))
                .or_default() += 1;
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
        let Some(rotation) = angle(g, 52)? else {
            skipped.bad_angle += 1;
            continue;
        };
        let plated = hole > 0.0 && g.get(60) == Some(&1);
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
            plated,
            owner: components.owner(g),
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

/// Reads the arcs from `/Arcs6/Data`: those on copper layers as track arcs, those on the overlay
/// layers into `graphics`.
pub(crate) fn arcs(
    data: &[u8],
    layers: &mut CopperLayers,
    components: &Components,
    graphics: &mut Vec<Graphic>,
    notes: &mut Vec<Note>,
) -> Result<Vec<CopperItem>, String> {
    let mut items = Vec::new();
    let mut skipped = Skipped::default();
    for body in records(data, ARC_RECORD, "/Arcs6/Data", notes)? {
        let layer_number = *body.first().ok_or("arc record is empty")?;
        let center = point(body, 13)?;
        let radius = length(body, 21)?;
        let width = length(body, 41)?;
        let (Some(start_deg), Some(end_deg)) = (angle(body, 25)?, angle(body, 33)?) else {
            skipped.bad_angle += 1;
            continue;
        };
        let net = layers.net(body).unwrap_or_else(|()| {
            skipped.unnamed_net += 1;
            None
        });
        let Some(layer) = layers.copper_index(layer_number) else {
            match layers.overlay_layer(layer_number) {
                Some(layer) => graphics.push(Graphic::Arc {
                    layer,
                    center,
                    radius,
                    start_deg,
                    end_deg,
                    width,
                    owner: components.owner(body),
                }),
                None => {
                    *skipped
                        .off_copper
                        .entry(layers.name(layer_number))
                        .or_default() += 1;
                }
            }
            continue;
        };
        items.push(CopperItem::TrackArc {
            layer,
            center,
            radius,
            start_deg,
            end_deg,
            width,
            net,
        });
    }
    skipped.into_notes("arc(s)", notes);
    Ok(items)
}

/// Reads the vias from `/Vias6/Data`. Every via is read as running through the whole board.
pub(crate) fn vias(
    data: &[u8],
    layers: &CopperLayers,
    notes: &mut Vec<Note>,
) -> Result<Vec<CopperItem>, String> {
    let mut items = Vec::new();
    let mut skipped = Skipped::default();
    let mut partial = 0usize;
    for body in records(data, VIA_RECORD, "/Vias6/Data", notes)? {
        let at = point(body, 13)?;
        let diameter = length(body, 21)?;
        let drill = length(body, 25)?;
        let ends = body.get(29..31).ok_or("via layers cut short")?;
        if let [a, b] = ends {
            if !layers.are_top_and_bottom(*a, *b) {
                partial += 1;
            }
        }
        if read_u16(body, 3) != Some(NO_NET) {
            skipped.with_net += 1;
        }
        items.push(CopperItem::Via {
            at,
            diameter,
            drill,
        });
    }
    if partial > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{partial} via(s) do not run from the top to the bottom copper layer (blind or \
                 buried?); read as running through the whole board"
            ),
        });
    }
    skipped.into_notes("via(s)", notes);
    Ok(items)
}

/// Reads the fills from `/Fills6/Data` as rectangular copper regions. A rotated fill is the
/// rectangle of its two corners turned counter-clockwise about its centre (`one-rotated-fill`).
pub(crate) fn fills(
    data: &[u8],
    layers: &CopperLayers,
    notes: &mut Vec<Note>,
) -> Result<Vec<CopperItem>, String> {
    let mut items = Vec::new();
    let mut skipped = Skipped::default();
    for body in records(data, FILL_RECORD, "/Fills6/Data", notes)? {
        let layer_number = *body.first().ok_or("fill record is empty")?;
        let a = point(body, 13)?;
        let b = point(body, 21)?;
        let Some(rotation) = angle(body, 29)? else {
            skipped.bad_angle += 1;
            continue;
        };
        if read_u16(body, 3) != Some(NO_NET) {
            skipped.with_net += 1;
        }
        let Some(layer) = layers.copper_index(layer_number) else {
            *skipped
                .off_copper
                .entry(layers.name(layer_number))
                .or_default() += 1;
            continue;
        };
        let centre = Point {
            x: (a.x + b.x) / 2.0,
            y: (a.y + b.y) / 2.0,
        };
        let (sin, cos) = rotation.to_radians().sin_cos();
        let turn = |p: Point| Point {
            x: centre.x + (p.x - centre.x) * cos - (p.y - centre.y) * sin,
            y: centre.y + (p.x - centre.x) * sin + (p.y - centre.y) * cos,
        };
        let corners = [a, Point { x: b.x, y: a.y }, b, Point { x: a.x, y: b.y }].map(turn);
        items.push(CopperItem::Region {
            layer,
            contour: closed(&corners),
            holes: Vec::new(),
        });
    }
    skipped.into_notes("fill(s)", notes);
    Ok(items)
}

/// Turns the non-cutout regions of `/Regions6/Data` into copper regions: `KIND=0` on a copper
/// layer is a solid copper region, or the poured copper of a polygon pour. Any other region is
/// left out with a note.
pub(crate) fn regions(
    others: &[OtherRegion],
    layers: &CopperLayers,
    notes: &mut Vec<Note>,
) -> Vec<CopperItem> {
    let mut items = Vec::new();
    let mut poured = 0usize;
    let mut not_copper: std::collections::BTreeMap<String, usize> = Default::default();
    for region in others {
        let layer = layers.copper_index(region.layer);
        match (layer, region.kind.as_deref().map(str::trim)) {
            (Some(layer), Some("0")) => {
                if region.poured {
                    poured += 1;
                }
                items.push(CopperItem::Region {
                    layer,
                    contour: region.contour.clone(),
                    holes: region.holes.clone(),
                });
            }
            (_, kind) => {
                let what = format!(
                    "region(s) of KIND {} on {}",
                    kind.unwrap_or("?"),
                    layers.name(region.layer)
                );
                *not_copper.entry(what).or_default() += 1;
            }
        }
    }
    if poured > 0 {
        notes.push(Note {
            severity: Severity::Info,
            message: format!("{poured} copper region(s) are the copper of a polygon pour"),
        });
    }
    for (what, count) in not_copper {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!("{count} {what} not read yet"),
        });
    }
    items
}

/// Joins the points with straight lines into a closed contour.
fn closed(points: &[Point]) -> Contour {
    let next = points.iter().skip(1).chain(points.first());
    Contour {
        segments: points
            .iter()
            .zip(next)
            .map(|(from, to)| Segment::Line {
                from: *from,
                to: *to,
            })
            .collect(),
    }
}

/// An `f64` angle in degrees at `at`: `None` when it is not a real angle (not finite, or more
/// than a full turn either way), an error when the record is too short.
fn angle(data: &[u8], at: usize) -> Result<Option<f64>, String> {
    let value = read_f64(data, at).ok_or_else(|| format!("record cut short at byte {at}"))?;
    Ok((value.is_finite() && value.abs() <= MAX_ANGLE_DEG).then_some(value))
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
             |LAYER1NAME=Top Layer|LAYER74NAME=Multi-Layer|LAYER33NAME=Top Overlay\
             |LAYER57NAME=Mechanical 1",
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
    fn a_track_on_the_overlay_is_a_graphic_and_elsewhere_a_note() {
        let mut data = Vec::new();
        for layer in [33u8, 57] {
            let mut body = vec![layer, 0, 0, 0xff, 0xff];
            body.resize(33, 0);
            data.push(TRACK_RECORD);
            data.extend_from_slice(&(body.len() as u32).to_le_bytes());
            data.extend_from_slice(&body);
        }
        let mut l = top_only();
        let (mut graphics, mut notes) = (Vec::new(), Vec::new());
        let none = Components::default();
        assert!(tracks(&data, &mut l, &none, &mut graphics, &mut notes)
            .unwrap()
            .is_empty());
        let [Graphic::Line { layer, owner, .. }] = graphics.as_slice() else {
            panic!("{graphics:?}");
        };
        assert_eq!(l.layers[*layer].name, "Top Overlay");
        assert_eq!(l.layers[*layer].kind, LayerKind::Silkscreen);
        assert_eq!(*owner, None);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].message.contains("Mechanical 1"));
    }

    /// A record of `kind` whose body is `len` zero bytes, with `set` applied to the body.
    fn record(kind: u8, len: usize, set: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
        let mut body = vec![0u8; len];
        set(&mut body);
        let mut data = vec![kind];
        data.extend_from_slice(&(body.len() as u32).to_le_bytes());
        data.extend_from_slice(&body);
        data
    }

    fn with_bottom() -> CopperLayers {
        CopperLayers::new(&board(
            "|V9_STACK_LAYER0_NAME=Top Layer|V9_STACK_LAYER0_COPTHICK=1.4mil\
             |V9_STACK_LAYER1_NAME=Bottom Layer|V9_STACK_LAYER1_COPTHICK=1.4mil\
             |LAYER1NAME=Top Layer|LAYER32NAME=Bottom Layer|LAYER74NAME=Multi-Layer",
        ))
    }

    #[test]
    fn a_via_between_other_layers_is_read_through_with_a_warning() {
        let l = with_bottom();
        assert!(l.are_top_and_bottom(32, 1));
        let data = record(VIA_RECORD, 31, |b| {
            b[0] = 74;
            b[3] = 0xff;
            b[4] = 0xff;
            b[29] = 1;
            b[30] = 74;
        });
        let mut notes = Vec::new();
        assert_eq!(vias(&data, &l, &mut notes).unwrap().len(), 1);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].severity, Severity::Warning);
    }

    #[test]
    fn an_arc_with_a_bad_angle_costs_that_arc_only() {
        let mut l = with_bottom();
        let mut data = record(ARC_RECORD, 45, |b| {
            b[0] = 1;
            b[25..33].copy_from_slice(&f64::NAN.to_le_bytes());
        });
        data.extend(record(ARC_RECORD, 45, |b| {
            b[0] = 1;
            b[25..33].copy_from_slice(&45.0f64.to_le_bytes());
            b[33..41].copy_from_slice(&90.0f64.to_le_bytes());
        }));
        let (mut graphics, mut notes) = (Vec::new(), Vec::new());
        let none = Components::default();
        let items = arcs(&data, &mut l, &none, &mut graphics, &mut notes).unwrap();
        assert_eq!(items.len(), 1);
        let severities: Vec<Severity> = notes.iter().map(|n| n.severity).collect();
        assert!(severities.contains(&Severity::Dropped), "{notes:?}");
        // The arc that starts at 45 degrees gets no note about its start (`one-overlay-arc`).
        assert!(
            !notes.iter().any(|n| n.message.contains("start")),
            "{notes:?}"
        );
    }

    #[test]
    fn hostile_streams_are_errors_not_panics() {
        let l = top_only();
        let mut m = top_only();
        let mut g = Vec::new();
        let none = Components::default();
        let mut notes = Vec::new();
        assert!(tracks(
            &[TRACK_RECORD, 0xff, 0xff, 0xff, 0x7f],
            &mut m,
            &none,
            &mut g,
            &mut notes
        )
        .is_err());
        assert!(tracks(
            &[TRACK_RECORD, 2, 0, 0, 0, 1, 0],
            &mut m,
            &none,
            &mut g,
            &mut notes
        )
        .is_err());
        assert!(pads(&[PAD_RECORD, 0xff, 0xff, 0xff, 0x7f], &l, &none, &mut notes).is_err());
        let mut few_blocks = vec![PAD_RECORD];
        few_blocks.extend_from_slice(&0u32.to_le_bytes());
        assert!(pads(&few_blocks, &l, &none, &mut notes).is_err());
        assert!(arcs(
            &record(ARC_RECORD, 40, |_| ()),
            &mut m,
            &none,
            &mut g,
            &mut notes
        )
        .is_err());
        assert!(vias(&record(VIA_RECORD, 30, |_| ()), &l, &mut notes).is_err());
        assert!(arcs(
            &[ARC_RECORD, 0xff, 0xff, 0xff, 0x7f],
            &mut m,
            &none,
            &mut g,
            &mut notes
        )
        .is_err());
        assert!(vias(&[VIA_RECORD, 1, 0, 0, 0], &l, &mut notes).is_err());
        assert!(fills(&[FILL_RECORD, 0xff, 0xff, 0xff, 0x7f], &l, &mut notes).is_err());
        assert!(fills(&[FILL_RECORD, 2, 0, 0, 0, 1, 0], &l, &mut notes).is_err());
        assert!(tracks(&[0x42], &mut m, &none, &mut g, &mut notes)
            .unwrap()
            .is_empty());
        assert_eq!(notes.last().map(|n| n.severity), Some(Severity::Dropped));
    }
}
