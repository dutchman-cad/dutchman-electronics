//! Roadmap step 3: tracks, track arcs, pads, vias, fills and copper regions on copper layers.
//!
//! Worked out from our own CC0 test boards (`one-track`, `outline-with-hole`, `one-resistor`,
//! `one-via`, `one-plated-pad`, `one-rotated-pad`, `one-oval-pad`, `one-bottom-track`, `one-arc`,
//! `one-fill`, `one-region`, `one-pour`)
//! by comparing each with `empty-outline`, and from the numbers in their README.txt (see
//! LEGAL.md). Coordinates and sizes are little-endian `i32` in 1/10000 mil, absolute (the boards'
//! origin is at 0, 0).
//!
//! The file stores whole units, so a typed length can come out a fraction of a micrometer off
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
//!   is the layer, bytes 3..5 the net index, from byte 13 the center x and y, at byte 21 the
//!   radius, at bytes 25..33 and 33..41 two `f64` in degrees, and at byte 41 the width. Our arc
//!   runs from 0 to 90 degrees: bytes 33..41 hold 90.0 (the end angle) and bytes 25..33 hold 0.0.
//!   `one-overlay-arc` (30 to 300 degrees) confirms that bytes 25..33 are the start angle.
//! - Lines and arcs on the overlay layers (`one-overlay-line`, `one-overlay-arc`,
//!   `one-bottom-overlay-line`) are the same track and arc records on layer 33 (Top Overlay) or
//!   34 (Bottom Overlay). They become [`Graphic`]s on a silkscreen layer. Bytes 7..9 are the index
//!   of the component a line belongs to (0 for R1's line on `one-resistor-silkscreen`, `ff ff` for
//!   none), as for pads; its designator becomes the owner.
//! - `/Vias6/Data` (`one-via`): records of type `0x03`, a `u32` length, then a body whose byte 0
//!   is the layer (74, Multi-Layer), bytes 3..5 the net index, from byte 13 the center x and y, at
//!   byte 21 the diameter and at byte 25 the hole diameter. Bytes 29 and 30 are the layers the
//!   via joins, as layer bytes (see below): 1 and 32 for Top Layer to Bottom Layer on `one-via`;
//!   on `vias-blind-buried` 1 and 2 (Top Layer to Layer 1), 3 and 32 (Layer 2 to Bottom Layer),
//!   2 and 3 (Layer 1 to Layer 2) and 1 and 3. A via that does not join the top and bottom copper
//!   layers keeps that span; one whose layer bytes name no copper layer is read as a through via,
//!   with a warning.
//!   Solder mask (`vias-tenting`, `vias-tented-rule`, compared with `vias-sizes`): byte 1 holds
//!   flags, 0x20 when the mask covers (tents) the via on the top side and 0x40 on the bottom (12 on
//!   an open via, 44 tented on top, 76 on the bottom, 108 on both). Altium sets them both for a
//!   via tented by hand and for one tented by the board-wide rule (`ISTENTINGTOP=TRUE`), so they
//!   are read as they stand. Byte 66 is 1 when the mask expansion comes from the rule and 2 when
//!   it is given on the via (Manual); bytes 54..58 then hold it, as a length (0.200 on the `M0.2`
//!   via, and the same again at bytes 242..246, as top and bottom were linked). With the rule,
//!   bytes 54..58 hold the rule's value, or minus the via's diameter when the rule tents it, and
//!   are not read.
//! - `/Pads6/Data`: records of type `0x02` followed by six blocks, each a `u32` length and its
//!   bytes. Block 4 holds the geometry: byte 0 the layer, bytes 3..5 the net index, from byte 13
//!   the center x and y, then three (x, y) size pairs (top, middle, bottom), from byte 45 the hole
//!   diameter (0 for a pad without a hole), and at bytes 49..52 the shapes of the three size pairs
//!   (1 round, 2 rectangle). Bytes 52..60 are the rotation as an `f64` in degrees: 30.0 on
//!   `one-rotated-pad`, 0.0 on every other pad. Its sizes are stored before rotation (x 2.000,
//!   y 1.000, as typed). Round with unequal sizes is the oval of `one-oval-pad` (2.000 x 1.000).
//!   Byte 60 is 1 on the plated hole of `one-plated-pad` and 0 on the unplated hole of
//!   `outline-with-hole` and on every SMD pad.
//!   Bytes 7..9 are the index of the component the pad belongs to (see `components.rs`);
//!   its designator becomes the pad's owner.
//! - A pad's own solder and paste mask (`pad-mask-overrides`, each pad compared with its
//!   reference pad P1, which keeps the rule). The block lines up with the via body 36 bytes on
//!   (pad bytes 68..86 are via bytes 32..50). Byte 1 holds the via's tenting bits: 0x20 on P4,
//!   tented on top, 0x40 on P6, tented on the bottom. Byte 102 is the solder mask mode, 1 by the
//!   rule and 2 Manual (P2 to P6 and P8), like the via's byte 66; byte 101 the paste mode (2 on
//!   P7 only). Bytes 90..94 are the top mask expansion (0 on P2, 0.200 on P3 and P5), bytes
//!   121..125 the bottom one (0.050 on P5, whose top and bottom are unlinked) and bytes 86..90
//!   the paste expansion (0.050 on P7). With the rule they hold the rule's value (0.1 here, the
//!   `mask-expansion-*` values on those boards; 0 for paste on every pad), and a tented pad keeps
//!   its expansion. Byte 120 is 1 only on P5, so it is read as "bottom unlinked"; byte 1 has its
//!   0x01 bit set there too. On a linked pad bytes 121..125 are not kept in step (0.1 on P3,
//!   Manual 0.2), so the bottom takes the top's value. Byte 125 is 1 on P8, whose expansion is
//!   measured from the hole's edge. Bytes 97..100 are 1 on every pad but P1 and P8 here and on
//!   every pad of the older boards; not read. The tenting and the expansion go on the pad. An
//!   expansion measured from the hole's edge is turned into one from the pad's edge: the opening
//!   reaches `hole / 2 + e` from the center and the pad `size / 2`, so it is
//!   `(hole + 2 e - size) / 2`, with the shorter side as the size of a pad that is not round (P8:
//!   (1.000 + 0.200 - 2.000) / 2 = -0.400), and a note counts the pads whose opening ends up
//!   smaller than the pad. The paste expansion has no place in the board model and is only
//!   counted in a note.
//! - Pad holes that are not round (`pad-hole-shapes`): the hole size at byte 45 of block 4 is a
//!   slot's width (0.800) or a square's side (1.000), and block 5 has its 651 bytes, with the
//!   hole type at byte 262 (1 square, 2 slot; a round hole has an empty block 5), the slot's
//!   length at bytes 263..267 (2.200) and the hole's turn at bytes 267..275 as an `f64` in degrees
//!   (45.0 on the turned slot, 0.0 on the others). Byte 60 is the plating as on a round hole. The
//!   board model holds a round drill only, so such a pad is read with a round hole of that width
//!   or side, and a note names it.
//! - Pad shapes (`pad-shapes`, checked against the Gerber file Altium wrote for its top copper):
//!   shape code 3 is an octagon, a rectangle of the pad's size with each corner cut off at 45
//!   degrees by a quarter of the shorter side (0.25 on a 2.000 x 1.000 pad, 0.375 on 1.500 x
//!   1.500). A rounded rectangle keeps code 1 (round) in block 4 and has a block 5 of 651 bytes,
//!   which no other pad on our boards has: 29 `i32` x sizes and 29 `i32` y sizes (bytes 0..232),
//!   29 shape bytes (1), 32 corner percentages (bytes 564..596), and at the end the top layer's
//!   shape (byte 640: 9, rounded rectangle), x size, y size (bytes 641..649) and corner percentage
//!   (byte 649: 50, 25 or 30 as set). The corner radius is that percentage of half the shorter
//!   side: 0.25 for 50% of a 2.000 x 1.000 pad, 0.125 for 25%, 0.24 for 30% of 1.600 x 1.600.
//! - Keepouts (`keepouts`): a track, arc, fill and region on the Keep-Out layer (layer byte 56,
//!   `Keep-Out Layer`), and a track, fill and region on Top Layer with Altium's Keepout box
//!   ticked. Byte 2 of the record body is 2 on the three ticked ones and 0 on every other track,
//!   arc, fill, region and via of every test board; the ticked track also has 31 at byte 45, the
//!   fill 31 at byte 46 and the region `KEEPOUTRESTRICTIONS=31` (Altium's default restrictions).
//!   A keepout is a rule for placing and routing, not something that is made, so both kinds are
//!   left out, each kind with an Info note.
//! - Mechanical layer kinds (`mech-layer-kinds`, one-resistor with four Component Layer Pairs
//!   added in View Configuration): `/Board6/Data` has `LAYERnMECHKIND` for each typed mechanical
//!   layer, `3DBodyTop` and `3DBodyBottom` on 58 and 59 (Mechanical 2 and 3, as on one-resistor),
//!   `CourtyardTop`, `CourtyardBottom`, `AssemblyTop`, `AssemblyBottom`, `ComponentOutlineTop`,
//!   `ComponentOutlineBottom`, `DesignatorTop` and `DesignatorBottom` on 60 to 67 (Mechanical 4
//!   to 11; Mechanical k is layer byte 56 + k), and the pairs as `MECHPAIRnL1`/`MECHPAIRnL2` =
//!   `MECHANICAL4`/`MECHANICAL5` and so on. The reader keeps each layer's kind, side and pair
//!   (no board model place yet), and the notes for what lies on such a layer name its kind:
//!   "on Bottom Courtyard (courtyard, bottom)".
//! - Nets (`one-net`): `/Nets6/Data` is a run of records, each a `u32` length and text with
//!   `NAME=GND`; the n-th record (from 0) is net n. Bytes 3..5 of a track's body are its net
//!   index: 0 on `one-net`'s track, `ff ff` for no net.
//! - `/Fills6/Data` (`one-fill`): records of type `0x06` and a `u32` length; in the body, byte 0
//!   is the layer, bytes 3..5 the net index, from byte 13 two corners (x1, y1, x2, y2), then at
//!   byte 29 the rotation as an `f64` in degrees: 0 on `one-fill`, 30 on `one-rotated-fill`, whose
//!   bounding box (x 18.17 to 31.83) shows it is turned counter-clockwise about its center.
//! - Copper regions (`one-region`) and the copper Altium poured for a polygon (`one-pour`) are
//!   region records in `/Regions6/Data` (see `board.rs`) with `KIND=0` and the layer in body
//!   byte 0; poured copper also has `SUBPOLYINDEX=0` (a free region has -1). The pour's own
//!   definition in `/Polygons6/Data` is not needed for its copper.
//! - Internal planes (`four-layer-planes`): its two plane layers are layer bytes 39 and 40
//!   (`LAYER39NAME=Layer 1`, `LAYER40NAME=Layer 2`), in the V9 stack like signal layers, with a
//!   `COPTHICK` and also a `PULLBACKDISTANCE`. A plane is copper wherever nothing is drawn, so
//!   what lies on it is where copper is left out: Altium drew the pull-back from the board edge as
//!   four tracks on each plane layer, 1.016 wide (twice the 0.508 pull-back) along the edges,
//!   and the split line we drew is a track too. The copper itself is in `/Polygons6/Data` (see
//!   `planes.rs`), which already leaves the pull-back and the split out, so tracks on a plane
//!   layer are not drawn. Bytes 39 to 54 are taken to be the 16 planes Altium offers.

use board_model::{
    Contour, CopperItem, Graphic, Layer, LayerKind, Note, PadShape, Point, Segment, Severity, Side,
    StackPosition,
};

use crate::board::{OtherRegion, Properties};
use crate::components::Components;

/// Millimeters per internal unit (1/10000 mil).
const MM_PER_UNIT: f64 = 0.0254 / 10_000.0;
const TRACK_RECORD: u8 = 0x04;
const PAD_RECORD: u8 = 0x02;
const ARC_RECORD: u8 = 0x01;
const VIA_RECORD: u8 = 0x03;
const FILL_RECORD: u8 = 0x06;
/// A via's flags byte; [`VIA_TENTED_TOP`] and [`VIA_TENTED_BOTTOM`] are its tenting bits.
const VIA_FLAGS_AT: usize = 1;
const VIA_TENTED_TOP: u8 = 0x20;
const VIA_TENTED_BOTTOM: u8 = 0x40;
/// Where a via's own mask expansion starts, and the byte that says whether it is used.
const VIA_MASK_EXPANSION_AT: usize = 54;
const VIA_MASK_MODE_AT: usize = 66;
/// A pad's tenting bits are in the same byte as a via's; the rest of its mask settings are in
/// the geometry block (see the module notes).
const PAD_FLAGS_AT: usize = 1;
const PAD_PASTE_EXPANSION_AT: usize = 86;
const PAD_MASK_TOP_AT: usize = 90;
const PAD_PASTE_MODE_AT: usize = 101;
const PAD_MASK_MODE_AT: usize = 102;
const PAD_MASK_UNLINKED_AT: usize = 120;
const PAD_MASK_BOTTOM_AT: usize = 121;
const PAD_MASK_FROM_HOLE_AT: usize = 125;
/// The mode byte of a mask expansion set on the object itself rather than by the rule.
const MASK_MANUAL: u8 = 2;
/// Angles beyond this many degrees (either way) are refused as not real.
const MAX_ANGLE_DEG: f64 = 360.0;
const PAD_BLOCKS: usize = 6;
const PAD_GEOMETRY_BLOCK: usize = 4;
/// The pad block with its designator: a length byte, then that many bytes (`\x01R` for "R").
const PAD_NAME_BLOCK: usize = 0;
/// Geometry bytes 7..9: the component index, `ff ff` for a free pad.
const COMPONENT_INDEX_AT: usize = 7;
const NO_COMPONENT_INDEX: u16 = 0xffff;
/// The pad block with a size, shape and corner per layer (only rounded rectangles have one).
const PAD_LAYER_TABLE_BLOCK: usize = 5;
/// Where in that block the top layer's shape is, and its corner percentage.
const LAYER_TABLE_TOP_SHAPE: usize = 640;
const LAYER_TABLE_TOP_CORNER: usize = 649;
/// Where in that block a hole that is not round keeps its type, a slot's length and its turn.
const LAYER_TABLE_HOLE_TYPE: usize = 262;
const LAYER_TABLE_SLOT_LENGTH: usize = 263;
const LAYER_TABLE_HOLE_ROTATION: usize = 267;
const SQUARE_HOLE: u8 = 1;
const SLOT_HOLE: u8 = 2;
/// The most pads with a hole that is not round named one by one in the notes.
const MAX_NAMED_HOLES: usize = 20;
/// The shape code of a rounded rectangle in that block.
const ROUNDED_RECTANGLE: u8 = 9;
/// The most records read from one stream before the file is treated as hostile.
const MAX_RECORDS: usize = 10_000_000;
/// The most layers named in `/Board6/Data` that are looked at.
const MAX_LAYER_NUMBER: u8 = u8::MAX;
const NO_NET: u16 = 0xffff;
/// Altium's mechanical layers: Mechanical 1 to 16.
const MAX_MECHANICAL: u8 = 16;
/// Byte 2 of a track, arc, fill or region body is [`KEEPOUT`] when Altium's Keepout box is ticked.
pub(crate) const KEEPOUT_AT: usize = 2;
pub(crate) const KEEPOUT: u8 = 2;
/// The name of Altium's Keep-Out layer (layer byte 56 on `keepouts`).
const KEEPOUT_LAYER: &str = "Keep-Out Layer";
/// The layer bytes of Altium's internal planes (Internal Plane 1 to 16).
const INTERNAL_PLANES: std::ops::RangeInclusive<u8> = 39..=54;

/// A hole drilled through the board (a via's, or a through-hole pad's), with the net of its via
/// or pad, for the planes it passes through (see `planes.rs`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Drill {
    pub at: Point,
    /// The hole diameter.
    pub hole: f64,
    pub net: Option<String>,
    /// What the hole is, for the plane rules' scopes.
    pub of: DrillOf,
}

/// What a drilled hole belongs to, as a rule's scope sees it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DrillOf {
    Via,
    /// A pad with the name Altium's queries give it: `Free-` and its designator for a pad of no
    /// component (`plane-polygon-connect`: `Name = 'Free-D'` matched pad D).
    Pad(String),
    /// A pad of a component, whose name in a query no test board shows yet.
    ComponentPad,
}

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
    /// Each plane layer's pull-back from the board edge (`PULLBACKDISTANCE` in the V9 stack), by
    /// the layer's name, in mm.
    pull_backs: std::collections::BTreeMap<String, f64>,
    /// For each layer byte, its mechanical layer kind, when `/Board6/Data` gives one.
    mech: Vec<Option<MechLayer>>,
}

/// What a typed mechanical layer is for (`LAYERnMECHKIND`), and its pair.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MechLayer {
    /// The kind in words, from Altium's name for it: `CourtyardBottom` is "courtyard",
    /// `ComponentOutlineTop` "component outline", `3DBodyTop` "3D body".
    pub kind: String,
    /// `Top` or `Bottom` at the end of the kind; `None` for a kind with neither.
    pub side: Option<Side>,
    /// The layer byte of the other layer of its pair (`MECHPAIRnL1`/`L2`), if it has one.
    pub pair: Option<u8>,
}

impl MechLayer {
    fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        let (base, side) = if let Some(b) = value.strip_suffix("Top") {
            (b, Some(Side::Top))
        } else if let Some(b) = value.strip_suffix("Bottom") {
            (b, Some(Side::Bottom))
        } else {
            (value, None)
        };
        if base.is_empty() {
            return None;
        }
        Some(Self {
            kind: kind_words(base),
            side,
            pair: None,
        })
    }
}

/// Altium's run-together kind name in words: a new word at each capital that follows a small
/// letter, or that starts a capitalised word; a word that is all capitals and digits ("3D")
/// keeps its case, the others are put in small letters.
fn kind_words(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    for (i, &c) in chars.iter().enumerate() {
        let before = i.checked_sub(1).and_then(|j| chars.get(j)).copied();
        let after = chars.get(i + 1).copied();
        let starts = c.is_uppercase()
            && (before.is_some_and(char::is_lowercase)
                || (before.is_some_and(|b| b.is_uppercase() || b.is_ascii_digit())
                    && after.is_some_and(char::is_lowercase)));
        if starts && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        word.push(c);
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
        .into_iter()
        .map(|w| {
            if w.chars().all(|c| c.is_uppercase() || c.is_ascii_digit()) {
                w
            } else {
                w.to_lowercase()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl CopperLayers {
    pub(crate) fn new(board: &Properties) -> Self {
        // Copper layer names in stack order, from the V9 stack (a layer with COPTHICK is copper),
        // with where each lies: the stack's thicknesses above it added up, and its own.
        let mut copper_names = Vec::new();
        let mut positions: Vec<Option<StackPosition>> = Vec::new();
        let mut depth = Some(0.0);
        let mut pull_backs = std::collections::BTreeMap::new();
        for n in 0..10_000usize {
            let Some(name) = board.get(&format!("V9_STACK_LAYER{n}_NAME")) else {
                break;
            };
            let length = |part: &str| {
                board
                    .length_mm(&format!("V9_STACK_LAYER{n}_{part}"))
                    .ok()
                    .flatten()
            };
            let dielectric = length("DIELHEIGHT").unwrap_or(0.0);
            if let Some(pull_back) = length("PULLBACKDISTANCE") {
                pull_backs.insert(name.to_string(), pull_back);
            }
            if board.get(&format!("V9_STACK_LAYER{n}_COPTHICK")).is_some() {
                let thickness = length("COPTHICK");
                copper_names.push(name.to_string());
                positions.push(
                    depth
                        .zip(thickness)
                        .map(|(depth, thickness)| StackPosition { depth, thickness }),
                );
                depth = depth.zip(thickness).map(|(d, t)| d + t + dielectric);
            } else {
                depth = depth.map(|d| d + dielectric);
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
                let position = if multi {
                    None
                } else {
                    copper_names
                        .iter()
                        .position(|c| c == name)
                        .and_then(|i| positions.get(i).copied().flatten())
                };
                layers.push(Layer {
                    name: name.to_string(),
                    kind: if INTERNAL_PLANES.contains(&n) {
                        LayerKind::Plane
                    } else {
                        LayerKind::Copper
                    },
                    side: if multi { None } else { side_of(name) },
                    position,
                });
            }
        }
        let mut mech: Vec<Option<MechLayer>> = (0..=MAX_LAYER_NUMBER)
            .map(|n| {
                board
                    .get(&format!("LAYER{n}MECHKIND"))
                    .and_then(MechLayer::parse)
            })
            .collect();
        // Mechanical k is layer byte 56 + k.
        let mechanical = |value: Option<&str>| -> Option<u8> {
            let k: u8 = value?.trim().strip_prefix("MECHANICAL")?.parse().ok()?;
            (1..=MAX_MECHANICAL).contains(&k).then(|| 56 + k)
        };
        for i in 0..=usize::from(MAX_MECHANICAL) {
            let (Some(a), Some(b)) = (
                mechanical(board.get(&format!("MECHPAIR{i}L1"))),
                mechanical(board.get(&format!("MECHPAIR{i}L2"))),
            ) else {
                continue;
            };
            for (this, other) in [(a, b), (b, a)] {
                if let Some(Some(m)) = mech.get_mut(usize::from(this)) {
                    m.pair = Some(other);
                }
            }
        }
        Self {
            layers,
            by_number,
            names,
            nets: Vec::new(),
            pull_backs,
            mech,
        }
    }

    /// The mechanical layer kind of a layer byte, when the file gives one.
    pub(crate) fn mech(&self, number: u8) -> Option<&MechLayer> {
        self.mech.get(usize::from(number))?.as_ref()
    }

    /// A layer's name for a note, with its mechanical kind and side when it has one
    /// ("Bottom Courtyard (courtyard, bottom)"); `None` when it has no kind.
    pub(crate) fn kind_label(&self, number: u8) -> Option<String> {
        let m = self.mech(number)?;
        let side = match m.side {
            Some(Side::Top) => ", top",
            Some(Side::Bottom) => ", bottom",
            None => "",
        };
        Some(format!("{} ({}{side})", self.name(number), m.kind))
    }

    /// A layer's name for a "not read yet" note: with its mechanical kind when it has one, else
    /// saying that it is neither copper nor overlay.
    pub(crate) fn off_copper_label(&self, number: u8) -> String {
        self.kind_label(number)
            .unwrap_or_else(|| format!("{} (not a copper or overlay layer)", self.name(number)))
    }

    /// The pull-back from the board edge of the plane layer at `index` in `layers`, in mm, when
    /// the stack gives one.
    pub(crate) fn pull_back(&self, index: usize) -> Option<f64> {
        self.layers
            .get(index)
            .and_then(|l| self.pull_backs.get(&l.name).copied())
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
            position: None,
        });
        Some(self.layers.len() - 1)
    }

    /// The net of a record whose body holds the net index at bytes 3..5: `Ok(None)` for no net,
    /// `Err(())` for a net index with no name in `/Nets6/Data`.
    fn net(&self, body: &[u8]) -> Result<Option<String>, ()> {
        match read_u16(body, 3) {
            None | Some(NO_NET) => Ok(None),
            Some(i) => self.nets.get(usize::from(i)).cloned().map(Some).ok_or(()),
        }
    }

    /// The index in `layers` of a copper layer that is not a plane.
    fn copper_index(&self, number: u8) -> Option<usize> {
        if INTERNAL_PLANES.contains(&number) {
            return None;
        }
        self.by_number.get(usize::from(number)).copied().flatten()
    }

    /// The index in `layers` of an internal plane layer of the stack.
    pub(crate) fn plane_index(&self, number: u8) -> Option<usize> {
        if !INTERNAL_PLANES.contains(&number) {
            return None;
        }
        self.by_number.get(usize::from(number)).copied().flatten()
    }

    /// The net named by index `index` of `/Nets6/Data`.
    pub(crate) fn net_name(&self, index: usize) -> Option<&str> {
        self.nets.get(index).map(String::as_str)
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
    /// Items on an internal plane layer, by layer name.
    on_plane: std::collections::BTreeMap<String, usize>,
    /// Whether the items on a plane are lines, which there mark where copper is left out and are
    /// already in the plane's split-plane regions; anything else on a plane is not read yet.
    plane_lines: bool,
    /// Items on the Keep-Out layer.
    keepout_layer: usize,
    /// Items on a copper layer with the Keepout box ticked, by layer name.
    keepouts: std::collections::BTreeMap<String, usize>,
}

impl Skipped {
    /// Counts an item on plane layer `number`, or returns false when that is not a plane layer.
    fn on_plane(&mut self, layers: &CopperLayers, number: u8) -> bool {
        if layers.plane_index(number).is_none() {
            return false;
        }
        *self.on_plane.entry(layers.name(number)).or_default() += 1;
        true
    }

    /// Counts and skips a keepout: an item on the Keep-Out layer, or one on a copper layer with
    /// the Keepout box ticked (byte [`KEEPOUT_AT`] of `body`).
    fn keepout(&mut self, layers: &CopperLayers, number: u8, body: &[u8]) -> bool {
        let name = layers.name(number);
        if name == KEEPOUT_LAYER {
            self.keepout_layer += 1;
            return true;
        }
        if body.get(KEEPOUT_AT) == Some(&KEEPOUT) && layers.copper_index(number).is_some() {
            *self.keepouts.entry(name).or_default() += 1;
            return true;
        }
        false
    }

    fn into_notes(self, what: &str, notes: &mut Vec<Note>) {
        keepout_notes(what, self.keepout_layer, self.keepouts, notes);
        for (layer, count) in self.on_plane {
            notes.push(if self.plane_lines {
                Note {
                    severity: Severity::Info,
                    message: format!(
                        "{count} {what} on plane layer {layer} are not drawn: on a plane they \
                         mark where copper is left out (the pull-back from the board edge, split \
                         lines), which its split-plane regions already show"
                    ),
                }
            } else {
                Note {
                    severity: Severity::Dropped,
                    message: format!("{count} {what} on plane layer {layer} not read yet"),
                }
            });
        }
        for (layer, count) in self.off_copper {
            notes.push(Note {
                severity: Severity::Dropped,
                message: format!("{count} {what} on {layer} not read yet"),
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

/// The notes for keepouts left out: those on the Keep-Out layer, and those on copper layers by
/// layer name.
fn keepout_notes(
    what: &str,
    on_layer: usize,
    flagged: std::collections::BTreeMap<String, usize>,
    notes: &mut Vec<Note>,
) {
    if on_layer > 0 {
        notes.push(Note {
            severity: Severity::Info,
            message: format!(
                "{on_layer} {what} on {KEEPOUT_LAYER} left out: a keepout is a rule for placing \
                 and routing, not part of the board"
            ),
        });
    }
    for (layer, count) in flagged {
        notes.push(Note {
            severity: Severity::Info,
            message: format!(
                "{count} {what} on {layer} are keepouts (Altium's Keepout box), not copper; left \
                 out"
            ),
        });
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
    let mut skipped = Skipped {
        plane_lines: true,
        ..Skipped::default()
    };
    for body in records(data, TRACK_RECORD, "/Tracks6/Data", notes)? {
        let layer_number = *body.first().ok_or("track record is empty")?;
        let from = point(body, 13)?;
        let to = point(body, 21)?;
        let width = length(body, 29)?;
        let net = layers.net(body).unwrap_or_else(|()| {
            skipped.unnamed_net += 1;
            None
        });
        if skipped.keepout(layers, layer_number, body) || skipped.on_plane(layers, layer_number) {
            continue;
        }
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
                .entry(layers.off_copper_label(layer_number))
                .or_default() += 1;
        }
    }
    skipped.into_notes("track(s)", notes);
    Ok(items)
}

/// A pad's designator from its name block: a length byte and that many bytes.
fn pad_designator(block: &[u8]) -> String {
    let n = block.first().copied().map_or(0, usize::from);
    String::from_utf8_lossy(block.get(1..1 + n).unwrap_or_default()).into_owned()
}

/// Reads the pads from `/Pads6/Data`. Each through-hole pad's hole also goes into `drills`.
pub(crate) fn pads(
    data: &[u8],
    layers: &CopperLayers,
    components: &Components,
    drills: &mut Vec<Drill>,
    notes: &mut Vec<Note>,
) -> Result<Vec<CopperItem>, String> {
    let mut items = Vec::new();
    let mut skipped = Skipped::default();
    let mut uneven = 0usize;
    let mut shaped_holes: Vec<String> = Vec::new();
    let mut own_masks = OwnMasks::default();
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
        let mut layer_table: &[u8] = &[];
        let mut name_block: &[u8] = &[];
        for block in 0..PAD_BLOCKS {
            let len = read_u32(data, cursor).ok_or("pad block length cut short")?;
            let start = cursor + 4;
            let end = start
                .checked_add(len)
                .filter(|&e| e <= data.len())
                .ok_or("pad block is longer than the stream")?;
            if block == PAD_NAME_BLOCK {
                name_block = data.get(start..end).unwrap_or_default();
            } else if block == PAD_GEOMETRY_BLOCK {
                geometry = data.get(start..end);
            } else if block == PAD_LAYER_TABLE_BLOCK {
                layer_table = data.get(start..end).unwrap_or_default();
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
                .entry(layers.off_copper_label(layer_number))
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
        if hole > 0.0 {
            drills.push(Drill {
                at: at_point,
                hole,
                net: layers.net(g).ok().flatten(),
                of: match read_u16(g, COMPONENT_INDEX_AT) {
                    Some(c) if c != NO_COMPONENT_INDEX => DrillOf::ComponentPad,
                    _ => DrillOf::Pad(format!("Free-{}", pad_designator(name_block))),
                },
            });
        }
        let shapes = g.get(49..52).ok_or("pad shapes cut short")?;
        let Some(rotation) = angle(g, 52)? else {
            skipped.bad_angle += 1;
            continue;
        };
        let plated = hole > 0.0 && g.get(60) == Some(&1);
        let own = pad_mask(g);
        own_masks.count(&own);
        let tented = own.tented.map(|t| t.then_some(true));
        let [top, middle, bottom] = sizes;
        let (size_x, size_y) = top;
        let mask_expansion = match own.expansion {
            Some(sides) if own.from_hole_edge && hole > 0.0 => {
                let from_pad = sides.map(|e| from_hole_edge(e, hole, size_x, size_y));
                own_masks.inside_pad += usize::from(from_pad.iter().any(|&e| e < 0.0));
                from_pad.map(Some)
            }
            Some(sides) => sides.map(Some),
            None => [None, None],
        };
        if middle != top || bottom != top || shapes.iter().any(|s| Some(s) != shapes.first()) {
            uneven += 1;
        }
        if hole > 0.0 {
            if let Some(what) = hole_shape(layer_table, hole) {
                shaped_holes.push(format!(
                    "the pad at ({:.3}, {:.3}) has {what}",
                    at_point.x, at_point.y
                ));
            }
        }
        let rounded = (layer_table.get(LAYER_TABLE_TOP_SHAPE) == Some(&ROUNDED_RECTANGLE))
            .then(|| layer_table.get(LAYER_TABLE_TOP_CORNER).copied())
            .flatten();
        let shape = match shapes.first() {
            Some(1) if rounded.is_some() => PadShape::RoundedRectangle {
                // Percent of half the shorter side, as a share of the whole shorter side.
                radius_ratio: (f64::from(rounded.unwrap_or_default()) / 200.0).clamp(0.0, 0.5),
            },
            Some(1) if size_x == size_y => PadShape::Round,
            Some(1) => PadShape::Oval,
            Some(2) => PadShape::Rectangle,
            Some(3) => {
                items.push(CopperItem::PolygonPad {
                    bottom: None,
                    layer,
                    at: at_point,
                    shapes: vec![octagon(at_point, size_x, size_y, rotation)],
                    drill: (hole > 0.0).then_some(hole),
                    plated,
                    owner: components.owner(g),
                    tented,
                    mask_expansion,
                });
                continue;
            }
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
            bottom: None,
            layer,
            at: at_point,
            shape,
            size_x,
            size_y,
            rotation_deg: rotation,
            drill: (hole > 0.0).then_some(hole),
            plated,
            owner: components.owner(g),
            tented,
            mask_expansion,
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
    let more = shaped_holes.len().saturating_sub(MAX_NAMED_HOLES);
    for what in shaped_holes.iter().take(MAX_NAMED_HOLES) {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{what}; the board model holds round holes only, so it is read as a round hole \
                 of that width"
            ),
        });
    }
    if more > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "and {more} more pad(s) with a slot or square hole, read as round holes of their \
                 width"
            ),
        });
    }
    own_masks.into_note(notes);
    skipped.into_notes("pad(s)", notes);
    Ok(items)
}

/// What a pad's hole is when it is not round, from its block 5 (see the module notes), e.g.
/// "a slot hole 0.800 wide and 2.200 long, turned 45 degrees"; `None` for a round hole.
fn hole_shape(layer_table: &[u8], width: f64) -> Option<String> {
    let turn = read_f64(layer_table, LAYER_TABLE_HOLE_ROTATION)
        .filter(|a| a.is_finite() && *a != 0.0)
        .map(|a| format!(", turned {a} degrees"))
        .unwrap_or_default();
    match *layer_table.get(LAYER_TABLE_HOLE_TYPE)? {
        SQUARE_HOLE => Some(format!("a square hole of side {width:.3}{turn}")),
        SLOT_HOLE => {
            let long = length(layer_table, LAYER_TABLE_SLOT_LENGTH).ok()?;
            Some(format!(
                "a slot hole {width:.3} wide and {long:.3} long{turn}"
            ))
        }
        _ => None,
    }
}

/// A pad's own solder and paste mask settings, from its geometry block (see the module notes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct PadMask {
    /// The mask covers the pad: index 0 is the top side, 1 the bottom.
    pub tented: [bool; 2],
    /// The solder mask expansion set on the pad (Manual), top and bottom, in mm. `None` when
    /// the pad follows the rule.
    pub expansion: Option<[f64; 2]>,
    /// The expansion is measured from the hole's edge rather than the pad's.
    pub from_hole_edge: bool,
    /// The paste mask expansion set on the pad (Manual), in mm. `None` when it follows the rule.
    pub paste: Option<f64>,
}

/// Reads a pad's own mask settings. A block too short for a field leaves that field as if set by
/// the rule.
pub(crate) fn pad_mask(g: &[u8]) -> PadMask {
    let flags = g.get(PAD_FLAGS_AT).copied().unwrap_or_default();
    let manual = |mode_at: usize| g.get(mode_at) == Some(&MASK_MANUAL);
    let expansion = manual(PAD_MASK_MODE_AT)
        .then(|| {
            let top = length(g, PAD_MASK_TOP_AT).ok()?;
            let bottom = if g.get(PAD_MASK_UNLINKED_AT) == Some(&1) {
                length(g, PAD_MASK_BOTTOM_AT).ok()?
            } else {
                top
            };
            Some([top, bottom])
        })
        .flatten();
    PadMask {
        tented: [flags & VIA_TENTED_TOP != 0, flags & VIA_TENTED_BOTTOM != 0],
        expansion,
        from_hole_edge: expansion.is_some() && g.get(PAD_MASK_FROM_HOLE_AT) == Some(&1),
        paste: manual(PAD_PASTE_MODE_AT)
            .then(|| length(g, PAD_PASTE_EXPANSION_AT).ok())
            .flatten(),
    }
}

/// A solder mask expansion `e` measured from the edge of the pad's hole, as one from the pad's
/// edge: `(hole + 2 e - size) / 2`, with the shorter side as the size of a pad that is not round.
fn from_hole_edge(e: f64, hole: f64, size_x: f64, size_y: f64) -> f64 {
    (hole + 2.0 * e - size_x.min(size_y)) / 2.0
}

/// What the notes say about the pads' own mask settings: those whose expansion from the hole's
/// edge leaves the opening smaller than the pad, and the paste mask expansions, which the board
/// model has no place for (paste is not drawn).
#[derive(Default)]
struct OwnMasks {
    inside_pad: usize,
    paste: usize,
}

impl OwnMasks {
    fn count(&mut self, mask: &PadMask) {
        self.paste += usize::from(mask.paste.is_some());
    }

    fn into_note(self, notes: &mut Vec<Note>) {
        if self.inside_pad > 0 {
            notes.push(Note {
                severity: Severity::Info,
                message: format!(
                    "{} pad(s) measure their solder mask expansion from the hole's edge, which \
                     leaves the opening smaller than the pad; read as the matching negative \
                     expansion from the pad's edge",
                    self.inside_pad
                ),
            });
        }
        if self.paste > 0 {
            notes.push(Note {
                severity: Severity::Info,
                message: format!(
                    "{} pad(s) have a paste mask expansion of their own; the paste mask is not \
                     drawn",
                    self.paste
                ),
            });
        }
    }
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
        if skipped.keepout(layers, layer_number, body) || skipped.on_plane(layers, layer_number) {
            continue;
        }
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
                        .entry(layers.off_copper_label(layer_number))
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

/// Reads the vias from `/Vias6/Data`. Every via is read as running through the whole board, and
/// its hole also goes into `drills`.
pub(crate) fn vias(
    data: &[u8],
    layers: &CopperLayers,
    drills: &mut Vec<Drill>,
    notes: &mut Vec<Note>,
) -> Result<Vec<CopperItem>, String> {
    let mut items = Vec::new();
    let mut skipped = Skipped::default();
    let (mut partial, mut spans) = (0usize, 0usize);
    for body in records(data, VIA_RECORD, "/Vias6/Data", notes)? {
        let at = point(body, 13)?;
        let diameter = length(body, 21)?;
        let drill = length(body, 25)?;
        let ends = body.get(29..31).ok_or("via layers cut short")?;
        let mut span = None;
        if let [a, b] = ends {
            if !layers.are_top_and_bottom(*a, *b) {
                // Only a copper layer with a place in the stack can end a via (not Multi-Layer).
                let stacked = |n: u8| {
                    layers
                        .copper_index(n)
                        .filter(|&i| layers.layers.get(i).is_some_and(|l| l.position.is_some()))
                };
                match (stacked(*a), stacked(*b)) {
                    (Some(i), Some(j)) => {
                        span = Some((i.min(j), i.max(j)));
                        spans += 1;
                    }
                    _ => partial += 1,
                }
            }
        }
        let tented = match body.get(VIA_FLAGS_AT) {
            Some(flags) => [
                Some(flags & VIA_TENTED_TOP != 0),
                Some(flags & VIA_TENTED_BOTTOM != 0),
            ],
            None => [None, None],
        };
        let mask_expansion = if body.get(VIA_MASK_MODE_AT) == Some(&MASK_MANUAL) {
            Some(length(body, VIA_MASK_EXPANSION_AT)?)
        } else {
            None
        };
        if read_u16(body, 3) != Some(NO_NET) {
            skipped.with_net += 1;
        }
        drills.push(Drill {
            at,
            hole: drill,
            net: layers.net(body).ok().flatten(),
            of: DrillOf::Via,
        });
        items.push(CopperItem::Via {
            at,
            diameter,
            drill,
            tented,
            mask_expansion: [mask_expansion, mask_expansion],
            layers: span,
        });
    }
    if spans > 0 {
        notes.push(Note {
            severity: Severity::Info,
            message: format!(
                "{spans} blind or buried via(s) read with the copper layers they join"
            ),
        });
    }
    if partial > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{partial} via(s) name a layer that is not a copper layer of the stack; read as \
                 running through the whole board"
            ),
        });
    }
    skipped.into_notes("via(s)", notes);
    Ok(items)
}

/// Reads the fills from `/Fills6/Data` as rectangular copper regions. A rotated fill is the
/// rectangle of its two corners turned counter-clockwise about its center (`one-rotated-fill`).
pub(crate) fn fills(
    data: &[u8],
    layers: &CopperLayers,
    notes: &mut Vec<Note>,
) -> Result<Vec<CopperItem>, String> {
    let mut items = Vec::new();
    let mut skipped = Skipped::default();
    let mut on_overlay = std::collections::BTreeMap::<String, usize>::new();
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
        if skipped.keepout(layers, layer_number, body) || skipped.on_plane(layers, layer_number) {
            continue;
        }
        let Some(layer) = layers.copper_index(layer_number) else {
            let name = layers.name(layer_number);
            if is_overlay(&name) {
                *on_overlay.entry(name).or_default() += 1;
            } else {
                *skipped
                    .off_copper
                    .entry(layers.off_copper_label(layer_number))
                    .or_default() += 1;
            }
            continue;
        };
        let center = Point {
            x: (a.x + b.x) / 2.0,
            y: (a.y + b.y) / 2.0,
        };
        let (sin, cos) = rotation.to_radians().sin_cos();
        let turn = |p: Point| Point {
            x: center.x + (p.x - center.x) * cos - (p.y - center.y) * sin,
            y: center.y + (p.x - center.x) * sin + (p.y - center.y) * cos,
        };
        let corners = [a, Point { x: b.x, y: a.y }, b, Point { x: a.x, y: b.y }].map(turn);
        items.push(CopperItem::Region {
            layer,
            contour: closed(&corners),
            holes: Vec::new(),
        });
    }
    filled_on_overlay("fill(s)", on_overlay, notes);
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
    let mut on_overlay = std::collections::BTreeMap::<String, usize>::new();
    let mut on_mask = std::collections::BTreeMap::<String, usize>::new();
    let (mut keepout_layer, mut keepouts) = (0usize, std::collections::BTreeMap::new());
    for region in others {
        let layer = layers.copper_index(region.layer);
        let name = layers.name(region.layer);
        if name == KEEPOUT_LAYER {
            keepout_layer += 1;
            continue;
        }
        if region.keepout && layer.is_some() {
            *keepouts.entry(name).or_default() += 1;
            continue;
        }
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
            (None, Some("0")) if is_overlay(&layers.name(region.layer)) => {
                *on_overlay.entry(layers.name(region.layer)).or_default() += 1;
            }
            (None, Some("0")) if is_solder_mask(&layers.name(region.layer)) => {
                *on_mask.entry(layers.name(region.layer)).or_default() += 1;
            }
            (_, kind) => {
                let what = format!(
                    "region(s) of KIND {} on {}",
                    kind.unwrap_or("?"),
                    layers
                        .kind_label(region.layer)
                        .unwrap_or_else(|| layers.name(region.layer))
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
    filled_on_overlay("region(s)", on_overlay, notes);
    keepout_notes("region(s)", keepout_layer, keepouts, notes);
    // `solder-mask-region`: a solid region on Top Solder is an opening in the mask drawn by hand.
    for (layer, count) in on_mask {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!(
                "{count} region(s) on {layer} left out: a solder mask opening drawn by hand has \
                 no place in the board model yet"
            ),
        });
    }
    items
}

/// Whether a layer is one of the solder mask layers, by its name (`Top Solder`, `Bottom Solder`).
fn is_solder_mask(name: &str) -> bool {
    name == "Top Solder" || name == "Bottom Solder"
}

/// Whether a layer is one of the silkscreen layers, by its name.
fn is_overlay(name: &str) -> bool {
    name.ends_with("Overlay")
}

/// A note for each overlay layer with filled shapes (`overlay-region-fill`): the board model
/// holds silkscreen lines, arcs and texts only, so a filled area there has no place yet.
fn filled_on_overlay(
    what: &str,
    counts: std::collections::BTreeMap<String, usize>,
    notes: &mut Vec<Note>,
) {
    for (layer, count) in counts {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!(
                "{count} {what} on {layer} left out: a filled silkscreen shape has no place in \
                 the board model yet"
            ),
        });
    }
}

/// An octagonal pad: a `size_x` x `size_y` rectangle about `at` with each corner cut off at 45
/// degrees by a quarter of the shorter side, turned counter-clockwise by `rotation_deg`.
fn octagon(at: Point, size_x: f64, size_y: f64, rotation_deg: f64) -> Contour {
    let (a, b) = (size_x / 2.0, size_y / 2.0);
    let c = size_x.min(size_y) / 4.0;
    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    let corners = [
        (a, -b + c),
        (a, b - c),
        (a - c, b),
        (-a + c, b),
        (-a, b - c),
        (-a, -b + c),
        (-a + c, -b),
        (a - c, -b),
    ]
    .map(|(x, y)| Point {
        x: at.x + x * cos - y * sin,
        y: at.y + x * sin + y * cos,
    });
    closed(&corners)
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

/// An `i32` in 1/10000 mil at `at`, in millimeters.
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

    /// mech-layer-kinds: each typed mechanical layer's kind, side and pair, and the labels the
    /// notes use.
    #[test]
    fn mechanical_layers_keep_their_kind_side_and_pair() {
        let l = CopperLayers::new(&board(
            "|LAYER58NAME=Mechanical 2|LAYER58MECHKIND=3DBodyTop\
             |LAYER59NAME=Mechanical 3|LAYER59MECHKIND=3DBodyBottom\
             |LAYER60NAME=Top Courtyard|LAYER60MECHKIND=CourtyardTop\
             |LAYER61NAME=Bottom Courtyard|LAYER61MECHKIND=CourtyardBottom\
             |LAYER64NAME=Top Component Outline|LAYER64MECHKIND=ComponentOutlineTop\
             |LAYER65NAME=Bottom Component Outline|LAYER65MECHKIND=ComponentOutlineBottom\
             |LAYER57NAME=Mechanical 1\
             |MECHPAIR0L1=MECHANICAL2|MECHPAIR0L2=MECHANICAL3\
             |MECHPAIR1L1=MECHANICAL4|MECHPAIR1L2=MECHANICAL5\r",
        ));
        let mech = |n: u8| l.mech(n).map(|m| (m.kind.as_str(), m.side, m.pair));
        assert_eq!(mech(58), Some(("3D body", Some(Side::Top), Some(59))));
        assert_eq!(mech(59), Some(("3D body", Some(Side::Bottom), Some(58))));
        assert_eq!(mech(60), Some(("courtyard", Some(Side::Top), Some(61))));
        assert_eq!(mech(61), Some(("courtyard", Some(Side::Bottom), Some(60))));
        assert_eq!(mech(64), Some(("component outline", Some(Side::Top), None)));
        assert_eq!(mech(57), None);
        assert_eq!(
            l.off_copper_label(61),
            "Bottom Courtyard (courtyard, bottom)"
        );
        assert_eq!(
            l.off_copper_label(57),
            "Mechanical 1 (not a copper or overlay layer)"
        );
        assert_eq!(kind_words("GoldPlating"), "gold plating");
        assert_eq!(kind_words("Coating"), "coating");
        assert_eq!(MechLayer::parse("Top"), None);
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
    fn a_via_to_a_layer_outside_the_stack_is_read_through_with_a_warning() {
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
        assert_eq!(
            vias(&data, &l, &mut Vec::new(), &mut notes).unwrap().len(),
            1
        );
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
        assert!(pads(
            &[PAD_RECORD, 0xff, 0xff, 0xff, 0x7f],
            &l,
            &none,
            &mut Vec::new(),
            &mut notes
        )
        .is_err());
        let mut few_blocks = vec![PAD_RECORD];
        few_blocks.extend_from_slice(&0u32.to_le_bytes());
        assert!(pads(&few_blocks, &l, &none, &mut Vec::new(), &mut notes).is_err());
        assert!(arcs(
            &record(ARC_RECORD, 40, |_| ()),
            &mut m,
            &none,
            &mut g,
            &mut notes
        )
        .is_err());
        assert!(vias(
            &record(VIA_RECORD, 30, |_| ()),
            &l,
            &mut Vec::new(),
            &mut notes
        )
        .is_err());
        assert!(arcs(
            &[ARC_RECORD, 0xff, 0xff, 0xff, 0x7f],
            &mut m,
            &none,
            &mut g,
            &mut notes
        )
        .is_err());
        assert!(vias(&[VIA_RECORD, 1, 0, 0, 0], &l, &mut Vec::new(), &mut notes).is_err());
        assert!(fills(&[FILL_RECORD, 0xff, 0xff, 0xff, 0x7f], &l, &mut notes).is_err());
        assert!(fills(&[FILL_RECORD, 2, 0, 0, 0, 1, 0], &l, &mut notes).is_err());
        assert!(tracks(&[0x42], &mut m, &none, &mut g, &mut notes)
            .unwrap()
            .is_empty());
        assert_eq!(notes.last().map(|n| n.severity), Some(Severity::Dropped));
    }

    /// The geometry block of every pad on a test board, sorted by x.
    fn pad_blocks(board: &str) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
        use std::io::Read;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/boards")
            .join(board);
        let mut file = cfb::open(path)?;
        let mut data = Vec::new();
        file.open_stream("/Pads6/Data")?.read_to_end(&mut data)?;
        let mut blocks = Vec::new();
        let mut at = 0;
        while at < data.len() {
            let mut cursor = at + 1;
            for block in 0..PAD_BLOCKS {
                let len = read_u32(&data, cursor).ok_or("cut short")?;
                if block == PAD_GEOMETRY_BLOCK {
                    blocks.push(
                        data.get(cursor + 4..cursor + 4 + len)
                            .ok_or("cut short")?
                            .to_vec(),
                    );
                }
                cursor += 4 + len;
            }
            at = cursor;
        }
        blocks.sort_by(|a, b| {
            let x = |g: &[u8]| length(g, 13).unwrap_or_default();
            x(a).total_cmp(&x(b))
        });
        Ok(blocks)
    }

    /// tests/boards/README.txt, "Pad mask overrides": P1 to P8 at x 13 to 41, each with one
    /// mask setting changed by hand. Numbers to 0.001 mm.
    #[test]
    fn reads_each_pads_own_mask_settings() -> Result<(), Box<dyn std::error::Error>> {
        let near = |a: Option<[f64; 2]>, b: [f64; 2]| {
            a.is_some_and(|a| (a[0] - b[0]).abs() < 0.001 && (a[1] - b[1]).abs() < 0.001)
        };
        let masks: Vec<PadMask> = pad_blocks("pad-mask-overrides.PcbDoc")?
            .iter()
            .map(|g| pad_mask(g))
            .collect();
        let [p1, p2, p3, p4, p5, p6, p7, p8] = masks.as_slice() else {
            return Err(format!("{masks:?}").into());
        };
        assert_eq!(*p1, PadMask::default());
        assert!(near(p2.expansion, [0.0, 0.0]), "{p2:?}");
        assert!(near(p3.expansion, [0.2, 0.2]), "{p3:?}");
        assert_eq!(p4.tented, [true, false]);
        assert!(near(p4.expansion, [0.1, 0.1]), "{p4:?}");
        assert!(near(p5.expansion, [0.2, 0.05]), "{p5:?}");
        assert_eq!(p6.tented, [false, true]);
        assert!(near(p6.expansion, [0.1, 0.1]), "{p6:?}");
        assert_eq!(p7.expansion, None);
        assert!(p7.paste.is_some_and(|p| (p - 0.05).abs() < 0.001), "{p7:?}");
        assert!(
            p8.from_hole_edge && near(p8.expansion, [0.1, 0.1]),
            "{p8:?}"
        );
        for (i, m) in masks.iter().enumerate() {
            assert_eq!(m.from_hole_edge, i == 7, "{m:?}");
            assert_eq!(m.paste.is_some(), i == 6, "{m:?}");
            assert_eq!(m.tented.contains(&true), i == 3 || i == 5, "{m:?}");
        }
        Ok(())
    }

    #[test]
    fn a_short_block_reads_as_by_the_rule() {
        assert_eq!(pad_mask(&[]), PadMask::default());
        let mut g = vec![0u8; PAD_MASK_MODE_AT + 1];
        g[PAD_MASK_MODE_AT] = MASK_MANUAL;
        assert_eq!(pad_mask(&g).expansion, Some([0.0, 0.0]));
        g.truncate(PAD_MASK_TOP_AT + 2);
        assert_eq!(pad_mask(&g), PadMask::default());
    }
}
