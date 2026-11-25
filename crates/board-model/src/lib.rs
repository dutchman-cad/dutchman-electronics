//! The neutral board model and the input-format trait.
//!
//! One input format (Altium `.PcbDoc` first) is one type, in its own crate, that implements
//! [`InputFormat`]. It reads a file's bytes into the neutral [`Board`] below. This crate knows
//! nothing about any one format, and nothing about SolidCraft or Dutchman: Dutchman turns a
//! [`Board`] into a SolidCraft design.
//!
//! ```text
//! board file --InputFormat::read--> Board (this crate) --Dutchman's writer--> .sc
//! ```
//!
//! All lengths are millimetres, all angles degrees, counter-clockwise, Y up.

/// The largest board file this module will open, in bytes. Anything bigger is refused up front.
pub const MAX_INPUT_BYTES: usize = 256 << 20;

// ---------------------------------------------------------------------------------------------
// The neutral board model
// ---------------------------------------------------------------------------------------------

/// A point on the board, in millimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Which face of the board something is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
}

/// What a layer is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    Copper,
    Silkscreen,
    SolderMask,
    Paste,
    Outline,
    Other,
}

/// One layer of the board stack, as the source file names it.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// The name the source file uses, e.g. "Top Layer" or "Top Overlay".
    pub name: String,
    pub kind: LayerKind,
    /// `None` for inner copper layers and layers that belong to neither face.
    pub side: Option<Side>,
}

/// One piece of a contour.
#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    Line {
        from: Point,
        to: Point,
    },
    /// A circular arc from `start_deg` to `end_deg`: counter-clockwise, or clockwise when
    /// `clockwise` is set (a concave rounded corner on a contour that runs counter-clockwise).
    /// Equal angles make a whole circle. [`Segment::sweep_deg`] gives the signed sweep.
    Arc {
        center: Point,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
        clockwise: bool,
    },
}

impl Segment {
    /// The angle an arc sweeps from `start_deg` to `end_deg`, in degrees: in (0, 360] when it
    /// runs counter-clockwise, in [-360, 0) when clockwise. Equal angles are a whole circle.
    /// `None` for a line.
    pub fn sweep_deg(&self) -> Option<f64> {
        let Segment::Arc {
            start_deg,
            end_deg,
            clockwise,
            ..
        } = *self
        else {
            return None;
        };
        let raw = if clockwise {
            start_deg - end_deg
        } else {
            end_deg - start_deg
        };
        let s = raw.rem_euclid(360.0);
        let s = if s < 1e-9 { 360.0 } else { s };
        Some(if clockwise { -s } else { s })
    }
}

/// A chain of segments. Closed when the last ends where the first starts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contour {
    pub segments: Vec<Segment>,
}

/// The board shape: one outer contour, plus holes and cutouts inside it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outline {
    pub outer: Contour,
    pub cutouts: Vec<Contour>,
}

/// A colour as a board file records it: 8-bit sRGB, and how opaque it is (1 = solid, 0 = clear).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub opacity: f64,
}

/// How the board's faces look, as the file records it. `None` where the file says nothing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Appearance {
    pub top_solder_mask: Option<Color>,
    pub bottom_solder_mask: Option<Color>,
    pub top_silkscreen: Option<Color>,
    pub bottom_silkscreen: Option<Color>,
    /// The finish plated on the outer copper where the mask leaves it bare.
    pub top_finish: Option<SurfaceFinish>,
    pub bottom_finish: Option<SurfaceFinish>,
}

/// A surface finish on the outer copper, as the file names it.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceFinish {
    /// The process, such as `ENIG` or `HASL`.
    pub process: String,
    /// The material, such as `Nickel, Gold` or `PbSn`.
    pub material: String,
    pub color: Option<Color>,
    /// Millimetres.
    pub thickness: Option<f64>,
}

/// A pad's shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PadShape {
    Round,
    Rectangle,
    Oval,
}

/// A piece of copper. `layer` is an index into [`Board::layers`].
#[derive(Clone, Debug, PartialEq)]
pub enum CopperItem {
    Track {
        layer: usize,
        from: Point,
        to: Point,
        width: f64,
        net: Option<String>,
    },
    TrackArc {
        layer: usize,
        center: Point,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
        width: f64,
        net: Option<String>,
    },
    Via {
        at: Point,
        diameter: f64,
        drill: f64,
    },
    Pad {
        layer: usize,
        at: Point,
        shape: PadShape,
        size_x: f64,
        size_y: f64,
        rotation_deg: f64,
        /// A through-hole pad has a drill diameter.
        drill: Option<f64>,
        /// The hole is plated (lined with copper). Always false for a pad without a hole.
        plated: bool,
        /// The designator of the component this pad belongs to, if any.
        owner: Option<String>,
    },
    /// A filled copper area (a pour or polygon). Each hole is a closed contour inside `contour`,
    /// in any winding (clearances around other copper, thermal-relief gaps); the copper is the
    /// outline minus the holes.
    Region {
        layer: usize,
        contour: Contour,
        holes: Vec<Contour>,
    },
}

/// A line or arc drawn on a layer that is not copper (today the silkscreen), in board
/// coordinates like pads and texts. `layer` is an index into [`Board::layers`], whose `kind` and
/// `side` say which layer it is. `owner` is the designator of the component it belongs to, if any.
#[derive(Clone, Debug, PartialEq)]
pub enum Graphic {
    Line {
        layer: usize,
        from: Point,
        to: Point,
        width: f64,
        owner: Option<String>,
    },
    /// A circular arc from `start_deg` to `end_deg`, counter-clockwise, like [`Segment::Arc`] and
    /// [`CopperItem::TrackArc`].
    Arc {
        layer: usize,
        center: Point,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
        width: f64,
        owner: Option<String>,
    },
}

/// Text on a layer (silkscreen labels, copper text).
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub layer: usize,
    pub at: Point,
    pub content: String,
    pub height: f64,
    pub rotation_deg: f64,
    /// Reads mirrored (text on the bottom side).
    pub mirrored: bool,
    /// Where its letter shapes come from.
    pub font: TextFont,
}

/// The letter shapes of a [`Text`].
#[derive(Clone, Debug, PartialEq)]
pub enum TextFont {
    /// The CAD program's own single-line font, drawn with a pen `width` mm wide.
    Stroke { width: f64 },
    /// A TrueType font, by family name and style; its outlines come from the font file, which
    /// [`ReadOutcome::font`] finds when the board embeds it.
    TrueType {
        name: String,
        bold: bool,
        italic: bool,
    },
}

/// Where a 3D model for a component comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelSource {
    /// Stored inside the board file itself: the `id` of one of [`ReadOutcome::models`], which
    /// holds the unpacked file. Parts that use the same model share one id.
    Embedded { id: String },
    /// A file name the board refers to; the user's library may or may not have it.
    External { file: String },
    /// No file: a prism made by extruding a flat outline, in component coordinates (mm, the
    /// component's origin at (0, 0), not rotated), from `standoff` above the board face the
    /// component sits on up to `height`. Altium calls this an extruded 3D body.
    Extruded {
        outline: Contour,
        standoff: f64,
        height: f64,
    },
}

/// A reference to a component's 3D model, with the model's own placement on the component.
///
/// `offset`, `rotation_deg` and an [`ModelSource::Extruded`] outline are in the component's own
/// frame (see [`Placement`]). The model is turned about its own X, then Y, then Z axis, and then
/// moved by `offset`.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelRef {
    pub name: String,
    pub source: ModelSource,
    /// Model offset from the component origin, millimetres.
    pub offset: [f64; 3],
    /// Model rotation about X, Y, Z in degrees.
    pub rotation_deg: [f64; 3],
    /// What an [`ModelSource::Extruded`] body is drawn in, when the file says. `None` for a model
    /// file, which brings its own colours.
    pub color: Option<Color>,
}

/// A component placed on the board.
///
/// The component's own frame: its origin at `at` on the face of the board it sits on, +Z out of
/// that face. A top-side part's frame is the board's, turned by `rotation_deg` about Z, at the
/// top face. A bottom-side part's frame is also turned by `rotation_deg` about Z, at the bottom
/// face, and then flipped over by half a turn about its own X axis, so its +Z points down and its
/// +Y is the board's -Y (before the turn). So a point (x, y) in the board seen from the top, turned
/// back into a bottom-side part's frame, has its y negated, and a turn about Z in that frame is a
/// turn the other way seen from the top. (Test boards confirm this for bottom-side parts at 180 and
/// 90 degrees, with models turned on their part.)
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    /// "R1", "U3".
    pub designator: String,
    /// The footprint name, e.g. "0603".
    pub footprint: String,
    pub at: Point,
    pub rotation_deg: f64,
    pub side: Side,
    pub model: Option<ModelRef>,
}

/// A whole board, read from a file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Board {
    pub name: String,
    /// Substrate thickness, when the file says.
    pub thickness: Option<f64>,
    pub layers: Vec<Layer>,
    pub outline: Outline,
    pub copper: Vec<CopperItem>,
    /// Lines and arcs on non-copper layers (silkscreen).
    pub graphics: Vec<Graphic>,
    pub texts: Vec<Text>,
    pub components: Vec<Placement>,
    /// Solder mask and silkscreen colours.
    pub appearance: Appearance,
}

// ---------------------------------------------------------------------------------------------
// The input-format trait
// ---------------------------------------------------------------------------------------------

/// How sure a format is that a file is its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detection {
    /// Not this format.
    No,
    /// The file name fits but the contents were not checked.
    MaybeByName,
    /// The contents are this format.
    Yes,
}

/// How serious a [`Note`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Something was left out of the result.
    Dropped,
    /// Something was read but may be wrong.
    Warning,
    /// Just good to know.
    Info,
}

/// One line of the read report. Nothing is dropped silently: a dropped item gets a note.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub severity: Severity,
    pub message: String,
}

/// A 3D model file stored inside the board file, unpacked, for [`ModelSource::Embedded`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EmbeddedModel {
    /// What [`ModelSource::Embedded`] points to.
    pub id: String,
    /// The model's file name as the board gives it, e.g. "box-1608.step".
    pub name: String,
    /// The STEP file itself.
    pub step: Vec<u8>,
}

/// A TrueType font file stored inside the board file, unpacked, for [`TextFont::TrueType`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EmbeddedFont {
    /// The family, as [`TextFont::TrueType`] names it, e.g. "DejaVu Sans Mono".
    pub family: String,
    /// The face's full name, e.g. "DejaVu Sans Mono Bold".
    pub full_name: String,
    /// The style's name, e.g. "Book", "Bold" or "Oblique".
    pub style: String,
    pub bold: bool,
    pub italic: bool,
    /// The TrueType file itself. It keeps its own licence, which the file names.
    pub data: Vec<u8>,
}

impl EmbeddedFont {
    /// True when this is the file for `font`: the same family (in any case) and style.
    pub fn serves(&self, font: &TextFont) -> bool {
        match font {
            TextFont::TrueType { name, bold, italic } => {
                self.family.eq_ignore_ascii_case(name)
                    && self.bold == *bold
                    && self.italic == *italic
            }
            TextFont::Stroke { .. } => false,
        }
    }
}

/// What [`InputFormat::read`] returns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReadOutcome {
    pub board: Board,
    pub notes: Vec<Note>,
    /// The embedded 3D models the board's components use, each once.
    pub models: Vec<EmbeddedModel>,
    /// The embedded font files the board's texts use, each once.
    pub fonts: Vec<EmbeddedFont>,
}

impl ReadOutcome {
    /// The embedded font file for a text's font, if the board has it.
    pub fn font(&self, font: &TextFont) -> Option<&EmbeddedFont> {
        self.fonts.iter().find(|f| f.serves(font))
    }
}

/// Why a file could not be read.
#[derive(Debug)]
pub enum ReadError {
    /// The file is bigger than [`MAX_INPUT_BYTES`].
    TooLarge { bytes: usize },
    /// The file is not this format, or is damaged.
    Invalid(String),
    /// A valid file, but it uses something this version cannot read yet.
    NotYet(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { bytes } => write!(f, "the file is too large ({bytes} bytes)"),
            Self::Invalid(why) => write!(f, "not a readable board file: {why}"),
            Self::NotYet(what) => write!(f, "not supported yet: {what}"),
        }
    }
}

impl std::error::Error for ReadError {}

/// One board file format. Add a format by implementing this; nothing else in Dutchman changes.
///
/// Rules every implementation follows: input is hostile, so it returns an error and never
/// panics; it never opens other files itself (the bytes are handed to it); it records every
/// loss as a [`Note`].
pub trait InputFormat {
    /// A stable id for reports: "altium_pcbdoc".
    fn id(&self) -> &'static str;
    /// A name for people: "Altium PCB document (.PcbDoc)".
    fn name(&self) -> &'static str;
    /// File extensions, lower case, no dot.
    fn extensions(&self) -> &'static [&'static str];
    /// Looks at the first bytes of a file (at least 512 are given when the file has them) and
    /// its extension, and says whether it is this format. Cheap; no full parse.
    fn detect(&self, head: &[u8], extension: Option<&str>) -> Detection;
    /// Reads the whole file into the neutral model.
    fn read(&self, bytes: &[u8]) -> Result<ReadOutcome, ReadError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_of_errors_is_plain_english() {
        let e = ReadError::NotYet("copper".into());
        assert_eq!(e.to_string(), "not supported yet: copper");
    }

    #[test]
    fn a_text_finds_its_font_by_family_and_style() {
        let face = |bold: bool, italic: bool| EmbeddedFont {
            family: "DejaVu Sans Mono".into(),
            bold,
            italic,
            ..EmbeddedFont::default()
        };
        let out = ReadOutcome {
            fonts: vec![face(false, false), face(true, false)],
            ..ReadOutcome::default()
        };
        let wants = |name: &str, bold: bool, italic: bool| TextFont::TrueType {
            name: name.into(),
            bold,
            italic,
        };
        let found = |font: TextFont| out.font(&font).map(|f| (f.bold, f.italic));
        assert_eq!(
            found(wants("DejaVu Sans Mono", false, false)),
            Some((false, false))
        );
        assert_eq!(
            found(wants("dejavu sans mono", true, false)),
            Some((true, false))
        );
        assert_eq!(found(wants("DejaVu Sans Mono", false, true)), None);
        assert_eq!(found(wants("Arial", false, false)), None);
        assert_eq!(found(TextFont::Stroke { width: 0.2 }), None);
    }
}

#[cfg(test)]
mod sweep_tests {
    use super::*;

    fn arc(start_deg: f64, end_deg: f64, clockwise: bool) -> Segment {
        Segment::Arc {
            center: Point { x: 0.0, y: 0.0 },
            radius: 1.0,
            start_deg,
            end_deg,
            clockwise,
        }
    }

    #[test]
    fn an_arc_sweeps_in_its_own_direction() {
        assert_eq!(arc(0.0, 90.0, false).sweep_deg(), Some(90.0));
        assert_eq!(arc(90.0, 0.0, true).sweep_deg(), Some(-90.0));
        assert_eq!(arc(0.0, 90.0, true).sweep_deg(), Some(-270.0));
        assert_eq!(arc(350.0, 10.0, false).sweep_deg(), Some(20.0));
        assert_eq!(arc(30.0, 30.0, false).sweep_deg(), Some(360.0));
        assert_eq!(arc(30.0, 390.0, true).sweep_deg(), Some(-360.0));
        assert_eq!(arc(-90.0, 90.0, false).sweep_deg(), Some(180.0));
        assert_eq!(arc(720.0, 10.0, false).sweep_deg(), Some(10.0));
        assert_eq!(arc(720.0, 10.0, true).sweep_deg(), Some(-350.0));
        // Ends a hair apart are a whole circle, not a sliver, in either direction.
        assert_eq!(arc(30.0, 30.0 + 1e-12, false).sweep_deg(), Some(360.0));
        assert_eq!(arc(30.0, 30.0 - 1e-12, true).sweep_deg(), Some(-360.0));
        let line = Segment::Line {
            from: Point { x: 0.0, y: 0.0 },
            to: Point { x: 1.0, y: 0.0 },
        };
        assert_eq!(line.sweep_deg(), None);
    }
}
