//! tscircuit Circuit JSON reader: a tscircuit board's compiled output into the neutral board
//! model.
//!
//! Circuit JSON is one JSON array of typed elements (`{"type": "pcb_board", ...}`), lengths in
//! millimetres, angles in degrees, Y up, usually centred on the board. Its public description is
//! the README of the `circuit-json` package (ISC licence); this reader was written from that
//! description and from boards we generated ourselves (see `tests/boards/README.txt`), tested
//! against circuit-json 0.0.560 as written by tscircuit 0.0.3288.
//!
//! What is read: the board outline (rectangle or polygon) and thickness, holes and cutouts,
//! plated holes, vias, SMT pads, traces, copper pours, silkscreen text, and component
//! placements. Everything else is counted in a [`Note`]; nothing is dropped silently.

use std::collections::BTreeMap;

use board_model::{
    Appearance, Board, Contour, CopperItem, Detection, InputFormat, Layer, LayerKind, Note, Outline, PadShape,
    Placement, Point, ReadError, ReadOutcome, Segment, Severity, Side, Text, TextFont, MAX_INPUT_BYTES,
};
use serde_json::Value;

/// The most elements one file may hold before it is refused as hostile.
const MAX_ELEMENTS: usize = 2_000_000;
/// The most points one outline, route or polygon may hold.
const MAX_POINTS: usize = 1_000_000;
/// The most copper layers a board may claim.
const MAX_LAYERS: u64 = 64;
/// The most tscircuit errors copied into notes one by one; the rest are counted.
const MAX_LISTED_ERRORS: usize = 20;
/// Lengths larger than this (mm) are refused as not a real board.
const MAX_LENGTH: f64 = 100_000.0;

/// The tscircuit Circuit JSON format.
#[derive(Clone, Copy, Debug, Default)]
pub struct CircuitJson;

impl InputFormat for CircuitJson {
    fn id(&self) -> &'static str {
        "tscircuit_circuit_json"
    }

    fn name(&self) -> &'static str {
        "tscircuit Circuit JSON (.circuit.json)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        // `.tsx` is a tscircuit source; the command compiles it to Circuit JSON first.
        &["circuit.json", "tsx"]
    }

    fn detect(&self, head: &[u8], extension: Option<&str>) -> Detection {
        let named = extension.is_some_and(|e| {
            e.eq_ignore_ascii_case("circuit.json") || e.eq_ignore_ascii_case("tsx")
        });
        let text = String::from_utf8_lossy(head);
        let looks = text.trim_start().starts_with('[')
            && (text.contains("\"type\":\"source_")
                || text.contains("\"type\": \"source_")
                || text.contains("\"type\":\"pcb_")
                || text.contains("\"type\": \"pcb_"));
        match (looks, named) {
            (true, _) => Detection::Yes,
            (false, true) => Detection::MaybeByName,
            (false, false) => Detection::No,
        }
    }

    fn read(&self, bytes: &[u8]) -> Result<ReadOutcome, ReadError> {
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(ReadError::TooLarge { bytes: bytes.len() });
        }
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|e| ReadError::Invalid(format!("not JSON: {e}")))?;
        let elements = value
            .as_array()
            .ok_or_else(|| ReadError::Invalid("Circuit JSON is a list of elements".into()))?;
        if elements.len() > MAX_ELEMENTS {
            return Err(ReadError::Invalid(format!(
                "more than {MAX_ELEMENTS} elements"
            )));
        }
        Reader::default().read(elements)
    }
}

// ---------------------------------------------------------------------------------------------
// Small readers for JSON values
// ---------------------------------------------------------------------------------------------

fn text<'a>(e: &'a Value, key: &str) -> Option<&'a str> {
    e.get(key).and_then(Value::as_str)
}

/// A length in mm: a number, or a string with a unit ("2mm", "100mil", "0.1in").
fn length_of(v: &Value) -> Option<f64> {
    let n = match v {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => {
            let s = s.trim();
            let (num, factor) = if let Some(n) = s.strip_suffix("mm") {
                (n, 1.0)
            } else if let Some(n) = s.strip_suffix("mil") {
                (n, 0.0254)
            } else if let Some(n) = s.strip_suffix("in") {
                (n, 25.4)
            } else {
                (s, 1.0)
            };
            num.trim().parse::<f64>().ok()? * factor
        }
        _ => return None,
    };
    (n.is_finite() && n.abs() <= MAX_LENGTH).then_some(n)
}

fn length(e: &Value, key: &str) -> Option<f64> {
    e.get(key).and_then(length_of)
}

/// An angle in degrees: a number, or a string like "90deg".
fn angle(e: &Value, key: &str) -> Option<f64> {
    let v = e.get(key)?;
    let n = match v {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s
            .trim()
            .trim_end_matches("deg")
            .trim()
            .parse::<f64>()
            .ok()?,
        _ => return None,
    };
    (n.is_finite() && n.abs() <= 3600.0).then_some(n)
}

fn point_of(v: &Value) -> Option<Point> {
    Some(Point {
        x: length(v, "x")?,
        y: length(v, "y")?,
    })
}

fn point(e: &Value, key: &str) -> Option<Point> {
    e.get(key).and_then(point_of)
}

/// `x`/`y` fields of the element itself.
fn xy(e: &Value) -> Option<Point> {
    point_of(e)
}

fn points(e: &Value, key: &str) -> Option<Vec<Point>> {
    let list = e.get(key)?.as_array()?;
    if list.len() > MAX_POINTS {
        return None;
    }
    list.iter().map(point_of).collect()
}

// ---------------------------------------------------------------------------------------------
// Plane shapes
// ---------------------------------------------------------------------------------------------

fn rotate(p: Point, about: Point, deg: f64) -> Point {
    let (s, c) = deg.to_radians().sin_cos();
    let (dx, dy) = (p.x - about.x, p.y - about.y);
    Point {
        x: about.x + dx * c - dy * s,
        y: about.y + dx * s + dy * c,
    }
}

fn polygon(points: &[Point]) -> Contour {
    let segments = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| Segment::Line { from: *a, to: *b })
        .collect();
    Contour { segments }
}

fn circle(center: Point, radius: f64) -> Contour {
    Contour {
        segments: vec![Segment::Arc {
            center,
            radius,
            start_deg: 0.0,
            end_deg: 360.0,
            clockwise: false,
        }],
    }
}

/// A rectangle centred on `center`, turned by `rotation` degrees, with rounded corners when
/// `corner` is above zero.
fn rect(center: Point, width: f64, height: f64, rotation: f64, corner: f64) -> Contour {
    let (hw, hh) = (width / 2.0, height / 2.0);
    let r = corner.clamp(0.0, hw.min(hh));
    let at = |x: f64, y: f64| {
        rotate(
            Point {
                x: center.x + x,
                y: center.y + y,
            },
            center,
            rotation,
        )
    };
    if r <= 1e-9 {
        return polygon(&[at(-hw, -hh), at(hw, -hh), at(hw, hh), at(-hw, hh)]);
    }
    let mut segments = Vec::new();
    // Counter-clockwise from the bottom edge; each corner is a quarter arc.
    let corners = [
        ((hw - r, -hh + r), -90.0),
        ((hw - r, hh - r), 0.0),
        ((-hw + r, hh - r), 90.0),
        ((-hw + r, -hh + r), 180.0),
    ];
    let mut previous: Option<Point> = None;
    for ((cx, cy), start) in corners {
        let c = at(cx, cy);
        let a0 = start + rotation;
        let from = Point {
            x: c.x + r * a0.to_radians().cos(),
            y: c.y + r * a0.to_radians().sin(),
        };
        if let Some(p) = previous {
            segments.push(Segment::Line { from: p, to: from });
        }
        segments.push(Segment::Arc {
            center: c,
            radius: r,
            start_deg: a0,
            end_deg: a0 + 90.0,
            clockwise: false,
        });
        let a1 = (a0 + 90.0).to_radians();
        previous = Some(Point {
            x: c.x + r * a1.cos(),
            y: c.y + r * a1.sin(),
        });
    }
    if let (
        Some(p),
        Some(Segment::Arc {
            center,
            radius,
            start_deg,
            ..
        }),
    ) = (previous, segments.first())
    {
        let to = Point {
            x: center.x + radius * start_deg.to_radians().cos(),
            y: center.y + radius * start_deg.to_radians().sin(),
        };
        segments.push(Segment::Line { from: p, to });
    }
    Contour { segments }
}

/// A pill (a rectangle with fully round ends) of the given width and height.
fn pill(center: Point, width: f64, height: f64, rotation: f64) -> Contour {
    rect(center, width, height, rotation, width.min(height) / 2.0)
}

// ---------------------------------------------------------------------------------------------
// The reader
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct Reader {
    notes: Vec<Note>,
    /// Counted things not read, by description.
    skipped: BTreeMap<String, usize>,
    /// Elements with a missing or unusable field, by type.
    unusable: BTreeMap<String, usize>,
}

impl Reader {
    fn skip(&mut self, what: &str) {
        let n = self.skipped.entry(what.to_string()).or_default();
        *n = n.saturating_add(1);
    }

    fn bad(&mut self, kind: &str) {
        let n = self.unusable.entry(kind.to_string()).or_default();
        *n = n.saturating_add(1);
    }

    fn read(mut self, elements: &[Value]) -> Result<ReadOutcome, ReadError> {
        let of = |t: &str| {
            elements
                .iter()
                .filter(move |e| text(e, "type") == Some(t))
                .collect::<Vec<_>>()
        };

        // The board.
        let boards = of("pcb_board");
        let board = boards
            .first()
            .ok_or_else(|| ReadError::Invalid("no pcb_board element".into()))?;
        if boards.len() > 1 {
            self.notes.push(Note {
                severity: Severity::Dropped,
                message: format!(
                    "{} more board(s) in this file (a panel?) were not read; only the first",
                    boards.len().saturating_sub(1)
                ),
            });
        }
        let outer = match points(board, "outline") {
            Some(p) if p.len() >= 3 => polygon(&p),
            _ => {
                let center = point(board, "center").unwrap_or_default();
                let width = length(board, "width");
                let height = length(board, "height");
                match (width, height) {
                    (Some(w), Some(h)) if w > 0.0 && h > 0.0 => rect(center, w, h, 0.0, 0.0),
                    _ => {
                        return Err(ReadError::Invalid(
                            "the board has neither an outline nor a width and height".into(),
                        ))
                    }
                }
            }
        };
        let thickness = length(board, "thickness").filter(|t| *t > 0.0);
        let copper_count = board
            .get("num_layers")
            .and_then(Value::as_u64)
            .unwrap_or(2)
            .clamp(1, MAX_LAYERS);

        // Layers: copper top, inner1.., bottom; then the two silkscreens.
        let mut layers = Vec::new();
        let mut layer_index: BTreeMap<String, usize> = BTreeMap::new();
        let mut add_layer = |name: String, kind: LayerKind, side: Option<Side>| {
            layer_index.insert(name.clone(), layers.len());
            layers.push(Layer {
                name,
                kind,
                side,
                // tscircuit gives no stack-up (copper thickness, layer depth) in Circuit JSON.
                position: None,
            });
        };
        add_layer("top".into(), LayerKind::Copper, Some(Side::Top));
        for i in 1..copper_count.saturating_sub(1) {
            add_layer(format!("inner{i}"), LayerKind::Copper, None);
        }
        if copper_count > 1 {
            add_layer("bottom".into(), LayerKind::Copper, Some(Side::Bottom));
        }
        let silk_top = layers.len();
        layers.push(Layer {
            name: "top silkscreen".into(),
            kind: LayerKind::Silkscreen,
            side: Some(Side::Top),
            position: None,
        });
        let silk_bottom = layers.len();
        layers.push(Layer {
            name: "bottom silkscreen".into(),
            kind: LayerKind::Silkscreen,
            side: Some(Side::Bottom),
            position: None,
        });
        let multi = layers.len();
        layers.push(Layer {
            name: "all copper layers".into(),
            kind: LayerKind::Copper,
            side: None,
            position: None,
        });
        let copper_layer = |name: Option<&str>| name.and_then(|n| layer_index.get(n).copied());

        let mut cutouts = Vec::new();
        let mut copper = Vec::new();
        let mut texts = Vec::new();
        let mut components = Vec::new();

        // Names of components, by source id.
        let names: BTreeMap<&str, &str> = of("source_component")
            .iter()
            .filter_map(|e| Some((text(e, "source_component_id")?, text(e, "name")?)))
            .collect();
        let footprints: BTreeMap<&str, &str> = of("cad_component")
            .iter()
            .filter_map(|e| Some((text(e, "pcb_component_id")?, text(e, "footprinter_string")?)))
            .collect();

        for e in elements {
            let Some(kind) = text(e, "type") else {
                self.bad("element without a type");
                continue;
            };
            match kind {
                "pcb_board" | "pcb_port" => {}
                "pcb_hole" => match hole_contour(e) {
                    Some(c) => cutouts.push(c),
                    None => self.bad(kind),
                },
                "pcb_cutout" => match cutout_contour(e) {
                    Some(c) => cutouts.push(c),
                    None => self.bad(kind),
                },
                "pcb_plated_hole" => match plated_hole(e, multi) {
                    Some((pad, slot)) => {
                        copper.push(pad);
                        if let Some(s) = slot {
                            cutouts.push(s);
                        }
                    }
                    None => self.bad(kind),
                },
                "pcb_via" => match (xy(e), length(e, "outer_diameter"), length(e, "hole_diameter")) {
                    (Some(at), Some(diameter), Some(drill)) => copper.push(CopperItem::Via {
                        at,
                        diameter,
                        drill,
                    }),
                    _ => self.bad(kind),
                },
                "pcb_smtpad" => match smt_pad(e, copper_layer(text(e, "layer"))) {
                    Some(pad) => copper.push(pad),
                    None => self.bad(kind),
                },
                "pcb_trace" => match traces(e, &copper_layer) {
                    Some((tracks, vias)) => {
                        copper.extend(tracks);
                        copper.extend(vias);
                    }
                    None => self.bad(kind),
                },
                "pcb_copper_pour" => {
                    match (pour_contour(e), copper_layer(text(e, "layer"))) {
                        (Some(contour), Some(layer)) => {
                            copper.push(CopperItem::Region {
                                layer,
                                contour,
                                // Circuit JSON pours carry no cut-outs around other copper.
                                holes: Vec::new(),
                            })
                        }
                        _ => self.bad(kind),
                    }
                }
                "pcb_silkscreen_text" => {
                    let layer = if text(e, "layer") == Some("bottom") {
                        silk_bottom
                    } else {
                        silk_top
                    };
                    match (point(e, "anchor_position"), text(e, "text")) {
                        (Some(at), Some(content)) => {
                            if text(e, "anchor_alignment").is_some_and(|a| a != "center") {
                                self.skip("silkscreen text anchors other than the centre (placed by their anchor point)");
                            }
                            let height = length(e, "font_size").unwrap_or(1.0);
                            texts.push(Text {
                                layer,
                                at,
                                content: content.to_string(),
                                height,
                                rotation_deg: angle(e, "ccw_rotation").unwrap_or(0.0),
                                mirrored: layer == silk_bottom
                                    || e.get("is_mirrored").and_then(Value::as_bool) == Some(true),
                                // tscircuit names no font for silkscreen text; its pen width
                                // is not in Circuit JSON either, so a default stroke is used.
                                font: TextFont::Stroke { width: height / 8.0 },
                                component: None,
                            });
                        }
                        _ => self.bad(kind),
                    }
                }
                "pcb_silkscreen_line" | "pcb_silkscreen_path" | "pcb_silkscreen_rect"
                | "pcb_silkscreen_circle" | "pcb_silkscreen_oval" | "pcb_silkscreen_pill"
                | "pcb_silkscreen_graphic" => {
                    self.skip("silkscreen graphics (lines, paths, rectangles, circles): not read yet, though the board model has silkscreen lines and arcs now")
                }
                "pcb_component" => {
                    let designator = text(e, "source_component_id")
                        .and_then(|s| names.get(s).copied());
                    let Some(designator) = designator else {
                        // Vias and other placed items carry a pcb_component without a name.
                        continue;
                    };
                    match point(e, "center") {
                        Some(at) => components.push(Placement {
                            designator: designator.to_string(),
                            footprint: text(e, "pcb_component_id")
                                .and_then(|id| footprints.get(id).copied())
                                .unwrap_or_default()
                                .to_string(),
                            at,
                            rotation_deg: angle(e, "rotation").unwrap_or(0.0),
                            side: if text(e, "layer") == Some("bottom") {
                                Side::Bottom
                            } else {
                                Side::Top
                            },
                            model: None,
                            comment: String::new(),
                        }),
                        None => self.bad(kind),
                    }
                }
                "pcb_solder_paste" => self.skip("solder paste openings (not part of the board model)"),
                "pcb_courtyard_rect" | "pcb_courtyard_outline" | "pcb_courtyard_circle"
                | "pcb_courtyard_polygon" | "pcb_courtyard_pill" => {
                    self.skip("courtyards (not part of the board model)")
                }
                k if k.starts_with("pcb_fabrication_note") => {
                    self.skip("fabrication notes (not part of the board model)")
                }
                k if k.ends_with("_error") => {
                    let listed = self
                        .notes
                        .iter()
                        .filter(|n| n.message.starts_with("tscircuit reported"))
                        .count();
                    if listed < MAX_LISTED_ERRORS {
                        let message = text(e, "message").unwrap_or(k);
                        self.notes.push(Note {
                            severity: Severity::Warning,
                            message: format!("tscircuit reported: {message}"),
                        });
                    } else {
                        self.skip("more errors tscircuit reported");
                    }
                }
                k if k.starts_with("pcb_") => self.skip(&format!("{k} elements (not read yet)")),
                _ => {}
            }
        }

        let modelled = components.len();
        if modelled > 0 {
            self.notes.push(Note {
                severity: Severity::Dropped,
                message: format!(
                    "{modelled} component(s) have no 3D model in the Circuit JSON (tscircuit \
                     builds them only when it exports); placed without models"
                ),
            });
        }
        for (what, count) in std::mem::take(&mut self.skipped) {
            self.notes.push(Note {
                severity: Severity::Dropped,
                message: format!("{count} {what}"),
            });
        }
        for (kind, count) in std::mem::take(&mut self.unusable) {
            self.notes.push(Note {
                severity: Severity::Dropped,
                message: format!(
                    "{count} {kind} element(s) with a missing or unusable field were not read"
                ),
            });
        }

        Ok(ReadOutcome {
            board: Board {
                name: String::new(),
                thickness,
                layers,
                outline: Outline { outer, cutouts },
                copper,
                graphics: Vec::new(),
                texts,
                components,
                appearance: Appearance::default(),
                solder_mask_expansion: None,
            },
            notes: self.notes,
            models: Vec::new(),
            fonts: Vec::new(),
        })
    }
}

fn hole_contour(e: &Value) -> Option<Contour> {
    let at = xy(e)?;
    match text(e, "hole_shape").unwrap_or("circle") {
        "circle" => Some(circle(at, length(e, "hole_diameter")? / 2.0)),
        "square" => {
            let d = length(e, "hole_diameter")?;
            Some(rect(at, d, d, 0.0, 0.0))
        }
        "rect" => Some(rect(
            at,
            length(e, "hole_width")?,
            length(e, "hole_height")?,
            angle(e, "ccw_rotation").unwrap_or(0.0),
            0.0,
        )),
        "oval" | "pill" | "rotated_pill" => Some(pill(
            at,
            length(e, "hole_width")?,
            length(e, "hole_height")?,
            angle(e, "ccw_rotation").unwrap_or(0.0),
        )),
        _ => None,
    }
}

fn cutout_contour(e: &Value) -> Option<Contour> {
    match text(e, "shape")? {
        "rect" => Some(rect(
            point(e, "center")?,
            length(e, "width")?,
            length(e, "height")?,
            angle(e, "rotation").unwrap_or(0.0),
            length(e, "corner_radius").unwrap_or(0.0),
        )),
        "circle" => Some(circle(point(e, "center")?, length(e, "radius")?)),
        "polygon" => {
            let p = points(e, "points")?;
            (p.len() >= 3).then(|| polygon(&p))
        }
        _ => None,
    }
}

fn pour_contour(e: &Value) -> Option<Contour> {
    match text(e, "shape")? {
        "rect" => Some(rect(
            point(e, "center")?,
            length(e, "width")?,
            length(e, "height")?,
            angle(e, "rotation").unwrap_or(0.0),
            0.0,
        )),
        "polygon" => {
            let p = points(e, "points")?;
            (p.len() >= 3).then(|| polygon(&p))
        }
        _ => None,
    }
}

/// A plated hole as a pad on every copper layer, plus a board opening when the drill is not
/// round (the board model's pad drill is a diameter).
fn plated_hole(e: &Value, multi: usize) -> Option<(CopperItem, Option<Contour>)> {
    let at = xy(e)?;
    let shape = text(e, "shape")?;
    let offset = Point {
        x: at.x + length(e, "hole_offset_x").unwrap_or(0.0),
        y: at.y + length(e, "hole_offset_y").unwrap_or(0.0),
    };
    let pad = |shape: PadShape, size_x: f64, size_y: f64, rotation: f64, drill: Option<f64>| {
        CopperItem::Pad {
            layer: multi,
            at,
            shape,
            size_x,
            size_y,
            rotation_deg: rotation,
            drill,
            plated: drill.is_some(),
            owner: None,
        }
    };
    match shape {
        "circle" => {
            let d = length(e, "outer_diameter")?;
            Some((
                pad(
                    PadShape::Round,
                    d,
                    d,
                    0.0,
                    Some(length(e, "hole_diameter")?),
                ),
                None,
            ))
        }
        "oval" | "pill" => {
            let (w, h) = (length(e, "outer_width")?, length(e, "outer_height")?);
            let slot = pill(at, length(e, "hole_width")?, length(e, "hole_height")?, 0.0);
            Some((pad(PadShape::Oval, w, h, 0.0, None), Some(slot)))
        }
        "circular_hole_with_rect_pad" => {
            let drill = length(e, "hole_diameter")?;
            let rotation = angle(e, "rect_ccw_rotation").unwrap_or(0.0);
            let item = pad(
                PadShape::Rectangle,
                length(e, "rect_pad_width")?,
                length(e, "rect_pad_height")?,
                rotation,
                None,
            );
            // An offset drill cannot be a pad drill (the pad's drill is at its centre).
            let hole = circle(offset, drill / 2.0);
            Some((item, Some(hole)))
        }
        "pill_hole_with_rect_pad" | "rotated_pill_hole_with_rect_pad" => {
            let rotation = angle(e, "rect_ccw_rotation").unwrap_or(0.0);
            let hole_rotation = angle(e, "hole_ccw_rotation").unwrap_or(0.0);
            let item = pad(
                PadShape::Rectangle,
                length(e, "rect_pad_width")?,
                length(e, "rect_pad_height")?,
                rotation,
                None,
            );
            let slot = pill(
                offset,
                length(e, "hole_width")?,
                length(e, "hole_height")?,
                hole_rotation,
            );
            Some((item, Some(slot)))
        }
        _ => None,
    }
}

fn smt_pad(e: &Value, layer: Option<usize>) -> Option<CopperItem> {
    let layer = layer?;
    let at = xy(e)?;
    let (shape, size_x, size_y, rotation) = match text(e, "shape")? {
        "rect" => (
            PadShape::Rectangle,
            length(e, "width")?,
            length(e, "height")?,
            0.0,
        ),
        "rotated_rect" => (
            PadShape::Rectangle,
            length(e, "width")?,
            length(e, "height")?,
            angle(e, "ccw_rotation").unwrap_or(0.0),
        ),
        "circle" => {
            let d = length(e, "radius")? * 2.0;
            (PadShape::Round, d, d, 0.0)
        }
        "pill" | "rotated_pill" => (
            PadShape::Oval,
            length(e, "width")?,
            length(e, "height")?,
            angle(e, "ccw_rotation").unwrap_or(0.0),
        ),
        _ => return None,
    };
    Some(CopperItem::Pad {
        layer,
        at,
        shape,
        size_x,
        size_y,
        rotation_deg: rotation,
        drill: None,
        plated: false,
        owner: None,
    })
}

type Tracks = (Vec<CopperItem>, Vec<CopperItem>);

/// A trace's route: wire points on one layer become tracks between them; via points become
/// vias (tscircuit's default via size when the point gives none).
fn traces(e: &Value, layer_of: &dyn Fn(Option<&str>) -> Option<usize>) -> Option<Tracks> {
    let route = e.get("route")?.as_array()?;
    if route.len() > MAX_POINTS {
        return None;
    }
    let mut tracks = Vec::new();
    let mut vias = Vec::new();
    let mut previous: Option<(Point, Option<usize>, f64)> = None;
    for p in route {
        let at = xy(p)?;
        match text(p, "route_type") {
            Some("via") => {
                vias.push(CopperItem::Via {
                    at,
                    diameter: length(p, "via_diameter").unwrap_or(0.6),
                    drill: length(p, "hole_diameter").unwrap_or(0.3),
                });
                previous = None;
            }
            _ => {
                let layer = layer_of(text(p, "layer"));
                let width = length(p, "width").unwrap_or(0.15);
                if let Some((from, prev_layer, prev_width)) = previous {
                    if let (Some(l), true) = (layer, prev_layer == layer) {
                        tracks.push(CopperItem::Track {
                            layer: l,
                            from,
                            to: at,
                            width: prev_width,
                            net: None,
                        });
                    }
                }
                previous = Some((at, layer, width));
            }
        }
    }
    Some((tracks, vias))
}

#[cfg(test)]
mod tests;
