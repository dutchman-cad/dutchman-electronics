//! Fills the board model from a parsed `.kicad_pcb`.
//!
//! Everything here follows KiCad's published file-format documentation (see README.md for the
//! pages) and our own test boards. Lengths in the file are millimeters with Y pointing down; the
//! model has Y pointing up, so every Y changes sign here and nowhere else.

use std::collections::{BTreeMap, HashMap};

use board_model::{
    Board, Contour, CopperItem, Graphic, Layer, LayerKind, ModelRef, ModelSource, Note, PadShape,
    Placement, Point, ReadOutcome, Segment, Severity, Side, StackPosition, Text, TextFont,
};

use crate::geom::{arc_through, circle_segment, place, pts_contour, split_fill, Frame};
use crate::outline;
use crate::padshape;
use crate::sexpr::Node;

/// The first board file version written by KiCad 9. Older files get a note instead of a guess.
pub const FIRST_SUPPORTED_VERSION: u64 = 20_241_229;

/// The name the model gives the one layer that through-hole pads sit on.
const ALL_COPPER: &str = "*.Cu";

/// What the reader counted and could not use, turned into notes at the end.
#[derive(Default)]
struct Tally {
    counts: BTreeMap<(u8, String), usize>,
}

impl Tally {
    fn add(&mut self, severity: Severity, what: impl Into<String>) {
        let rank = match severity {
            Severity::Dropped => 0,
            Severity::Warning => 1,
            Severity::Info => 2,
        };
        let n = self.counts.entry((rank, what.into())).or_default();
        *n = n.saturating_add(1);
    }

    fn into_notes(self) -> Vec<Note> {
        self.counts
            .into_iter()
            .map(|((rank, what), n)| Note {
                severity: match rank {
                    0 => Severity::Dropped,
                    1 => Severity::Warning,
                    _ => Severity::Info,
                },
                message: if n == 1 {
                    format!("1 {what}")
                } else {
                    format!("{n} {what}")
                },
            })
            .collect()
    }
}

/// Which face and kind a canonical layer name has.
fn classify(name: &str) -> (LayerKind, Option<Side>) {
    let side = if name.starts_with("F.") {
        Some(Side::Top)
    } else if name.starts_with("B.") {
        Some(Side::Bottom)
    } else {
        None
    };
    let kind = if name.ends_with(".Cu") {
        LayerKind::Copper
    } else if name.ends_with(".SilkS") {
        LayerKind::Silkscreen
    } else if name.ends_with(".Mask") {
        LayerKind::SolderMask
    } else if name.ends_with(".Paste") {
        LayerKind::Paste
    } else if name == "Edge.Cuts" {
        LayerKind::Outline
    } else {
        LayerKind::Other
    };
    (kind, side)
}

/// Where a layer sits in the stack, top face first.
fn stack_rank(name: &str, file_order: usize) -> usize {
    match name {
        "F.SilkS" => 0,
        "F.Paste" => 1,
        "F.Mask" => 2,
        "F.Cu" => 3,
        "B.Cu" => 100,
        "B.Mask" => 101,
        "B.Paste" => 102,
        "B.SilkS" => 103,
        _ => match name
            .strip_prefix("In")
            .and_then(|r| r.strip_suffix(".Cu"))
            .and_then(|n| n.parse::<usize>().ok())
        {
            Some(n) => 10 + n.min(80),
            None => 200 + file_order,
        },
    }
}

/// A piece of text's position in the file's terms.
struct TextSpot {
    x: f64,
    y: f64,
    angle: f64,
}

struct Reader {
    layers: Vec<Layer>,
    by_name: HashMap<String, usize>,
    nets: HashMap<i64, String>,
    tally: Tally,
    copper: Vec<CopperItem>,
    graphics: Vec<Graphic>,
    texts: Vec<Text>,
    components: Vec<Placement>,
    edge_open: Vec<Segment>,
    edge_closed: Vec<Contour>,
    inexact_text: bool,
}

/// What a drawn shape is, in Y-up board coordinates.
enum Shape {
    Line(Point, Point),
    Arc(crate::geom::Arc3),
    Circle(Point, f64),
    /// A closed outline (polygons and rectangles).
    Poly(Contour),
}

impl Reader {
    fn layer_index(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    fn all_copper(&mut self) -> usize {
        if let Some(i) = self.layer_index(ALL_COPPER) {
            return i;
        }
        self.layers.push(Layer {
            name: ALL_COPPER.into(),
            kind: LayerKind::Copper,
            side: None,
            position: None,
        });
        let i = self.layers.len() - 1;
        self.by_name.insert(ALL_COPPER.into(), i);
        i
    }

    fn net_name(&self, node: &Node) -> Option<String> {
        let net = node.child("net")?;
        let arg = net.args().first()?;
        match arg.number() {
            Some(n) => self
                .nets
                .get(&(n as i64))
                .cloned()
                .filter(|s| !s.is_empty()),
            None => arg.text().map(str::to_string).filter(|s| !s.is_empty()),
        }
    }

    // -----------------------------------------------------------------------------------------
    // Drawn shapes
    // -----------------------------------------------------------------------------------------

    /// The shape a `gr_*` or `fp_*` item draws, placed with `frame`. `None` when the numbers are
    /// unusable.
    fn shape(&mut self, kind: &str, n: &Node, f: &Frame) -> Option<Shape> {
        match kind {
            "line" => Some(Shape::Line(
                place(f, n.child("start")?)?,
                place(f, n.child("end")?)?,
            )),
            "arc" => {
                let s = place(f, n.child("start")?)?;
                let m = place(f, n.child("mid")?)?;
                let e = place(f, n.child("end")?)?;
                match arc_through(s, m, e) {
                    Some(a) => Some(Shape::Arc(a)),
                    None => {
                        self.tally.add(
                            Severity::Warning,
                            "arc(s) with all three points on one line were read as lines",
                        );
                        Some(Shape::Line(s, e))
                    }
                }
            }
            "circle" => {
                let c = place(f, n.child("center")?)?;
                let e = place(f, n.child("end")?)?;
                let r = crate::geom::dist(c, e);
                (r > 0.0).then_some(Shape::Circle(c, r))
            }
            "rect" => {
                let a = n.child("start")?.pair()?;
                let b = n.child("end")?.pair()?;
                let corners =
                    [(a.0, a.1), (b.0, a.1), (b.0, b.1), (a.0, b.1)].map(|(x, y)| f.point(x, y));
                Some(Shape::Poly(outline::polygon(&corners)))
            }
            "poly" => Some(Shape::Poly(pts_contour(f, n.child("pts")?)?)),
            _ => None,
        }
    }

    /// The line width of an item: 0 when it names none, `None` when it names one that cannot be
    /// read (a number beyond the sane range, or negative), so the item is dropped.
    fn stroke_width(n: &Node) -> Option<f64> {
        match n
            .child("stroke")
            .and_then(|s| s.child("width"))
            .or_else(|| n.child("width"))
        {
            None => Some(0.0),
            Some(w) => w.num(0).filter(|w| *w >= 0.0),
        }
    }

    /// One `gr_*` / `fp_*` shape item.
    fn graphic_item(&mut self, kind: &str, n: &Node, f: &Frame, owner: Option<&str>) {
        if !matches!(kind, "line" | "arc" | "circle" | "rect" | "poly") {
            self.tally.add(
                Severity::Dropped,
                format!("drawing item(s) of kind \"{kind}\" (not read)"),
            );
            return;
        }
        let Some(layer_name) = n.child("layer").and_then(|l| l.str_at(0)) else {
            self.tally
                .add(Severity::Dropped, "drawing item(s) without a layer");
            return;
        };
        let Some(shape) = self.shape(kind, n, f) else {
            self.tally
                .add(Severity::Dropped, "drawing item(s) with unreadable numbers");
            return;
        };
        if layer_name == "Edge.Cuts" {
            match shape {
                Shape::Line(a, b) => self.edge_open.push(Segment::Line { from: a, to: b }),
                Shape::Arc(a) => self.edge_open.push(a.segment()),
                Shape::Circle(c, r) => self.edge_closed.push(Contour {
                    segments: vec![circle_segment(c, r)],
                }),
                Shape::Poly(c) => self.edge_closed.push(c),
            }
            return;
        }
        let Some(layer) = self.layer_index(layer_name) else {
            self.tally.add(
                Severity::Dropped,
                "drawing item(s) on a layer the board does not list",
            );
            return;
        };
        let kind_of = self.layers.get(layer).map(|l| l.kind);
        let Some(width) = Self::stroke_width(n) else {
            self.tally
                .add(Severity::Dropped, "drawing item(s) with unreadable numbers");
            return;
        };
        let filled = n
            .child("fill")
            .is_some_and(|x| matches!(x.str_at(0), Some("yes") | Some("solid")))
            || n.is_yes("fill");
        match kind_of {
            Some(LayerKind::Silkscreen) => {
                if filled {
                    self.tally.add(
                        Severity::Warning,
                        "filled silkscreen shape(s) were drawn as outlines only",
                    );
                }
                let owner = owner.map(str::to_string);
                self.silk_shape(layer, shape, width, owner);
            }
            Some(LayerKind::Copper) => self.copper_shape(layer, shape, width),
            _ => self.tally.add(
                Severity::Dropped,
                format!("drawing item(s) on layer {layer_name} (only silkscreen, copper and the outline are read)"),
            ),
        }
    }

    fn silk_shape(&mut self, layer: usize, shape: Shape, width: f64, owner: Option<String>) {
        match shape {
            Shape::Line(from, to) => self.graphics.push(Graphic::Line {
                layer,
                from,
                to,
                width,
                owner,
            }),
            Shape::Arc(a) => {
                let (start_deg, end_deg) = a.ccw_angles();
                self.graphics.push(Graphic::Arc {
                    layer,
                    center: a.center,
                    radius: a.radius,
                    start_deg,
                    end_deg,
                    width,
                    owner,
                });
            }
            Shape::Circle(center, radius) => self.graphics.push(Graphic::Arc {
                layer,
                center,
                radius,
                start_deg: 0.0,
                end_deg: 0.0,
                width,
                owner,
            }),
            Shape::Poly(c) => {
                for s in &c.segments {
                    match *s {
                        Segment::Line { from, to } => self.graphics.push(Graphic::Line {
                            layer,
                            from,
                            to,
                            width,
                            owner: owner.clone(),
                        }),
                        Segment::Arc {
                            center,
                            radius,
                            start_deg,
                            end_deg,
                            clockwise,
                        } => {
                            let (start_deg, end_deg) = if clockwise {
                                (end_deg, start_deg)
                            } else {
                                (start_deg, end_deg)
                            };
                            self.graphics.push(Graphic::Arc {
                                layer,
                                center,
                                radius,
                                start_deg,
                                end_deg,
                                width,
                                owner: owner.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    /// A line or arc drawn on a copper layer is a track with no net.
    fn copper_shape(&mut self, layer: usize, shape: Shape, width: f64) {
        match shape {
            Shape::Line(from, to) => self.copper.push(CopperItem::Track {
                layer,
                from,
                to,
                width,
                net: None,
            }),
            Shape::Arc(a) => {
                let (start_deg, end_deg) = a.ccw_angles();
                self.copper.push(CopperItem::TrackArc {
                    layer,
                    center: a.center,
                    radius: a.radius,
                    start_deg,
                    end_deg,
                    width,
                    net: None,
                });
            }
            Shape::Circle(center, radius) => self.copper.push(CopperItem::TrackArc {
                layer,
                center,
                radius,
                start_deg: 0.0,
                end_deg: 0.0,
                width,
                net: None,
            }),
            Shape::Poly(c) => {
                for s in &c.segments {
                    if let Segment::Line { from, to } = *s {
                        self.copper.push(CopperItem::Track {
                            layer,
                            from,
                            to,
                            width,
                            net: None,
                        });
                    } else {
                        self.tally.add(
                            Severity::Dropped,
                            "arc piece(s) of polygons drawn on copper (not read)",
                        );
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Texts
    // -----------------------------------------------------------------------------------------

    fn text(&mut self, content: &str, node: &Node, spot: TextSpot, frame: &Frame) {
        if node.has_flag("hide") || node.is_yes("hide") {
            self.tally
                .add(Severity::Info, "hidden text(s) were not drawn");
            return;
        }
        if content.trim().is_empty() {
            self.tally
                .add(Severity::Info, "empty text(s) were not drawn");
            return;
        }
        let Some(layer_name) = node.child("layer").and_then(|l| l.str_at(0)) else {
            return;
        };
        let Some(layer) = self.layer_index(layer_name) else {
            self.tally.add(
                Severity::Dropped,
                "text(s) on a layer the board does not list",
            );
            return;
        };
        if !matches!(
            self.layers.get(layer).map(|l| l.kind),
            Some(LayerKind::Silkscreen) | Some(LayerKind::Copper)
        ) {
            self.tally.add(
                Severity::Dropped,
                format!(
                    "text(s) on layer {layer_name} (only silkscreen and copper texts are read)"
                ),
            );
            return;
        }
        let effects = node.child("effects");
        let font = effects.and_then(|e| e.child("font"));
        let height = font
            .and_then(|f| f.child("size"))
            .and_then(|s| s.num(0))
            .filter(|h| *h > 0.0)
            .unwrap_or(1.0);
        let thickness = font
            .and_then(|f| f.child("thickness"))
            .and_then(|t| t.num(0))
            .filter(|t| *t > 0.0)
            // Our own estimate when the text sets none: KiCad's default pen of 0.15 mm on a
            // 1.27 mm text (a board's explicit value) is 0.118 of the height.
            .unwrap_or(height * 0.12);
        if let Some(face) = font.and_then(|f| f.child("face")).and_then(|f| f.str_at(0)) {
            if face != "KiCad Font" {
                self.tally.add(
                    Severity::Info,
                    "text(s) set in a named font were drawn with the stroke font",
                );
            }
        }
        let justify = effects.and_then(|e| e.child("justify"));
        let has = |w: &str| justify.is_some_and(|j| j.has_flag(w));
        let mirrored = has("mirror");
        // KiCad anchors a text by its center unless told otherwise; the model anchors it at the
        // left end of its baseline. Without glyph widths the shift is an estimate.
        let lines: Vec<&str> = content.split('\n').collect();
        let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f64;
        // Our own estimate, not KiCad's metrics: 0.6 of the height per letter.
        let width = longest * height * 0.6;
        let mut dx = if has("left") {
            0.0
        } else if has("right") {
            -width
        } else {
            -width / 2.0
        };
        let dy = if has("bottom") {
            0.0
        } else if has("top") {
            -height
        } else {
            -height / 2.0
        };
        if mirrored {
            dx = -dx;
        }
        if dx != 0.0 || dy != 0.0 {
            self.inexact_text = true;
        }
        let rotation_deg = spot.angle;
        let a = rotation_deg.to_radians();
        let anchor = frame.point(spot.x, spot.y);
        let at = Point {
            x: anchor.x + dx * a.cos() - dy * a.sin(),
            y: anchor.y + dx * a.sin() + dy * a.cos(),
        };
        self.texts.push(Text {
            layer,
            at,
            content: content.to_string(),
            height,
            rotation_deg,
            mirrored,
            font: TextFont::Stroke {
                width: thickness,
                font: board_model::StrokeFont::Default,
            },
            component: None,
        });
    }

    fn text_spot(node: &Node) -> Option<TextSpot> {
        let at = node.child("at")?;
        Some(TextSpot {
            x: at.num(0)?,
            y: at.num(1)?,
            angle: at.num(2).unwrap_or(0.0),
        })
    }

    // -----------------------------------------------------------------------------------------
    // Copper
    // -----------------------------------------------------------------------------------------

    fn segment(&mut self, n: &Node) {
        let (Some(a), Some(b), Some(layer_name)) = (
            n.child("start").and_then(Node::pair),
            n.child("end").and_then(Node::pair),
            n.child("layer").and_then(|l| l.str_at(0)),
        ) else {
            self.tally
                .add(Severity::Dropped, "track(s) with unreadable numbers");
            return;
        };
        let Some(layer) = self.layer_index(layer_name) else {
            self.tally.add(
                Severity::Dropped,
                "track(s) on a layer the board does not list",
            );
            return;
        };
        let Some(width) = Self::stroke_width(n) else {
            self.tally
                .add(Severity::Dropped, "track(s) with unreadable numbers");
            return;
        };
        let f = Frame::IDENTITY;
        self.copper.push(CopperItem::Track {
            layer,
            from: f.point(a.0, a.1),
            to: f.point(b.0, b.1),
            width,
            net: self.net_name(n),
        });
    }

    fn track_arc(&mut self, n: &Node) {
        let f = Frame::IDENTITY;
        let pts = (
            n.child("start").and_then(|c| place(&f, c)),
            n.child("mid").and_then(|c| place(&f, c)),
            n.child("end").and_then(|c| place(&f, c)),
            n.child("layer").and_then(|l| l.str_at(0)),
        );
        let (Some(s), Some(m), Some(e), Some(layer_name)) = pts else {
            self.tally
                .add(Severity::Dropped, "track arc(s) with unreadable numbers");
            return;
        };
        let Some(layer) = self.layer_index(layer_name) else {
            self.tally.add(
                Severity::Dropped,
                "track arc(s) on a layer the board does not list",
            );
            return;
        };
        let Some(width) = Self::stroke_width(n) else {
            self.tally
                .add(Severity::Dropped, "track arc(s) with unreadable numbers");
            return;
        };
        let net = self.net_name(n);
        match arc_through(s, m, e) {
            Some(a) => {
                let (start_deg, end_deg) = a.ccw_angles();
                self.copper.push(CopperItem::TrackArc {
                    layer,
                    center: a.center,
                    radius: a.radius,
                    start_deg,
                    end_deg,
                    width,
                    net,
                });
            }
            None => self.copper.push(CopperItem::Track {
                layer,
                from: s,
                to: e,
                width,
                net,
            }),
        }
    }

    fn via(&mut self, n: &Node) {
        let (Some(at), Some(size), Some(drill)) = (
            n.child("at").and_then(Node::pair),
            n.child("size").and_then(|s| s.num(0)),
            n.child("drill").and_then(|s| s.num(0)),
        ) else {
            self.tally
                .add(Severity::Dropped, "via(s) with unreadable numbers");
            return;
        };
        if n.has_flag("blind") || n.has_flag("micro") {
            self.tally.add(
                Severity::Warning,
                "blind or micro via(s) were read as through vias (the model has no via layers)",
            );
        }
        self.copper.push(CopperItem::Via {
            at: Frame::IDENTITY.point(at.0, at.1),
            diameter: size,
            drill,
        });
    }

    fn zone(&mut self, n: &Node) {
        if n.child("keepout").is_some() {
            self.tally
                .add(Severity::Dropped, "keep-out zone(s) (they hold no copper)");
            return;
        }
        let mut filled = 0usize;
        for fp in n.children("filled_polygon") {
            let Some(layer_name) = fp.child("layer").and_then(|l| l.str_at(0)) else {
                continue;
            };
            let Some(layer) = self.layer_index(layer_name) else {
                self.tally.add(
                    Severity::Dropped,
                    "zone fill(s) on a layer the board does not list",
                );
                continue;
            };
            let Some(split) = fp
                .child("pts")
                .and_then(|p| split_fill(&Frame::IDENTITY, p))
            else {
                self.tally.add(
                    Severity::Dropped,
                    "zone fill piece(s) with unreadable points",
                );
                continue;
            };
            if let Some(why) = split.kept_whole {
                self.tally.add(
                    Severity::Warning,
                    format!("zone fill(s) kept as the one contour KiCad wrote, because the fill {why} (their holes are not cut out)"),
                );
            }
            if split.orphan_holes > 0 {
                for _ in 0..split.orphan_holes {
                    self.tally.add(
                        Severity::Warning,
                        "hole(s) of a zone fill inside none of its copper pieces were left out",
                    );
                }
            }
            for piece in split.pieces {
                filled += 1;
                self.copper.push(CopperItem::Region {
                    layer,
                    contour: piece.outer,
                    holes: piece.holes,
                });
            }
        }
        if filled == 0 {
            self.tally.add(
                Severity::Dropped,
                "zone(s) with no filled copper in the file (fill them in KiCad and save)",
            );
        }
    }

    // -----------------------------------------------------------------------------------------
    // Footprints
    // -----------------------------------------------------------------------------------------

    fn footprint(&mut self, fp: &Node) {
        let at = fp.child("at");
        let (Some(x), Some(y)) = (at.and_then(|a| a.num(0)), at.and_then(|a| a.num(1))) else {
            self.tally.add(
                Severity::Dropped,
                "footprint(s) without a readable position",
            );
            return;
        };
        let angle = at.and_then(|a| a.num(2)).unwrap_or(0.0);
        let frame = Frame::new(x, y, angle);
        let bottom = fp.child("layer").and_then(|l| l.str_at(0)) == Some("B.Cu");
        let designator = fp
            .children("property")
            .find(|p| p.str_at(0) == Some("Reference"))
            .and_then(|p| p.str_at(1))
            .or_else(|| {
                fp.children("fp_text")
                    .find(|t| t.str_at(0) == Some("reference"))
                    .and_then(|t| t.str_at(1))
            })
            .unwrap_or("")
            .to_string();
        let value = fp
            .children("property")
            .find(|p| p.str_at(0) == Some("Value"))
            .and_then(|p| p.str_at(1))
            .unwrap_or("")
            .to_string();
        let owner = (!designator.is_empty()).then_some(designator.as_str());
        let lib_id = fp.str_at(0).unwrap_or("");
        let name = lib_id.rsplit(':').next().unwrap_or(lib_id).to_string();

        for item in fp.args() {
            let Some(head) = item.head() else { continue };
            match head {
                "pad" => self.pad(item, &frame, owner),
                "property" => {
                    if let (Some(spot), Some(text)) = (Self::text_spot(item), item.str_at(1)) {
                        if matches!(item.str_at(0), Some("Reference") | Some("Value")) {
                            self.text(&expand(text, &designator, &value), item, spot, &frame);
                        }
                    }
                }
                "fp_text" => {
                    if let (Some(spot), Some(text)) = (Self::text_spot(item), item.str_at(1)) {
                        self.text(&expand(text, &designator, &value), item, spot, &frame);
                    }
                }
                h if h.starts_with("fp_") => {
                    let kind = h.trim_start_matches("fp_");
                    if kind == "text_box" {
                        self.tally.add(Severity::Dropped, "text box(es) (not read)");
                    } else {
                        self.graphic_item(kind, item, &frame, owner);
                    }
                }
                "zone" => self
                    .tally
                    .add(Severity::Dropped, "zone(s) inside footprints (not read)"),
                _ => {}
            }
        }

        let mut models = fp.children("model");
        let model = models.next().and_then(|m| self.model(m));
        let extra = models.count();
        if extra > 0 {
            self.tally.add(
                Severity::Dropped,
                "extra 3D model reference(s) on a footprint (only the first is read)",
            );
        }
        if let Some(ModelRef {
            source: ModelSource::External { file, .. },
            ..
        }) = &model
        {
            self.tally.add(
                Severity::Info,
                format!("3D model file(s) named by the board, not stored in it: first {file}"),
            );
        }
        let pos = frame.point(0.0, 0.0);
        self.components.push(Placement {
            designator,
            footprint: name,
            at: pos,
            rotation_deg: angle,
            side: if bottom { Side::Bottom } else { Side::Top },
            model,
            comment: String::new(),
        });
    }

    fn model(&mut self, m: &Node) -> Option<ModelRef> {
        let file = m.args().first()?.text()?.to_string();
        let name = file.rsplit(['/', '\\']).next().unwrap_or(&file).to_string();
        let offset = m
            .child("offset")
            .or_else(|| m.child("at"))
            .and_then(Node::triple)
            .unwrap_or([0.0; 3]);
        // KiCad's rotation (x, y, z) turns the model the other way about each axis, X first, then
        // Y, then Z (measured on KiCad's own VRML export, see fixtures/kicad/model_axes): the
        // model's offset and the footprint's frame then agree with `Placement` as they are.
        let rotation_deg = m
            .child("rotate")
            .and_then(Node::triple)
            .unwrap_or([0.0; 3])
            .map(|d| if d == 0.0 { 0.0 } else { -d });
        if let Some(scale) = m.child("scale").and_then(Node::triple) {
            if scale.iter().any(|s| (s - 1.0).abs() > 1e-9) {
                self.tally.add(
                    Severity::Warning,
                    "3D model scale(s) other than 1 were not applied (the model has no scale)",
                );
            }
        }
        Some(ModelRef {
            name,
            source: ModelSource::External {
                file,
                checksum: None,
            },
            offset,
            rotation_deg,
            color: None,
        })
    }

    /// The contours of a custom pad, in the pad's frame: its anchor (a rectangle of the pad's
    /// size, or a circle of its width) and the filled primitives. Strokes become capsules; what
    /// cannot be drawn exactly is named in a note.
    fn custom_pad(&mut self, pad: &Node, sx: f64, sy: f64) -> Vec<Contour> {
        /// The most primitives read from one pad.
        const MAX_PRIMITIVES: usize = 256;
        let mut out: Vec<Contour> = Vec::new();
        let anchor = pad
            .child("options")
            .and_then(|o| o.child("anchor"))
            .and_then(|a| a.str_at(0))
            .unwrap_or("rect");
        if anchor == "circle" {
            out.push(padshape::circle(Point { x: 0.0, y: 0.0 }, sx / 2.0));
        } else {
            let (hx, hy) = (sx / 2.0, sy / 2.0);
            out.push(padshape::polygon(&[
                Point { x: -hx, y: -hy },
                Point { x: hx, y: -hy },
                Point { x: hx, y: hy },
                Point { x: -hx, y: hy },
            ]));
        }
        let local = |n: &Node| n.pair().map(|(x, y)| Point { x, y: -y });
        let prims = pad.child("primitives");
        let all = prims.map(Node::args).unwrap_or(&[]);
        if all.len() > MAX_PRIMITIVES {
            self.tally.add(
                Severity::Dropped,
                "custom pad primitive(s) past the 256 read for each pad",
            );
        }
        for prim in all.iter().take(MAX_PRIMITIVES) {
            let width = prim
                .child("width")
                .and_then(|w| w.num(0))
                .or_else(|| {
                    prim.child("stroke")
                        .and_then(|s| s.child("width"))
                        .and_then(|w| w.num(0))
                })
                .unwrap_or(0.0)
                .max(0.0);
            let filled = prim
                .child("fill")
                .and_then(|f| f.str_at(0))
                .is_some_and(|f| f == "yes" || f == "solid");
            let corners: Option<Vec<Point>> = match prim.head() {
                Some("gr_poly") => prim.child("pts").map(|p| {
                    let mut points = Vec::new();
                    for n in p.args() {
                        match n.head() {
                            Some("xy") => points.extend(local(n)),
                            Some("arc") => {
                                // Drawn as the straight line between its ends.
                                points.extend(n.child("start").and_then(local));
                                points.extend(n.child("end").and_then(local));
                                self.tally.add(
                                    Severity::Warning,
                                    "custom pad polygon arc(s) were drawn as straight edges",
                                );
                            }
                            _ => {}
                        }
                    }
                    points
                }),
                Some("gr_rect") => prim
                    .child("start")
                    .and_then(local)
                    .zip(prim.child("end").and_then(local))
                    .map(|(a, b)| vec![a, Point { x: b.x, y: a.y }, b, Point { x: a.x, y: b.y }]),
                _ => None,
            };
            let mut drawn = false;
            if let Some(corners) = corners.filter(|c| c.len() >= 3) {
                drawn = true;
                if filled {
                    out.push(padshape::polygon(&corners));
                    if width > 0.0 {
                        self.tally.add(
                            Severity::Warning,
                            "custom pad primitive(s) with a stroke width were drawn without the half-width the stroke adds",
                        );
                    }
                } else if width > 0.0 {
                    for (i, a) in corners.iter().enumerate() {
                        if let Some(b) = corners.get(i + 1).or(corners.first()) {
                            out.push(padshape::capsule(*a, *b, width));
                        }
                    }
                } else {
                    drawn = false;
                }
            } else if prim.head() == Some("gr_circle") {
                let c = prim.child("center").and_then(local);
                let e = prim.child("end").and_then(local);
                if let (Some(c), Some(e)) = (c, e) {
                    let r = (e.x - c.x).hypot(e.y - c.y);
                    if filled && r > 0.0 {
                        out.push(padshape::circle(c, r));
                        drawn = true;
                    }
                }
            } else if prim.head() == Some("gr_line") {
                let a = prim.child("start").and_then(local);
                let b = prim.child("end").and_then(local);
                if let (Some(a), Some(b)) = (a, b) {
                    if width > 0.0 {
                        out.push(padshape::capsule(a, b, width));
                        drawn = true;
                    }
                }
            }
            if !drawn {
                self.tally.add(
                    Severity::Dropped,
                    "custom pad primitive(s) that are arcs, curves, outlines without width, or unreadable (left out of the pad)",
                );
            }
        }
        out
    }

    fn pad(&mut self, pad: &Node, f: &Frame, owner: Option<&str>) {
        let kind = pad.str_at(1).unwrap_or("");
        let shape_name = pad.str_at(2).unwrap_or("");
        let at = pad.child("at");
        let size = pad.child("size").and_then(Node::pair);
        let (Some(ax), Some(ay), Some((sx, sy))) =
            (at.and_then(|a| a.num(0)), at.and_then(|a| a.num(1)), size)
        else {
            self.tally
                .add(Severity::Dropped, "pad(s) with unreadable numbers");
            return;
        };
        if !(sx.is_finite() && sy.is_finite() && sx > 0.0 && sy > 0.0) {
            self.tally.add(
                Severity::Dropped,
                "pad(s) whose size is not a positive number",
            );
            return;
        }
        let rotation_deg = at.and_then(|a| a.num(2)).unwrap_or(0.0);
        let center = f.point(ax, ay);
        // The pad's own outline when it is not a plain shape: contours in its frame (Y up).
        let mut polygon: Option<Vec<Contour>> = None;
        let min_side = sx.min(sy);
        let shape = match shape_name {
            "circle" => PadShape::Round,
            "oval" => PadShape::Oval,
            "rect" => PadShape::Rectangle,
            "roundrect" => {
                let ratio = pad
                    .child("roundrect_rratio")
                    .and_then(|r| r.num(0))
                    .unwrap_or(0.0)
                    .clamp(0.0, 0.5);
                let corners = padshape::Corners::from_names(
                    pad.child("chamfer")
                        .map(Node::args)
                        .unwrap_or(&[])
                        .iter()
                        .filter_map(Node::text),
                );
                let chamfer = pad
                    .child("chamfer_ratio")
                    .and_then(|r| r.num(0))
                    .unwrap_or(0.0)
                    .clamp(0.0, 0.5);
                if corners.any() && chamfer > 0.0 {
                    polygon = Some(vec![padshape::rounded_chamfered_rect(
                        sx,
                        sy,
                        ratio * min_side,
                        chamfer * min_side,
                        corners,
                    )]);
                    PadShape::Rectangle
                } else if ratio > 0.0 {
                    PadShape::RoundedRectangle {
                        radius_ratio: ratio,
                    }
                } else {
                    PadShape::Rectangle
                }
            }
            "trapezoid" => {
                let (dx, dy) = pad
                    .child("rect_delta")
                    .and_then(Node::pair)
                    .unwrap_or((0.0, 0.0));
                match padshape::trapezoid(sx, sy, dx, dy) {
                    Some(c) => polygon = Some(vec![c]),
                    None => self.tally.add(
                        Severity::Warning,
                        "trapezoid pad(s) whose deltas leave no shape were read as rectangles",
                    ),
                }
                PadShape::Rectangle
            }
            "custom" => {
                let shapes = self.custom_pad(pad, sx, sy);
                if shapes.is_empty() {
                    self.tally.add(
                        Severity::Warning,
                        "custom pad(s) with no readable shape were read as rectangles of their anchor size",
                    );
                } else {
                    polygon = Some(shapes);
                }
                PadShape::Rectangle
            }
            _ => {
                self.tally.add(
                    Severity::Warning,
                    "pad(s) of an unknown shape were read as rectangles",
                );
                PadShape::Rectangle
            }
        };
        let drill_node = pad.child("drill");
        let drill = drill_node.and_then(|d| match d.args().first() {
            Some(Node::Atom(w)) if w == "oval" => {
                self.tally.add(
                    Severity::Info,
                    "slotted hole(s) were read as round holes as wide as the slot's narrow side",
                );
                Some(d.num(1)?.min(d.num(2)?))
            }
            _ => d.num(0),
        });
        if drill_node.is_some_and(|d| d.child("offset").is_some()) {
            self.tally.add(
                Severity::Warning,
                "pad hole(s) set off-center in their pad were drawn centered (the offset is not read)",
            );
        }
        let drill = drill.filter(|d| *d > 0.0);
        let plated = kind == "thru_hole" && drill.is_some();
        // The copper layers the pad has copper on.
        let mut targets: Vec<usize> = Vec::new();
        let mut wildcard = false;
        for l in pad
            .child("layers")
            .map(Node::args)
            .unwrap_or(&[])
            .iter()
            .filter_map(Node::text)
        {
            match l {
                "*.Cu" => wildcard = true,
                "F&B.Cu" => {
                    for n in ["F.Cu", "B.Cu"] {
                        if let Some(i) = self.layer_index(n) {
                            targets.push(i);
                        }
                    }
                }
                other if other.ends_with(".Cu") => {
                    if let Some(i) = self.layer_index(other) {
                        targets.push(i);
                    }
                }
                _ => {}
            }
        }
        if wildcard {
            targets = vec![self.all_copper()];
        } else if targets.is_empty() && drill.is_some() {
            // A bare hole (no copper on any layer) still has to be cut through the board.
            targets = vec![self.all_copper()];
        }
        targets.sort_unstable();
        targets.dedup();
        let hole_only_note = targets.is_empty();
        for layer in targets {
            if let Some(shapes) = &polygon {
                self.copper.push(CopperItem::PolygonPad {
                    layer,
                    at: center,
                    shapes: shapes
                        .iter()
                        .map(|c| padshape::placed(c, center, rotation_deg))
                        .collect(),
                    drill,
                    plated,
                    owner: owner.map(str::to_string),
                });
                continue;
            }
            self.copper.push(CopperItem::Pad {
                layer,
                at: center,
                shape,
                size_x: sx,
                size_y: sy,
                rotation_deg,
                drill,
                plated,
                owner: owner.map(str::to_string),
            });
        }
        if hole_only_note {
            self.tally
                .add(Severity::Dropped, "pad(s) with no copper layer and no hole");
        }
    }
}

/// Fills in the two text variables KiCad uses on silkscreen labels.
fn expand(text: &str, reference: &str, value: &str) -> String {
    text.replace("${REFERENCE}", reference)
        .replace("${VALUE}", value)
}

/// Reads the layer table: canonical names in stack order, with kind and side.
fn read_layers(root: &Node) -> Vec<Layer> {
    let mut found: Vec<(usize, usize, Layer)> = Vec::new();
    if let Some(layers) = root.child("layers") {
        for (order, entry) in layers.args().iter().enumerate() {
            let Node::List(items) = entry else { continue };
            let Some(name) = items.get(1).and_then(Node::text) else {
                continue;
            };
            let (kind, side) = classify(name);
            found.push((
                stack_rank(name, order),
                order,
                Layer {
                    name: name.to_string(),
                    kind,
                    side,
                    position: None,
                },
            ));
        }
    }
    found.sort_by_key(|(rank, order, _)| (*rank, *order));
    found.into_iter().map(|(_, _, l)| l).collect()
}

/// The board thickness: the stack-up's copper, dielectric and solder mask layers added up, or the
/// `general` thickness when the file has no stack-up. A difference is reported in `tally`.
fn read_thickness(root: &Node, tally: &mut Tally) -> Option<f64> {
    let general = root
        .child("general")
        .and_then(|g| g.child("thickness"))
        .and_then(|t| t.num(0))
        .filter(|t| *t > 0.0);
    let stackup = root.child("setup").and_then(|s| s.child("stackup"));
    let mut sum = 0.0;
    let mut any = false;
    for layer in stackup.map(|s| s.children("layer")).into_iter().flatten() {
        let name = layer.str_at(0).unwrap_or("");
        let counts =
            name.ends_with(".Cu") || name.ends_with(".Mask") || name.starts_with("dielectric");
        if !counts {
            continue;
        }
        // A dielectric may be split into sub-layers, each with its own thickness.
        let own = layer.child("thickness").and_then(|t| t.num(0));
        let subs = layer
            .children("addsublayer")
            .filter_map(|a| a.child("thickness").and_then(|t| t.num(0)));
        let total = own.into_iter().chain(subs).sum::<f64>();
        if total > 0.0 {
            sum += total;
            any = true;
        }
    }
    match (any.then_some(sum), general) {
        (Some(s), Some(g)) => {
            if (s - g).abs() > 0.001 {
                tally.add(
                    Severity::Warning,
                    format!("board(s) whose stack-up adds up to {s:.3} mm but whose general thickness says {g:.3} mm; the stack-up was used"),
                );
            }
            Some(s)
        }
        (Some(s), None) => Some(s),
        (None, g) => g,
    }
}

/// The copper thickness assumed for a layer the stack-up does not give, in mm (1 oz).
const DEFAULT_COPPER_THICKNESS: f64 = 0.035;

/// Gives each copper layer of the stack its [`StackPosition`]: from the stack-up, adding up the
/// thicknesses above it as [`read_thickness`] does, or, when the file has no stack-up, spread
/// evenly from the top face to the bottom one (with a note when there are inner layers).
fn place_copper(root: &Node, layers: &mut [Layer], thickness: Option<f64>, tally: &mut Tally) {
    let stackup = root.child("setup").and_then(|s| s.child("stackup"));
    let mut depth = 0.0;
    let mut placed: HashMap<String, StackPosition> = HashMap::new();
    for layer in stackup.map(|s| s.children("layer")).into_iter().flatten() {
        let name = layer.str_at(0).unwrap_or("");
        let copper = name.ends_with(".Cu");
        if !(copper || name.ends_with(".Mask") || name.starts_with("dielectric")) {
            continue;
        }
        let own = layer.child("thickness").and_then(|t| t.num(0));
        let subs = layer
            .children("addsublayer")
            .filter_map(|a| a.child("thickness").and_then(|t| t.num(0)));
        let total = own.into_iter().chain(subs).sum::<f64>().max(0.0);
        if copper {
            let thickness = if total > 0.0 {
                total
            } else {
                DEFAULT_COPPER_THICKNESS
            };
            placed.insert(name.to_string(), StackPosition { depth, thickness });
        }
        depth += total;
    }
    let copper: Vec<usize> = layers
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == LayerKind::Copper && l.name != ALL_COPPER)
        .map(|(i, _)| i)
        .collect();
    if !placed.is_empty() {
        for &i in &copper {
            if let Some(l) = layers.get_mut(i) {
                l.position = placed.get(&l.name).copied();
            }
        }
        return;
    }
    let (Some(total), true) = (thickness, copper.len() >= 2) else {
        return;
    };
    let span = (total - DEFAULT_COPPER_THICKNESS).max(0.0);
    let last = (copper.len() - 1) as f64;
    for (n, &i) in copper.iter().enumerate() {
        if let Some(l) = layers.get_mut(i) {
            l.position = Some(StackPosition {
                depth: span * n as f64 / last,
                thickness: DEFAULT_COPPER_THICKNESS,
            });
        }
    }
    if copper.len() > 2 {
        tally.add(
            Severity::Info,
            "board(s) with no stack-up: the inner copper layers are placed evenly through the \
             thickness",
        );
    }
}

/// Reads a parsed `.kicad_pcb`.
pub fn read(mut root: Node) -> ReadOutcome {
    let absurd = crate::sexpr::limit_numbers(&mut root);
    let root = &root;
    let mut tally = Tally::default();
    if absurd > 0 {
        tally.counts.insert(
            (
                0,
                "number(s) beyond 100 m for a position or 10 m for a size were treated as unreadable, and the items holding them dropped".into(),
            ),
            absurd,
        );
    }
    let version = root
        .child("version")
        .and_then(|v| v.num(0))
        .map(|v| v as u64);
    let generator_major = root
        .child("generator_version")
        .and_then(|v| v.str_at(0))
        .and_then(|v| v.split('.').next())
        .and_then(|v| v.parse::<u32>().ok());
    let name = root
        .child("title_block")
        .and_then(|t| t.child("title"))
        .and_then(|t| t.str_at(0))
        .unwrap_or("")
        .to_string();
    let too_old = version.is_some_and(|v| v < FIRST_SUPPORTED_VERSION)
        || generator_major.is_some_and(|m| m < 9);
    if too_old || version.is_none() {
        let what = match version {
            Some(v) => format!(
                "this board file is version {v}, written by a KiCad older than 9; we read KiCad 9 and 10 files and do not guess at older ones. Open it in KiCad 9 or later and save it again."
            ),
            None => "this board file has no version, so we cannot tell which KiCad wrote it; we read KiCad 9 and 10 files. Open it in KiCad 9 or later and save it again.".to_string(),
        };
        return ReadOutcome {
            board: Board {
                name,
                ..Board::default()
            },
            notes: vec![Note {
                severity: Severity::Dropped,
                message: what,
            }],
            ..ReadOutcome::default()
        };
    }

    let mut layers = read_layers(root);
    let thickness = read_thickness(root, &mut tally);
    place_copper(root, &mut layers, thickness, &mut tally);
    let by_name = layers
        .iter()
        .enumerate()
        .map(|(i, l)| (l.name.clone(), i))
        .collect();
    let mut nets = HashMap::new();
    for n in root.children("net") {
        if let (Some(id), Some(name)) = (n.num(0), n.str_at(1)) {
            nets.insert(id as i64, name.to_string());
        }
    }
    // `pad_to_mask_clearance`: how far the solder mask opens past each pad's edge.
    let mask_expansion = root
        .child("setup")
        .and_then(|s| s.child("pad_to_mask_clearance"))
        .and_then(|c| c.num(0))
        .filter(|c| *c >= 0.0);
    let mut r = Reader {
        layers,
        by_name,
        nets,
        tally,
        copper: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        components: Vec::new(),
        edge_open: Vec::new(),
        edge_closed: Vec::new(),
        inexact_text: false,
    };

    for item in root.args() {
        let Some(head) = item.head() else { continue };
        match head {
            "version" | "generator" | "generator_version" | "general" | "paper" | "title_block"
            | "layers" | "setup" | "net" | "embedded_fonts" | "uuid" | "property" => {}
            "footprint" | "module" => r.footprint(item),
            "segment" => r.segment(item),
            "arc" => r.track_arc(item),
            "via" => r.via(item),
            "zone" => r.zone(item),
            "gr_text" => {
                if let (Some(spot), Some(text)) = (Reader::text_spot(item), item.str_at(0)) {
                    r.text(text, item, spot, &Frame::IDENTITY);
                }
            }
            h if h.starts_with("gr_") => {
                let kind = h.trim_start_matches("gr_");
                r.graphic_item(kind, item, &Frame::IDENTITY, None);
            }
            other => r.tally.add(
                Severity::Dropped,
                format!("board item(s) of kind \"{other}\" (not read)"),
            ),
        }
    }

    let joined = outline::join(
        std::mem::take(&mut r.edge_open),
        std::mem::take(&mut r.edge_closed),
    );
    if joined.outline.outer.segments.is_empty() {
        r.tally.add(
            Severity::Warning,
            "board(s) with no closed outline on Edge.Cuts",
        );
    }
    if joined.open_chains > 0 {
        r.tally.counts.insert(
            (
                0,
                "chain(s) of Edge.Cuts lines that do not close were left out of the outline".into(),
            ),
            joined.open_chains,
        );
    }
    if joined.outside > 0 {
        r.tally.counts.insert(
            (
                0,
                "closed Edge.Cuts shape(s) outside the outline were left out".into(),
            ),
            joined.outside,
        );
    }
    if joined.over_limit > 0 {
        r.tally.counts.insert(
            (
                0,
                "Edge.Cuts piece(s) beyond the limit were not joined".into(),
            ),
            joined.over_limit,
        );
    }
    if r.inexact_text {
        r.tally.add(
            Severity::Info,
            "board(s) whose centered or right-aligned texts sit where we estimate them (KiCad aligns a text by its middle and we have no glyph widths)",
        );
    }
    r.tally.add(
        Severity::Info,
        "board(s) whose stack-up colors, materials, copper weights and finish were not read",
    );
    let mut notes = r.tally.into_notes();
    notes.sort_by_key(|n| match n.severity {
        Severity::Dropped => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    });
    ReadOutcome {
        board: Board {
            name,
            thickness,
            layers: r.layers,
            outline: joined.outline,
            copper: r.copper,
            graphics: r.graphics,
            texts: r.texts,
            components: r.components,
            appearance: Default::default(),
            solder_mask_expansion: mask_expansion,
        },
        notes,
        models: Vec::new(),
        fonts: Vec::new(),
    }
}
