//! Fills the board model from a parsed `.kicad_pcb`.
//!
//! Everything here follows KiCad's published file-format documentation (see README.md for the
//! pages) and our own test boards. Lengths in the file are millimeters with Y pointing down; the
//! model has Y pointing up, so every Y changes sign here and nowhere else.

use std::collections::{BTreeMap, HashMap};

use board_model::{
    Board, Contour, CopperItem, EmbeddedFont, Graphic, HorizontalAnchor, Layer, LayerKind,
    ModelRef, ModelSource, Note, PadShape, Placement, Point, ReadOutcome, Segment, Severity, Side,
    StackPosition, Text, TextAnchor, TextFont, Variant, VariantValue, VerticalAnchor,
    KICAD_LINE_PITCH,
};

use crate::geom::{arc_through, circle_segment, place, pts_contour, split_fill, Frame};
use crate::outline;
use crate::padshape;
use crate::sexpr::Node;
use crate::text_vars::{Fields, Resolver, Scope, Variables};

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
    /// Which sides' vias the board covers with solder mask unless a via says otherwise
    /// (`(setup (tenting ...))`): top, bottom. `None` when the setup does not say.
    default_tenting: Option<[bool; 2]>,
    /// The fonts the board embeds, and the ones texts use (handed over, each once).
    embedded: Vec<crate::fonts::Embedded>,
    fonts_used: Vec<EmbeddedFont>,
    fonts_unused: Vec<bool>,
    /// Each footprint's fields as stored, for the text variables.
    fields: Vec<Fields>,
    /// For each text in `texts`: its stored string, the footprint it belongs to (an index into
    /// `fields`) and the name KiCad shows for its layer.
    sources: Vec<(String, Option<usize>, String)>,
    /// Canonical layer name -> the name KiCad shows for it (the layer table's user name).
    shown_layer_names: HashMap<String, String>,
    /// Each footprint's `(variant ...)` entries, in the file's order.
    variant_entries: Vec<FootprintVariant>,
}

/// What one footprint's `(variant ...)` entry changes in one board variant (`k10-dnp`): KiCad
/// writes only what differs from the footprint's own flags and fields.
struct FootprintVariant {
    /// The variant's name, as the board's `(variants ...)` list gives it.
    variant: String,
    designator: String,
    /// Whether the footprint itself is marked do-not-populate.
    board_dnp: bool,
    /// `(dnp yes)` or `(dnp no)` in the variant, when given.
    dnp: Option<bool>,
    /// The `Value` field in the variant, when given.
    value: Option<String>,
}

/// The sides a `(tenting ...)` list covers: top, bottom. KiCad 9 writes the sides it covers as
/// bare words (`(tenting front back)`, or `(tenting none)`); KiCad 10 writes
/// `(tenting (front yes) (back no))`.
fn tenting_sides(n: &Node) -> [bool; 2] {
    [n.is_yes("front"), n.is_yes("back")]
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
            Some(LayerKind::Copper) => self.copper_shape(layer, shape, width, filled),
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
    /// A shape drawn on copper. A filled circle or polygon (rectangles are polygons) is a copper
    /// region, with its stroke drawn round its edge when it has one (KiCad's copper is the fill
    /// and the stroke together); any other shape is its stroke alone.
    fn copper_shape(&mut self, layer: usize, shape: Shape, width: f64, filled: bool) {
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
            Shape::Circle(center, radius) => {
                if filled {
                    self.copper.push(CopperItem::Region {
                        layer,
                        contour: Contour {
                            segments: vec![circle_segment(center, radius)],
                        },
                        holes: Vec::new(),
                    });
                }
                if !filled || width > 0.0 {
                    self.copper.push(CopperItem::TrackArc {
                        layer,
                        center,
                        radius,
                        start_deg: 0.0,
                        end_deg: 0.0,
                        width,
                        net: None,
                    });
                }
            }
            Shape::Poly(c) => {
                if !filled || width > 0.0 {
                    self.copper_edge(layer, &c, width);
                }
                if filled {
                    self.copper.push(CopperItem::Region {
                        layer,
                        contour: c,
                        holes: Vec::new(),
                    });
                }
            }
        }
    }

    /// The edge of a polygon drawn on copper as tracks and track arcs of `width`.
    fn copper_edge(&mut self, layer: usize, c: &Contour, width: f64) {
        for s in &c.segments {
            match *s {
                Segment::Line { from, to } => self.copper.push(CopperItem::Track {
                    layer,
                    from,
                    to,
                    width,
                    net: None,
                }),
                Segment::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                    clockwise,
                } => {
                    // A track arc runs counter-clockwise from its start to its end.
                    let (start_deg, end_deg) = if clockwise {
                        (end_deg, start_deg)
                    } else {
                        (start_deg, end_deg)
                    };
                    self.copper.push(CopperItem::TrackArc {
                        layer,
                        center,
                        radius,
                        start_deg,
                        end_deg,
                        width,
                        net: None,
                    });
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Texts
    // -----------------------------------------------------------------------------------------

    /// Adds a text as stored; its text variables are filled in once the whole board is read.
    /// `owner` is the footprint it belongs to (an index into `fields`).
    fn text(
        &mut self,
        content: &str,
        node: &Node,
        spot: TextSpot,
        frame: &Frame,
        owner: Option<usize>,
    ) {
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
            // KiCad draws a pen at most a quarter of the text's height wide (its Gerber export
            // of a 1 mm text asked to have a 0.3 mm pen draws it with 0.25).
            .map(|t| t.min(height / 4.0))
            // Our own estimate when the text sets none: KiCad's default pen of 0.15 mm on a
            // 1.27 mm text (a board's explicit value) is 0.118 of the height.
            .unwrap_or(height * 0.12);
        let mut text_font = TextFont::Stroke {
            width: thickness,
            font: board_model::StrokeFont::KiCad,
        };
        if let Some(face) = font.and_then(|f| f.child("face")).and_then(|f| f.str_at(0)) {
            if face != "KiCad Font" {
                let (bold, italic) =
                    font.map_or((false, false), |f| (f.is_yes("bold"), f.is_yes("italic")));
                match self.embedded_font(face, bold, italic) {
                    Some(tf) => text_font = tf,
                    None => self.tally.add(
                        Severity::Warning,
                        format!(
                            "text(s) set in the font \"{face}\", which the board does not embed, were drawn with the stroke font"
                        ),
                    ),
                }
            }
        }
        let justify = effects.and_then(|e| e.child("justify"));
        let has = |w: &str| justify.is_some_and(|j| j.has_flag(w));
        let mirrored = has("mirror");
        // KiCad anchors a text by the middle of its box unless told otherwise, each line
        // justified on its own; the model carries the justification and the overlay resolves it
        // from the letters it draws. For a mirrored text it is in the text's own frame (the end
        // its letters start from), as the Gerber export shows.
        let anchor = TextAnchor {
            horizontal: if has("left") {
                HorizontalAnchor::Left
            } else if has("right") {
                HorizontalAnchor::Right
            } else {
                HorizontalAnchor::Center
            },
            vertical: if has("top") {
                VerticalAnchor::Top
            } else if has("bottom") {
                VerticalAnchor::Bottom
            } else {
                VerticalAnchor::Center
            },
        };
        let line_pitch = matches!(text_font, TextFont::Stroke { .. }).then_some(KICAD_LINE_PITCH);
        let rotation_deg = spot.angle;
        let at = frame.point(spot.x, spot.y);
        self.texts.push(Text {
            layer,
            at,
            content: content.to_string(),
            height,
            rotation_deg,
            mirrored,
            anchor,
            line_pitch,
            font: text_font,
            component: None,
        });
        let shown = self
            .shown_layer_names
            .get(layer_name)
            .cloned()
            .unwrap_or_else(|| layer_name.to_string());
        self.sources.push((content.to_string(), owner, shown));
    }

    /// The embedded font a text's `face` names, in the style the text asks for or else the
    /// closest the board has, handed over once.
    fn embedded_font(&mut self, face: &str, bold: bool, italic: bool) -> Option<TextFont> {
        let exact = self
            .embedded
            .iter()
            .position(|e| e.answers_to(face) && e.font.bold == bold && e.font.italic == italic);
        let i = exact.or_else(|| self.embedded.iter().position(|e| e.answers_to(face)))?;
        let e = self.embedded.get(i)?;
        if exact.is_none() {
            let face_name = if e.font.style.is_empty() {
                "embedded".to_string()
            } else {
                format!("embedded {}", e.font.style)
            };
            self.tally.add(
                Severity::Warning,
                format!(
                    "text(s) set in the font \"{face}\" in a style the board does not embed were drawn in the {face_name} face"
                ),
            );
        }
        // The font is named by its own family, not by the spelling the text used, so a font that
        // texts call by several names is handed over once.
        let tf = TextFont::TrueType {
            name: e.font.family.clone(),
            bold: e.font.bold,
            italic: e.font.italic,
        };
        if self.fonts_unused.get(i).copied().unwrap_or(false) {
            self.fonts_used.push(e.font.clone());
            if let Some(flag) = self.fonts_unused.get_mut(i) {
                *flag = false;
            }
        }
        Some(tf)
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
        // A blind, buried or micro via joins only the layers it names; any other via runs
        // through the whole board whatever it lists.
        let mut layers = None;
        if n.has_flag("blind") || n.has_flag("micro") {
            let ends = n.child("layers").map(Node::args).unwrap_or(&[]);
            let found: Vec<usize> = ends
                .iter()
                .filter_map(|e| e.text().and_then(|name| self.layer_index(name)))
                .filter(|&i| {
                    self.layers
                        .get(i)
                        .is_some_and(|l| l.kind == LayerKind::Copper && l.name != ALL_COPPER)
                })
                .collect();
            match (ends.len(), found.as_slice()) {
                (2, [a, b]) if a != b => layers = Some(((*a).min(*b), (*a).max(*b))),
                _ => self.tally.add(
                    Severity::Warning,
                    "blind or micro via(s) whose layers are not two different copper layers of \
                     the board were read as through vias",
                ),
            }
        }
        // The via's own `(tenting ...)`, else the board's default; `None` when neither says.
        let tented = n
            .child("tenting")
            .map(tenting_sides)
            .or(self.default_tenting)
            .map_or([None, None], |[front, back]| [Some(front), Some(back)]);
        self.copper.push(CopperItem::Via {
            at: Frame::IDENTITY.point(at.0, at.1),
            diameter: size,
            drill,
            tented,
            mask_expansion: [None, None],
            layers,
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
            if self.layers.get(layer).map(|l| l.kind) != Some(LayerKind::Copper) {
                self.tally.add(
                    Severity::Dropped,
                    format!("zone fill(s) on layer {layer_name} (only copper zones are read)"),
                );
                continue;
            }
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
        let owner = (!designator.is_empty()).then_some(designator.as_str());
        let mut fields: Vec<(String, String)> = fp
            .children("property")
            .filter_map(|p| Some((p.str_at(0)?.to_string(), p.str_at(1)?.to_string())))
            .collect();
        if !fields.iter().any(|(n, _)| n == "Reference") {
            fields.push(("Reference".into(), designator.clone()));
        }
        if !fields.iter().any(|(n, _)| n == "Value") {
            let value = fp
                .children("fp_text")
                .find(|t| t.str_at(0) == Some("value"))
                .and_then(|t| t.str_at(1));
            if let Some(value) = value {
                fields.push(("Value".into(), value.to_string()));
            }
        }
        // The part's comment is its Value field, as stored (`k10-dnp`: "10k").
        let comment = fields
            .iter()
            .find(|(n, _)| n == "Value")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        let fields_at = self.fields.len();
        self.fields.push(Fields {
            reference: designator.clone(),
            fields,
        });
        // `solder_mask_margin`: how far the mask opens past a pad's edge. A pad's own value wins
        // over its footprint's; both sides of the pad get it.
        let footprint_margin = fp
            .child("solder_mask_margin")
            .and_then(|m| m.num(0))
            .filter(|m| m.is_finite());
        let lib_id = fp.str_at(0).unwrap_or("");
        let name = lib_id.rsplit(':').next().unwrap_or(lib_id).to_string();

        for item in fp.args() {
            let Some(head) = item.head() else { continue };
            match head {
                "pad" => self.pad(item, &frame, owner, footprint_margin),
                "property" => {
                    // Every field that is shown is a text, through the same path as any other;
                    // a hidden field is left out without a note (most footprints hide their
                    // Datasheet and Description).
                    let hidden = item.has_flag("hide") || item.is_yes("hide");
                    if let (Some(spot), Some(text), false) =
                        (Self::text_spot(item), item.str_at(1), hidden)
                    {
                        self.text(text, item, spot, &frame, Some(fields_at));
                    }
                }
                "fp_text" => {
                    if let (Some(spot), Some(text)) = (Self::text_spot(item), item.str_at(1)) {
                        self.text(text, item, spot, &frame, Some(fields_at));
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
                // KiCad writes a footprint's zones, outline and fill, in board coordinates, as it
                // does the board's own (k10-footprint-zone).
                "zone" => self.zone(item),
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
        // KiCad's do-not-populate flag: `dnp` among the footprint's `attr` keywords
        // (`(attr smd dnp)`, `k10-dnp`'s R2). `exclude_from_pos_files`, `exclude_from_bom` and
        // `board_only` (R3 to R5) leave the part fitted.
        let fitted = !fp
            .child("attr")
            .is_some_and(|a| a.args().iter().any(|k| k.text() == Some("dnp")));
        self.footprint_variants(fp, &designator, !fitted);
        let pos = frame.point(0.0, 0.0);
        self.components.push(Placement {
            designator,
            footprint: name,
            at: pos,
            rotation_deg: angle,
            side: if bottom { Side::Bottom } else { Side::Top },
            models: model.into_iter().collect(),
            comment,
            fitted,
        });
    }

    /// Keeps a footprint's `(variant (name "LITE") (dnp yes) (field (name "Value") (value "1k")))`
    /// entries (`k10-dnp`). Other settings in an entry, and fields other than `Value`, get a note.
    fn footprint_variants(&mut self, fp: &Node, designator: &str, board_dnp: bool) {
        for entry in fp.children("variant") {
            let Some(variant) = entry.child("name").and_then(|n| n.str_at(0)) else {
                self.tally.add(
                    Severity::Warning,
                    "footprint variant entr(ies) without a name (not read)",
                );
                continue;
            };
            let mut found = FootprintVariant {
                variant: variant.to_string(),
                designator: designator.to_string(),
                board_dnp,
                dnp: None,
                value: None,
            };
            for item in entry.args() {
                match item.head() {
                    Some("name") => {}
                    Some("dnp") => match item.str_at(0) {
                        Some("yes") => found.dnp = Some(true),
                        Some("no") => found.dnp = Some(false),
                        _ => self.tally.add(
                            Severity::Warning,
                            "footprint variant do-not-populate setting(s) that are neither yes \
                             nor no (not read)",
                        ),
                    },
                    Some("field") => {
                        let name = item.child("name").and_then(|n| n.str_at(0));
                        let value = item.child("value").and_then(|n| n.str_at(0));
                        match (name, value) {
                            (Some("Value"), Some(value)) => found.value = Some(value.to_string()),
                            (name, _) => self.tally.add(
                                Severity::Info,
                                format!(
                                    "footprint variant field(s) {} (not read; only the Value is)",
                                    name.unwrap_or("without a name")
                                ),
                            ),
                        }
                    }
                    Some(other) => self.tally.add(
                        Severity::Info,
                        format!("footprint variant setting(s) {other} (not read)"),
                    ),
                    None => {}
                }
            }
            self.variant_entries.push(found);
        }
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
            let shape: Option<Contour> = match prim.head() {
                Some("gr_poly") => prim.child("pts").and_then(padshape::pts_contour),
                Some("gr_rect") => prim
                    .child("start")
                    .and_then(local)
                    .zip(prim.child("end").and_then(local))
                    .map(|(a, b)| {
                        padshape::polygon(&[
                            a,
                            Point { x: b.x, y: a.y },
                            b,
                            Point { x: a.x, y: b.y },
                        ])
                    }),
                Some("gr_circle") => {
                    let c = prim.child("center").and_then(local);
                    let e = prim.child("end").and_then(local);
                    c.zip(e)
                        .map(|(c, e)| (c, (e.x - c.x).hypot(e.y - c.y)))
                        .filter(|(_, r)| *r > 0.0 && r.is_finite())
                        .map(|(c, r)| padshape::circle(c, r))
                }
                Some("gr_arc") => {
                    let pts = (
                        prim.child("start").and_then(local),
                        prim.child("mid").and_then(local),
                        prim.child("end").and_then(local),
                    );
                    match pts {
                        (Some(s), Some(m), Some(e)) => Some(Contour {
                            // Points on one line are a straight stroke.
                            segments: vec![crate::geom::arc_through(s, m, e)
                                .map_or(Segment::Line { from: s, to: e }, |a| a.segment())],
                        }),
                        _ => None,
                    }
                }
                Some("gr_line") => {
                    let a = prim.child("start").and_then(local);
                    let b = prim.child("end").and_then(local);
                    a.zip(b).map(|(a, b)| Contour {
                        segments: vec![Segment::Line { from: a, to: b }],
                    })
                }
                _ => None,
            };
            let closed = matches!(prim.head(), Some("gr_poly" | "gr_rect" | "gr_circle"));
            let mut drawn = false;
            if let Some(shape) = shape.filter(|c| !c.segments.is_empty()) {
                if closed && filled {
                    out.push(shape);
                    drawn = true;
                    if width > 0.0 {
                        self.tally.add(
                            Severity::Warning,
                            "custom pad primitive(s) with a stroke width were drawn without the half-width the stroke adds",
                        );
                    }
                } else if width > 0.0 {
                    out.extend(padshape::stroke(&shape, width));
                    drawn = true;
                }
            }
            if !drawn {
                self.tally.add(
                    Severity::Dropped,
                    "custom pad primitive(s) that are curves, strokes without width, or unreadable (left out of the pad)",
                );
            }
        }
        out
    }

    fn pad(&mut self, pad: &Node, f: &Frame, owner: Option<&str>, footprint_margin: Option<f64>) {
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
        let margin = pad
            .child("solder_mask_margin")
            .and_then(|m| m.num(0))
            .filter(|m| m.is_finite())
            .or(footprint_margin);
        for layer in targets {
            if let Some(shapes) = &polygon {
                self.copper.push(CopperItem::PolygonPad {
                    bottom: None,
                    layer,
                    at: center,
                    shapes: shapes
                        .iter()
                        .map(|c| padshape::placed(c, center, rotation_deg))
                        .collect(),
                    drill,
                    plated,
                    owner: owner.map(str::to_string),
                    tented: [None, None],
                    mask_expansion: [margin, margin],
                });
                continue;
            }
            self.copper.push(CopperItem::Pad {
                bottom: None,
                layer,
                at: center,
                shape,
                size_x: sx,
                size_y: sy,
                rotation_deg,
                drill,
                plated,
                owner: owner.map(str::to_string),
                tented: [None, None],
                mask_expansion: [margin, margin],
            });
        }
        if hole_only_note {
            self.tally
                .add(Severity::Dropped, "pad(s) with no copper layer and no hole");
        }
    }
}

/// The names KiCad shows for the layers (`F.SilkS` shows as `F.Silkscreen`): the user name the
/// layer table gives after the layer's kind, or the canonical name when it gives none.
fn shown_layer_names(root: &Node) -> HashMap<String, String> {
    let mut names = HashMap::new();
    if let Some(layers) = root.child("layers") {
        for entry in layers.args() {
            let Node::List(items) = entry else { continue };
            let (Some(name), Some(shown)) = (
                items.get(1).and_then(Node::text),
                items.get(3).and_then(Node::text),
            ) else {
                continue;
            };
            names.insert(name.to_string(), shown.to_string());
        }
    }
    names
}

/// The title block's fields as text variables: `TITLE`, `REVISION`, `COMPANY`, `ISSUE_DATE` and
/// `COMMENT1` to `COMMENT9`.
fn title_block_variables(root: &Node) -> Vec<(String, String)> {
    let Some(tb) = root.child("title_block") else {
        return Vec::new();
    };
    let mut vars = Vec::new();
    for (key, name) in [
        ("title", "TITLE"),
        ("rev", "REVISION"),
        ("company", "COMPANY"),
        ("date", "ISSUE_DATE"),
    ] {
        if let Some(v) = tb.child(key).and_then(|c| c.str_at(0)) {
            vars.push((name.to_string(), v.to_string()));
        }
    }
    for c in tb.children("comment") {
        if let (Some(n), Some(v)) = (c.num(0), c.str_at(1)) {
            if (1.0..=9.0).contains(&n) && n.fract() == 0.0 {
                vars.push((format!("COMMENT{}", n as u32), v.to_string()));
            }
        }
    }
    vars
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
/// `variables` is what the texts' text variables may use besides the board file.
/// The board's variants (`(variants (variant (name "LITE")))`, KiCad 10, `k10-dnp`) with what
/// each footprint's entries change in them: `(dnp yes)` leaves the part out, `(dnp no)` fits a
/// part the board marks do-not-populate (R2 in LITE), a `Value` field changes its comment. A
/// part is never in both lists: when two entries of one variant disagree, the first is kept.
fn board_variants(root: &Node, entries: &[FootprintVariant], tally: &mut Tally) -> Vec<Variant> {
    let mut variants: Vec<Variant> = Vec::new();
    for v in root
        .child("variants")
        .into_iter()
        .flat_map(|v| v.children("variant"))
    {
        match v.child("name").and_then(|n| n.str_at(0)) {
            Some(name) if !variants.iter().any(|seen| seen.name == name) => {
                variants.push(Variant {
                    name: name.to_string(),
                    ..Variant::default()
                });
            }
            Some(_) => tally.add(
                Severity::Warning,
                "board variant(s) listed twice (read once)",
            ),
            None => tally.add(
                Severity::Warning,
                "board variant(s) without a name (not read)",
            ),
        }
    }
    for e in entries {
        let Some(variant) = variants.iter_mut().find(|v| v.name == e.variant) else {
            tally.add(
                Severity::Warning,
                format!(
                    "footprint variant entr(ies) for {}, a variant the board does not list (not \
                     read)",
                    e.variant
                ),
            );
            continue;
        };
        let d = &e.designator;
        let both = || {
            format!(
                "footprint variant entr(ies) that both fit and leave out a part in {} (the first \
                 is kept)",
                e.variant
            )
        };
        match e.dnp {
            Some(true) if variant.fitted.contains(d) => tally.add(Severity::Warning, both()),
            Some(true) if !variant.not_fitted.contains(d) => variant.not_fitted.push(d.clone()),
            // Fitting a part the board fits changes nothing; only a DNP part is listed.
            Some(false) if e.board_dnp && variant.not_fitted.contains(d) => {
                tally.add(Severity::Warning, both());
            }
            Some(false) if e.board_dnp && !variant.fitted.contains(d) => {
                variant.fitted.push(d.clone());
            }
            _ => {}
        }
        if let Some(comment) = &e.value {
            variant.values.push(VariantValue {
                designator: e.designator.clone(),
                comment: comment.clone(),
            });
        }
    }
    variants
}

pub fn read(mut root: Node, variables: &Variables) -> ReadOutcome {
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
                "This board was saved by KiCad 8 or older (file version {v}). We read KiCad 9 and 10 boards: open it in KiCad 9 or 10, save it, and convert again."
            ),
            None => "This board file has no version, so we cannot tell which KiCad saved it. We read KiCad 9 and 10 boards: open it in KiCad 9 or 10, save it, and convert again.".to_string(),
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
        default_tenting: root
            .child("setup")
            .and_then(|s| s.child("tenting"))
            .map(tenting_sides),
        fonts_unused: Vec::new(),
        fonts_used: Vec::new(),
        embedded: Vec::new(),
        fields: Vec::new(),
        sources: Vec::new(),
        shown_layer_names: shown_layer_names(root),
        variant_entries: Vec::new(),
    };
    let mut font_notes = Vec::new();
    r.embedded = crate::fonts::read(root, &mut font_notes);
    r.fonts_unused = vec![true; r.embedded.len()];
    for n in font_notes {
        r.tally.add(n.severity, n.message);
    }

    for item in root.args() {
        let Some(head) = item.head() else { continue };
        match head {
            "version" | "generator" | "generator_version" | "general" | "paper" | "title_block"
            | "layers" | "setup" | "net" | "embedded_fonts" | "embedded_files" | "uuid"
            | "property" | "variants" => {}
            "footprint" | "module" => r.footprint(item),
            "segment" => r.segment(item),
            "arc" => r.track_arc(item),
            "via" => r.via(item),
            "zone" => r.zone(item),
            "gr_text" => {
                if let (Some(spot), Some(text)) = (Reader::text_spot(item), item.str_at(0)) {
                    r.text(text, item, spot, &Frame::IDENTITY, None);
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
    for (e, unused) in r.embedded.iter().zip(&r.fonts_unused) {
        if *unused {
            r.tally.add(
                Severity::Info,
                format!(
                    "embedded font(s) ({}) that no text uses were not handed over",
                    e.file_name
                ),
            );
        }
    }
    r.tally.add(
        Severity::Info,
        "board(s) whose stack-up colors, materials, copper weights and finish were not read",
    );
    let scope = Scope {
        title_block: title_block_variables(root),
        properties: root
            .children("property")
            .filter_map(|p| Some((p.str_at(0)?.to_string(), p.str_at(1)?.to_string())))
            .collect(),
        footprints: &r.fields,
        variables,
    };
    let mut resolver = Resolver::new(&scope);
    for (text, (raw, owner, layer)) in r.texts.iter_mut().zip(&r.sources) {
        text.content = resolver.resolve(raw, *owner, layer);
    }
    let variants = board_variants(root, &r.variant_entries, &mut r.tally);
    let mut notes = r.tally.into_notes();
    notes.extend(resolver.notes());
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
            mask_openings: Vec::new(),
            graphics: r.graphics,
            texts: r.texts,
            components: r.components,
            appearance: Default::default(),
            free_models: Vec::new(),
            variants,
            solder_mask_expansion: mask_expansion,
        },
        notes,
        models: Vec::new(),
        fonts: r.fonts_used,
    }
}
