//! Text as pen strokes, in Hershey fonts (`fonts/hershey/`, free to use with the acknowledgement
//! in `fonts/hershey/README.txt`).
//!
//! A stroke text is drawn at the text's height and pen width, in the font family closest to the
//! one it names ([`StrokeFont`]): Roman Simplex for Sans Serif (`rowmans.jhf`), Times Roman for
//! Serif (`timesr.jhf`), and for Default the Roman Simplex glyphs on a fixed advance, see
//! [`default_advance`]. A TrueType text whose font file the board does not embed (or that does
//! not parse) is drawn in Roman Simplex as well. Letter shapes then differ from the CAD program's
//! (and, in Default, so does nothing else about where the text lies: its width and placement
//! follow the CAD program's), which the package says in a Degraded note.
//!
//! A TrueType text whose font file is embedded is drawn with the real letter outlines instead
//! ([`outline`]): filled areas, no pen.
//!
//! A Hershey glyph is a list of pen strokes in font units, y down, with the baseline at 9 and
//! the top of a capital at -12. The text's height is taken as the height of a capital, and its
//! position as the point its [`board_model::TextAnchor`] names (by default the left end of its
//! baseline), before it is mirrored and turned. The anchor is resolved from what is drawn: each
//! line's advance width for the horizontal part, KiCad's text box (constants in `board_model`) for
//! the vertical one.

use std::collections::{HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::OnceLock;

use board_model::{
    HorizontalAnchor, Point, StrokeFont, Text, TextFont, VerticalAnchor, TEXT_BASELINE_GAP,
    TEXT_BOX_HEIGHT, TEXT_CAP_GAP, TEXT_SIDE_GAP_PER_HEIGHT, TEXT_SIDE_GAP_PER_PEN,
};
use geo::algorithm::bool_ops::FillRule;
use geo::orient::{Direction, Orient};
use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

/// Font units from the top of a capital down to the baseline.
const CAP_UNITS: f64 = 21.0;
/// The baseline, in font units (y down).
const BASELINE: f64 = 9.0;
/// Font units from one line's baseline to the next.
const LINE_UNITS: f64 = 32.0;
/// The first character the font holds; the rest follow in order.
const FIRST: u32 = 32;
/// Where `?` sits in the font (it stands in for characters the font lacks).
const QUESTION: usize = b'?' as usize - FIRST as usize;
/// The tallest text drawn, in mm; a taller one is unusable.
const MAX_HEIGHT: f64 = 1_000.0;
/// The most characters drawn of one text; the rest are counted as left out.
pub(super) const MAX_CHARS: usize = 1_000;
/// The pen width for a TrueType text, as a share of its height (the width the test boards'
/// stroke texts use: 0.2 for 2.0).
const TRUETYPE_PEN: f64 = 0.1;

/// One glyph: its left and right edges and its strokes, in font units.
struct Glyph {
    left: f64,
    right: f64,
    strokes: Vec<Vec<(f64, f64)>>,
}

/// The glyphs a stroke font is drawn with. Default has no free equivalent: it takes the Roman
/// Simplex shapes (see [`default_advance`] for its spacing).
fn font(which: StrokeFont) -> &'static [Glyph] {
    static SIMPLEX: OnceLock<Vec<Glyph>> = OnceLock::new();
    static TIMES: OnceLock<Vec<Glyph>> = OnceLock::new();
    match which {
        StrokeFont::Serif => {
            TIMES.get_or_init(|| parse(include_str!("../../fonts/hershey/timesr.jhf")))
        }
        StrokeFont::SansSerif | StrokeFont::Default | StrokeFont::KiCad => {
            SIMPLEX.get_or_init(|| parse(include_str!("../../fonts/hershey/rowmans.jhf")))
        }
    }
}

/// The advance, in mm, Altium's Default stroke font puts between two characters at text height
/// `h`: the same for every character (the font is monospaced). Fitted to the right ends of our
/// own test boards' text rectangles (`crates/altium-pcbdoc/tests/boards/README.txt`), a
/// rectangle reaching one stroke width past the ink on each side, with the first character at
/// the reference point; each sample is the advance at which our centered Roman Simplex glyphs
/// end where Altium's ink does:
/// - height 2.0, stroke 0.2 (one-text "TEST" against one-rotated-text "ROT", which end in the
///   same letter): (17.53097 - 10) - (20.53162 - 15) = 1.99935 mm;
/// - height 1.5, stroke 0.15 (special-strings "L: Top Overlay" against "Top Overlay", three
///   characters more): (22.144 - 17.895) / 3 = 1.41633 mm;
/// - height 1.2, stroke 0.15 (stroke-fonts, Default's three lines, right ends 32.740, 32.140 and
///   34.340 for 26, 26 and 29 characters): 1.18392, 1.16375 and 1.12747 mm. The lines differ
///   because our glyph shapes end differently from Altium's (and the digit line holds "[]",
///   which Altium draws as one box); the sample is their mean, 1.15838 mm.
///
/// The three points are not on one line (the advance is 0.965, 0.944 and 1.000 of the height),
/// so the advance is the straight pieces through them, and beyond the ends the nearest piece
/// continued. It is exact at 1.5 and 2.0, and within 0.03 mm per character at 1.2.
const DEFAULT_ADVANCES: [(f64, f64); 3] = [(1.2, 1.15838), (1.5, 1.41633), (2.0, 1.99935)];
/// The narrowest advance, as a share of the height, so a very small text keeps its letters apart.
const MIN_ADVANCE: f64 = 0.5;

fn default_advance(height: f64) -> f64 {
    let [(h0, a0), (h1, a1), (h2, a2)] = DEFAULT_ADVANCES;
    let (ha, aa, hb, ab) = if height <= h1 {
        (h0, a0, h1, a1)
    } else {
        (h1, a1, h2, a2)
    };
    (aa + (height - ha) * (ab - aa) / (hb - ha)).max(MIN_ADVANCE * height)
}

/// How far Altium's proportional stroke fonts (Sans Serif and Serif) advance, as a share of the
/// Hershey glyph's own width, for the three kinds of character we measured: capitals, lower
/// case, and the rest (digits, space and punctuation, which the board holds in one line, so
/// they share one factor). The Hershey shapes are not scaled, only the spacing is tightened.
/// Fitted at height 1.2 to the right ends of the stroke-fonts text rectangles (see
/// [`DEFAULT_ADVANCES`] for how the rectangle relates to the ink): per line, the factor `k` for
/// which our ink ends at the rectangle's right end less one stroke width, i.e.
/// `k = (target - last glyph's ink width) / (sum of the other glyphs' widths)`:
/// - Sans Serif: A-Z 29.218, a-z 24.711, digits and punctuation 28.03: 0.9263, 0.8982, 0.8644;
/// - Serif: A-Z 31.075, a-z 28.133, digits and punctuation 26.79: 0.9353, 0.9237, 0.8370.
///
/// The punctuation set is a single measurement, so the factor is a mean over those characters
/// (Altium sets punctuation much tighter than Hershey does); a line with other mixes is only
/// as close as its mix is to these.
#[derive(Clone, Copy)]
struct Tightness {
    upper: f64,
    lower: f64,
    other: f64,
}

const SANS_SERIF_TIGHTNESS: Tightness = Tightness {
    upper: 0.9263,
    lower: 0.8982,
    other: 0.8644,
};
const SERIF_TIGHTNESS: Tightness = Tightness {
    upper: 0.9353,
    lower: 0.9237,
    other: 0.8370,
};

impl Tightness {
    fn of(self, c: char) -> f64 {
        match c {
            'A'..='Z' => self.upper,
            'a'..='z' => self.lower,
            _ => self.other,
        }
    }
}

/// Parses the "James Hurt" format: per glyph a 5-character number, a 3-character count of
/// coordinate pairs, then the pairs (the first is the left and right edge), each letter less
/// `R`; " R" lifts the pen. Entries may wrap across lines.
fn parse(source: &str) -> Vec<Glyph> {
    let chars: Vec<u8> = source
        .bytes()
        .filter(|&b| b != b'\n' && b != b'\r')
        .collect();
    let mut glyphs = Vec::new();
    let mut at = 0usize;
    let unit = |b: u8| f64::from(b) - f64::from(b'R');
    while let Some(count) = chars
        .get(at.saturating_add(5)..at.saturating_add(8))
        .and_then(|c| std::str::from_utf8(c).ok())
        .and_then(|c| c.trim().parse::<usize>().ok())
    {
        let start = at.saturating_add(8);
        let end = start.saturating_add(count.saturating_mul(2));
        let Some(pairs) = chars.get(start..end) else {
            break;
        };
        let Some((&[l, r], rest)) = pairs.as_chunks::<2>().0.split_first() else {
            break;
        };
        let mut strokes = vec![Vec::new()];
        for pair in rest {
            match *pair {
                [b' ', b'R'] => strokes.push(Vec::new()),
                [x, y] => {
                    if let Some(s) = strokes.last_mut() {
                        s.push((unit(x), unit(y)));
                    }
                }
            }
        }
        strokes.retain(|s| !s.is_empty());
        glyphs.push(Glyph {
            left: unit(l),
            right: unit(r),
            strokes,
        });
        at = end;
    }
    glyphs
}

/// A text drawn as pen strokes on the board.
pub(super) struct Drawn {
    /// Each stroke as a chain of points in board coordinates.
    pub strokes: Vec<Vec<Point>>,
    /// The pen width, in mm.
    pub pen: f64,
    /// Characters the font does not hold, drawn as `?`.
    pub missing: usize,
    /// Characters past [`MAX_CHARS`], left out.
    pub cut: usize,
}

fn positive(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

/// True when the text's height (not zero, not over [`MAX_HEIGHT`]), position (within the
/// overlay limit) and rotation can be drawn. Written so that NaN fails too.
fn usable(text: &Text) -> bool {
    let in_range = |v: f64| v.abs() <= super::MAX_COORDINATE;
    positive(text.height)
        && text.height <= MAX_HEIGHT
        && in_range(text.at.x)
        && in_range(text.at.y)
        && text.rotation_deg.is_finite()
}

/// The pen strokes of `text`, or `None` when its height (zero, or over [`MAX_HEIGHT`]), pen
/// width, position (beyond the overlay limit) or rotation is not usable. Nothing is drawn before
/// those checks.
pub(super) fn draw(text: &Text) -> Option<Drawn> {
    let (pen, which) = match &text.font {
        TextFont::Stroke { width, font } => (*width, *font),
        TextFont::TrueType { .. } => (text.height * TRUETYPE_PEN, StrokeFont::SansSerif),
    };
    if !usable(text) || !positive(pen) {
        return None;
    }
    let glyphs = font(which);
    let question = glyphs.get(QUESTION)?;
    let scale = text.height / CAP_UNITS;
    // Default: every character takes a cell of one advance; its glyph is centered in it, and
    // squeezed sideways when wider than the cell. The others run on the glyphs' widths times a fitted
    // factor per kind of character ([`Tightness`]), each glyph starting at its own pen position.
    let cell = (which == StrokeFont::Default).then(|| default_advance(text.height) / scale);
    // (A TrueType text drawn in strokes keeps the glyphs' own widths: nothing was measured for it.)
    let tightness = match (&text.font, which) {
        (TextFont::TrueType { .. }, _) | (_, StrokeFont::Default | StrokeFont::KiCad) => None,
        (_, StrokeFont::SansSerif) => Some(SANS_SERIF_TIGHTNESS),
        (_, StrokeFont::Serif) => Some(SERIF_TIGHTNESS),
    };
    let (sin, cos) = text.rotation_deg.to_radians().sin_cos();
    let mirror = if text.mirrored { -1.0 } else { 1.0 };
    let place = |x: f64, y: f64| {
        let x = x * mirror;
        Point {
            x: text.at.x + x * cos - y * sin,
            y: text.at.y + x * sin + y * cos,
        }
    };
    let mut drawn = Drawn {
        strokes: Vec::new(),
        pen,
        missing: 0,
        cut: text.content.chars().count().saturating_sub(MAX_CHARS),
    };
    // First lay the glyphs out from the left end of each line's baseline, noting each line's
    // advance width; then shift every line by its anchor.
    let line_units = text.line_pitch.map_or(LINE_UNITS, |p| p * CAP_UNITS);
    let mut placed: Vec<(&Glyph, f64, f64, usize)> = Vec::new();
    let mut widths: Vec<f64> = Vec::new();
    let (mut pen_x, mut line) = (0.0f64, 0usize);
    for c in text.content.chars().take(MAX_CHARS) {
        match c {
            '\n' => {
                widths.push(pen_x);
                pen_x = 0.0;
                line += 1;
                continue;
            }
            '\r' => continue,
            _ => {}
        }
        let glyph = u32::from(c)
            .checked_sub(FIRST)
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| glyphs.get(i))
            .unwrap_or_else(|| {
                drawn.missing = drawn.missing.saturating_add(1);
                question
            });
        let width = glyph.right - glyph.left;
        let (squeeze, inset) = match cell {
            Some(cell) if width > cell => (cell / width, 0.0),
            Some(cell) => (1.0, (cell - width) / 2.0),
            None => (1.0, 0.0),
        };
        placed.push((glyph, pen_x + inset, squeeze, line));
        pen_x += cell.unwrap_or_else(|| width * tightness.map_or(1.0, |t| t.of(c)));
    }
    widths.push(pen_x);
    let rise = baseline_rise(text, widths.len(), line_units * scale);
    for (glyph, x0, squeeze, line) in placed {
        // The line's box reaches the side gap past its advance width on each side.
        // The default anchor is the bare left end of the baseline, with no box around it.
        let gap = if text.anchor == board_model::TextAnchor::default() {
            0.0
        } else {
            (TEXT_SIDE_GAP_PER_HEIGHT * text.height + TEXT_SIDE_GAP_PER_PEN * pen) / scale
        };
        let shift = widths.get(line).map_or(0.0, |w| {
            horizontal_share(text.anchor.horizontal) * (w + 2.0 * gap) - gap
        });
        for s in &glyph.strokes {
            drawn.strokes.push(
                s.iter()
                    .map(|&(x, y)| {
                        place(
                            (x0 - shift + (x - glyph.left) * squeeze) * scale,
                            rise + (BASELINE - y - line as f64 * line_units) * scale,
                        )
                    })
                    .collect(),
            );
        }
    }
    Some(drawn)
}

/// How much of a line's advance width the anchor puts to its left.
fn horizontal_share(h: HorizontalAnchor) -> f64 {
    match h {
        HorizontalAnchor::Left => 0.0,
        HorizontalAnchor::Center => 0.5,
        HorizontalAnchor::Right => 1.0,
    }
}

/// How far above the anchor the first line's baseline lies, in the text's own frame (y up), for
/// a text of `lines` lines `pitch` mm apart: the baseline itself, or the text box of
/// [`board_model::TextAnchor`] hung from, centered on or standing on the anchor.
fn baseline_rise(text: &Text, lines: usize, pitch: f64) -> f64 {
    let h = text.height;
    let below_first = lines.saturating_sub(1) as f64 * pitch;
    let box_height = TEXT_BOX_HEIGHT * h + below_first;
    let first_baseline_below_top = (TEXT_CAP_GAP + 1.0) * h;
    match text.anchor.vertical {
        VerticalAnchor::Baseline => 0.0,
        VerticalAnchor::Top => -first_baseline_below_top,
        VerticalAnchor::Center => box_height / 2.0 - first_baseline_below_top,
        VerticalAnchor::Bottom => TEXT_BASELINE_GAP * h + below_first,
    }
}

/// The most points one glyph may flatten to; a glyph with more is not drawn.
const MAX_GLYPH_POINTS: usize = 20_000;
/// The most contours one glyph may have; a glyph with more is not drawn.
const MAX_GLYPH_CONTOURS: usize = 64;
/// The most points a glyph may have to be united: past it the glyph is not drawn, so a hostile
/// glyph cannot make the union slow.
const MAX_UNION_POINTS: usize = 5_000;
/// The most levels of composite glyphs inside composite glyphs a glyph may have. The font
/// library follows these without a limit on how many components each level has, so the
/// structure is checked before any glyph is outlined. This is harfbuzz/ttf-parser#220
/// (composites sharing a subtree cost branching^depth outline calls), fixed upstream but in no
/// release yet: drop this check, and [`MAX_COMPOSITE_PARTS`], once we use a ttf-parser release
/// that carries the fix.
const MAX_COMPOSITE_DEPTH: u32 = 8;
/// The most simple glyphs a glyph may expand to once all its composite parts are followed.
const MAX_COMPOSITE_PARTS: u64 = 64;
/// A text's curves are followed to this fraction of its height, so letters stay smooth at any
/// size (1.2 mm letters within 0.003 mm), but never more coarsely than [`super::CHORD`].
const CURVE_FRACTION: f64 = 400.0;
/// The most straight pieces one curve is cut into (DejaVu's curves need a handful).
const MAX_CURVE_STEPS: usize = 256;

/// The font file cannot be used: it does not parse, or its heights are not a usable size.
pub(super) struct FontError;

/// A TrueType text drawn with its glyph outlines.
#[derive(Default)]
pub(super) struct Outlined {
    /// The filled areas in board coordinates: outer rings counter-clockwise, holes clockwise.
    pub areas: Vec<Polygon>,
    /// The flattened points all glyphs together used, to be counted against the board's budget.
    pub points: usize,
    /// Characters the font has no glyph for: drawn as its `.notdef` glyph if it has an outline
    /// for it, else as nothing.
    pub missing: usize,
    /// Characters past [`MAX_CHARS`], left out.
    pub cut: usize,
    /// Glyphs left out because the budget was used up.
    pub over_budget: usize,
    /// Glyphs left out because they are too complex (past [`MAX_GLYPH_POINTS`] or
    /// [`MAX_GLYPH_CONTOURS`]), reach beyond the overlay limit or could not be built.
    pub failed: usize,
}

/// How far a flattened curve of a text `height` mm high may lie from the true one.
fn curve_tolerance(height: f64) -> f64 {
    (height / CURVE_FRACTION).min(super::CHORD)
}

/// Flattens a glyph outline into rings of points (in mm, y up), within `tolerance`.
struct Flatten {
    /// The most a flattened curve may stray from the true one, in mm.
    tolerance: f64,
    /// Font units to mm.
    scale: f64,
    /// Shift of the glyph in font units, and of its line in mm.
    shift_x: f64,
    shift_y: f64,
    rings: Vec<Vec<Coord>>,
    ring: Vec<Coord>,
    last: Coord,
    points: usize,
    /// Set when a limit was hit; the rest of the outline is ignored.
    over: bool,
}

impl Flatten {
    fn at(&self, x: f32, y: f32) -> Coord {
        Coord {
            x: (f64::from(x) + self.shift_x) * self.scale,
            y: f64::from(y) * self.scale + self.shift_y,
        }
    }

    fn push(&mut self, c: Coord) {
        if self.points >= MAX_GLYPH_POINTS {
            self.over = true;
            return;
        }
        self.points = self.points.saturating_add(1);
        self.ring.push(c);
        self.last = c;
    }

    fn finish(&mut self) {
        let ring = std::mem::take(&mut self.ring);
        if ring.len() >= 3 {
            if self.rings.len() >= MAX_GLYPH_CONTOURS {
                self.over = true;
            } else {
                self.rings.push(ring);
            }
        }
    }

    /// The number of pieces a curve whose second difference is `d` mm long needs to stay
    /// within the tolerance: the error of `n` equal pieces is `factor * d / n^2`.
    fn steps(&self, d: f64, factor: f64) -> usize {
        let n = (factor * d / self.tolerance).sqrt().ceil();
        if n.is_finite() {
            (n.max(1.0) as usize).min(MAX_CURVE_STEPS)
        } else {
            MAX_CURVE_STEPS
        }
    }
}

impl OutlineBuilder for Flatten {
    fn move_to(&mut self, x: f32, y: f32) {
        self.finish();
        let c = self.at(x, y);
        if !self.over {
            self.push(c);
        }
    }

    fn line_to(&mut self, x: f32, y: f32) {
        if !self.over {
            let c = self.at(x, y);
            self.push(c);
        }
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (p0, p1, p2) = (self.last, self.at(x1, y1), self.at(x, y));
        let d = (p0.x - 2.0 * p1.x + p2.x).hypot(p0.y - 2.0 * p1.y + p2.y);
        let n = self.steps(d, 0.25);
        for i in 1..=n {
            if self.over {
                return;
            }
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            self.push(Coord {
                x: u * u * p0.x + 2.0 * u * t * p1.x + t * t * p2.x,
                y: u * u * p0.y + 2.0 * u * t * p1.y + t * t * p2.y,
            });
        }
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (p0, p1, p2, p3) = (self.last, self.at(x1, y1), self.at(x2, y2), self.at(x, y));
        let a = (p0.x - 2.0 * p1.x + p2.x).hypot(p0.y - 2.0 * p1.y + p2.y);
        let b = (p1.x - 2.0 * p2.x + p3.x).hypot(p1.y - 2.0 * p2.y + p3.y);
        let n = self.steps(a.max(b), 0.75);
        for i in 1..=n {
            if self.over {
                return;
            }
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.push(Coord {
                x: w0 * p0.x + w1 * p1.x + w2 * p2.x + w3 * p3.x,
                y: w0 * p0.y + w1 * p1.y + w2 * p2.y + w3 * p3.y,
            });
        }
    }

    fn close(&mut self) {
        self.finish();
    }
}

/// Twice the signed area of a ring: positive when counter-clockwise.
fn twice_area(ring: &[Coord]) -> f64 {
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum()
}

/// One glyph's rings as filled areas, by the non-zero winding rule the font format uses: a point
/// is inside when the rings wind round it a net number of times other than zero, so a ring of
/// the opposite winding inside another is a hole, and two separate rings of opposite winding are
/// both filled. The rings keep their own winding (geo's polygon type is not reoriented before
/// the fill rule reads it). `place` maps a ring point from the text's own frame to the board.
/// `None` when a point lands past the overlay limit, the glyph is too complex to unite, or the
/// union fails.
fn glyph_areas(rings: &[Vec<Coord>], place: &dyn Fn(Coord) -> Coord) -> Option<Vec<Polygon>> {
    let points = rings.iter().fold(0usize, |n, r| n.saturating_add(r.len()));
    if rings.len() > MAX_GLYPH_CONTOURS || points > MAX_UNION_POINTS {
        return None;
    }
    let mut polygons = Vec::new();
    for ring in rings.iter().filter(|r| twice_area(r).abs() > 1e-12) {
        let ring: Vec<Coord> = ring.iter().map(|&c| place(c)).collect();
        let in_range =
            |c: &Coord| c.x.abs() <= super::MAX_COORDINATE && c.y.abs() <= super::MAX_COORDINATE;
        if !ring.iter().all(in_range) {
            return None;
        }
        polygons.push(Polygon::new(LineString::from(ring), Vec::new()));
    }
    let shape = catch_unwind(AssertUnwindSafe(|| {
        MultiPolygon::new(polygons)
            .union_with_fill_rule(&MultiPolygon::new(Vec::new()), FillRule::NonZero)
            .orient(Direction::Default)
    }))
    .ok()?;
    Some(shape.0)
}

/// The glyph table's composite glyphs, measured before anything is outlined: the font library
/// follows a composite glyph's parts to a depth of 32 but with no limit on their number, so a
/// chain of composites that each use the next one twice would take 2^31 steps. The layout is
/// the public OpenType `glyf` and `loca` tables.
struct Composites<'a> {
    glyf: &'a [u8],
    loca: &'a [u8],
    long_loca: bool,
    /// Per glyph: its height in composite levels and its number of simple parts, or `None` when
    /// it is refused.
    done: HashMap<u16, Option<(u32, u64)>>,
    /// Glyphs being measured: meeting one again is a cycle.
    active: HashSet<u16>,
}

/// Why a glyph cannot be measured.
enum Unfit {
    /// It is too deep or wide, loops, or its data is damaged.
    Refused,
    /// A measurement went deeper than [`MAX_COMPOSITE_DEPTH`]; the glyph it started from is
    /// refused, but the glyphs in between may be fine from elsewhere.
    TooDeep,
}

fn be16(data: &[u8], at: usize) -> Option<u16> {
    let end = at.checked_add(2)?;
    Some(u16::from_be_bytes(data.get(at..end)?.try_into().ok()?))
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
    let end = at.checked_add(4)?;
    Some(u32::from_be_bytes(data.get(at..end)?.try_into().ok()?))
}

impl<'a> Composites<'a> {
    /// `Ok(None)` for a font without a `glyf` table (outlines of another kind, which the
    /// library does not follow this way); `Err` when it has one but no usable `loca`.
    fn new(face: &Face<'a>) -> Result<Option<Self>, FontError> {
        let raw = face.raw_face();
        let Some(glyf) = raw.table(ttf_parser::Tag::from_bytes(b"glyf")) else {
            return Ok(None);
        };
        let loca = raw
            .table(ttf_parser::Tag::from_bytes(b"loca"))
            .ok_or(FontError)?;
        let long_loca = face.tables().head.index_to_location_format
            == ttf_parser::head::IndexToLocationFormat::Long;
        Ok(Some(Self {
            glyf,
            loca,
            long_loca,
            done: HashMap::new(),
            active: HashSet::new(),
        }))
    }

    /// The bytes of a glyph (empty for a glyph without outline), or `None` if damaged.
    fn data(&self, gid: u16) -> Option<&'a [u8]> {
        let i = usize::from(gid);
        let at = |n: usize| -> Option<usize> {
            if self.long_loca {
                usize::try_from(be32(self.loca, n.checked_mul(4)?)?).ok()
            } else {
                Some(usize::from(be16(self.loca, n.checked_mul(2)?)?).checked_mul(2)?)
            }
        };
        let (start, end) = (at(i)?, at(i.checked_add(1)?)?);
        self.glyf.get(start..end)
    }

    fn visit(&mut self, gid: u16, level: u32) -> Result<(u32, u64), Unfit> {
        if let Some(known) = self.done.get(&gid) {
            return known.ok_or(Unfit::Refused);
        }
        if level > MAX_COMPOSITE_DEPTH {
            return Err(Unfit::TooDeep);
        }
        if !self.active.insert(gid) {
            return Err(Unfit::Refused);
        }
        let measured = self.measure(gid, level);
        self.active.remove(&gid);
        match &measured {
            Ok(m) => {
                self.done.insert(gid, Some(*m));
            }
            Err(Unfit::Refused) => {
                self.done.insert(gid, None);
            }
            Err(Unfit::TooDeep) => {}
        }
        measured
    }

    fn measure(&mut self, gid: u16, level: u32) -> Result<(u32, u64), Unfit> {
        let data = self.data(gid).ok_or(Unfit::Refused)?;
        // A simple glyph, or none: one part. A composite has a negative contour count.
        if data.len() < 10 || be16(data, 0).is_none_or(|n| n < 0x8000) {
            return Ok((0, 1));
        }
        const ARGS_ARE_WORDS: u16 = 0x0001;
        const MORE_COMPONENTS: u16 = 0x0020;
        const HAVE_SCALE: u16 = 0x0008;
        const HAVE_XY_SCALE: u16 = 0x0040;
        const HAVE_2X2: u16 = 0x0080;
        let (mut at, mut height, mut parts) = (10usize, 0u32, 0u64);
        loop {
            let flags = be16(data, at).ok_or(Unfit::Refused)?;
            let part = be16(data, at.saturating_add(2)).ok_or(Unfit::Refused)?;
            let (child_height, child_parts) = self.visit(part, level.saturating_add(1))?;
            height = height.max(child_height.saturating_add(1));
            parts = parts.saturating_add(child_parts);
            if height > MAX_COMPOSITE_DEPTH || parts > MAX_COMPOSITE_PARTS {
                return Err(Unfit::Refused);
            }
            let args = if flags & ARGS_ARE_WORDS != 0 { 4 } else { 2 };
            let transform = if flags & HAVE_SCALE != 0 {
                2
            } else if flags & HAVE_XY_SCALE != 0 {
                4
            } else if flags & HAVE_2X2 != 0 {
                8
            } else {
                0
            };
            at = at
                .saturating_add(4)
                .saturating_add(args)
                .saturating_add(transform);
            if flags & MORE_COMPONENTS == 0 {
                return Ok((height, parts));
            }
        }
    }
}

/// The glyphs among `needed` that are too deep or wide to outline safely.
fn refused_composites(face: &Face, needed: &[GlyphId]) -> Result<HashSet<u16>, FontError> {
    let mut refused = HashSet::new();
    let Some(mut table) = Composites::new(face)? else {
        return Ok(refused);
    };
    for gid in needed {
        if table.visit(gid.0, 0).is_err() {
            refused.insert(gid.0);
        }
    }
    Ok(refused)
}

/// `text` drawn with the glyph outlines of the TrueType file `font`, using at most `budget`
/// flattened points. `Err` when the file cannot be used (the text is then drawn as strokes),
/// `Ok(None)` when the text's size, position or rotation is not usable.
///
/// The layout: the font's `hhea` ascender less its descender is the text's height; glyphs sit
/// side by side on their `hmtx` advances (no kerning: the monospaced fonts boards embed have
/// none); the left edge of the ink, the union of the placed glyphs' bounding boxes, is at the
/// text's x and the baseline at its y. A new line starts one `hhea` line (height plus line gap)
/// lower. Then the text is mirrored and turned about its position as a stroke text is.
/// The ink box comes from the glyphs' bounding boxes, not from their flattened points, which lie
/// inside it by at most one chord.
pub(super) fn outline(
    text: &Text,
    font: &[u8],
    budget: usize,
) -> Result<Option<Outlined>, FontError> {
    let face = Face::parse(font, 0).map_err(|_| FontError)?;
    let span = f64::from(face.ascender()) - f64::from(face.descender());
    if !positive(span) {
        return Err(FontError);
    }
    if !usable(text) {
        return Ok(None);
    }
    let scale = text.height / span;
    let line_step = (span + f64::from(face.line_gap().max(0))) * scale;
    let mut out = Outlined {
        cut: text.content.chars().count().saturating_sub(MAX_CHARS),
        ..Outlined::default()
    };
    // Where each glyph goes: its pen position in font units and its line.
    let mut placed: Vec<(GlyphId, f64, f64)> = Vec::new();
    let mut widths: Vec<f64> = Vec::new();
    let (mut pen_x, mut line) = (0.0f64, 0.0f64);
    for c in text.content.chars().take(MAX_CHARS) {
        match c {
            '\n' => {
                widths.push(pen_x);
                pen_x = 0.0;
                line += 1.0;
                continue;
            }
            '\r' => continue,
            _ => {}
        }
        let gid = face.glyph_index(c).unwrap_or_else(|| {
            out.missing = out.missing.saturating_add(1);
            GlyphId(0)
        });
        placed.push((gid, pen_x, line));
        pen_x += face.glyph_hor_advance(gid).map_or(0.0, f64::from);
    }
    widths.push(pen_x);
    // A text anchored by its left end keeps the ink's left edge at x (above); one anchored by its
    // middle or right end is placed by the advance width of each line, from the first glyph's
    // pen position. The vertical anchors use the font's own box: the ascender line is its top and
    // the descender line its bottom (a measurement of KiCad's own placement of outline fonts is
    // not made).
    let ascent = f64::from(face.ascender()) * scale;
    let descent = f64::from(face.descender()) * scale;
    let below_first = widths.len().saturating_sub(1) as f64 * line_step;
    let rise = match text.anchor.vertical {
        VerticalAnchor::Baseline => 0.0,
        VerticalAnchor::Top => -ascent,
        VerticalAnchor::Center => (text.height + below_first) / 2.0 - ascent,
        VerticalAnchor::Bottom => -descent + below_first,
    };
    // The composites are measured before the library outlines anything: its bounding box of a
    // glyph is made by outlining it.
    let needed: Vec<GlyphId> = placed.iter().map(|p| p.0).collect();
    let refused = refused_composites(&face, &needed)?;
    let mut boxes: HashMap<u16, Option<ttf_parser::Rect>> = HashMap::new();
    let mut ink = |gid: GlyphId| {
        if refused.contains(&gid.0) {
            return None;
        }
        *boxes
            .entry(gid.0)
            .or_insert_with(|| face.glyph_bounding_box(gid))
    };
    let ink_left = placed
        .iter()
        .filter_map(|&(gid, x, _)| ink(gid).map(|b| x + f64::from(b.x_min)))
        .fold(f64::INFINITY, f64::min);
    let ink_left = if ink_left.is_finite() { ink_left } else { 0.0 };
    let (sin, cos) = text.rotation_deg.to_radians().sin_cos();
    let mirror = if text.mirrored { -1.0 } else { 1.0 };
    let place = |c: Coord| {
        let x = c.x * mirror;
        Coord {
            x: text.at.x + x * cos - c.y * sin,
            y: text.at.y + x * sin + c.y * cos,
        }
    };
    let mut left = budget;
    let mut exhausted = false;
    for (gid, x, line) in placed {
        if exhausted {
            if refused.contains(&gid.0) || ink(gid).is_some() {
                out.over_budget = out.over_budget.saturating_add(1);
            }
            continue;
        }
        // Every glyph with ink costs at least one point, whether it is drawn or not.
        if refused.contains(&gid.0) {
            out.failed = out.failed.saturating_add(1);
            out.points = out.points.saturating_add(1);
            left = left.saturating_sub(1);
            continue;
        }
        let mut flat = Flatten {
            tolerance: curve_tolerance(text.height),
            scale,
            shift_x: match text.anchor.horizontal {
                HorizontalAnchor::Left => x - ink_left,
                h => x - widths.get(line as usize).copied().unwrap_or(0.0) * horizontal_share(h),
            },
            shift_y: rise - line * line_step,
            rings: Vec::new(),
            ring: Vec::new(),
            last: Coord { x: 0.0, y: 0.0 },
            points: 0,
            over: false,
        };
        if face.outline_glyph(gid, &mut flat).is_none() {
            if ink(gid).is_some() {
                out.failed = out.failed.saturating_add(1);
                out.points = out.points.saturating_add(1);
                left = left.saturating_sub(1);
            }
            continue;
        }
        flat.finish();
        let cost = flat.points.max(1);
        out.points = out.points.saturating_add(cost);
        if flat.over {
            // Past its own cap: stop the text, a glyph like it costs too much to look for more.
            out.failed = out.failed.saturating_add(1);
            exhausted = true;
            continue;
        }
        if cost > left {
            exhausted = true;
            left = 0;
            out.over_budget = out.over_budget.saturating_add(1);
            continue;
        }
        left = left.saturating_sub(cost);
        match glyph_areas(&flat.rings, &place) {
            Some(areas) => out.areas.extend(areas),
            None => out.failed = out.failed.saturating_add(1),
        }
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(content: &str, rotation_deg: f64, mirrored: bool) -> Text {
        Text {
            layer: 0,
            at: Point { x: 10.0, y: 10.0 },
            content: content.into(),
            height: 2.1,
            rotation_deg,
            mirrored,
            anchor: board_model::TextAnchor::default(),
            line_pitch: None,
            font: TextFont::Stroke {
                width: 0.2,
                font: board_model::StrokeFont::Default,
            },
            component: None,
        }
    }

    fn bounds(d: &Drawn) -> (f64, f64, f64, f64) {
        let pts = d.strokes.iter().flatten();
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in pts {
            (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
        }
        (x0, y0, x1, y1)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn the_font_holds_ascii_32_to_127() {
        let glyphs = font(StrokeFont::SansSerif);
        assert_eq!(glyphs.len(), 96);
        assert!(glyphs[0].strokes.is_empty(), "a space draws nothing");
        // 'H' (72) is "9G]KFK[ RYFY[ RKPYP": two uprights and a bar, so three strokes.
        assert_eq!(glyphs[72 - 32].strokes.len(), 3);
    }

    fn text_in(content: &str, which: StrokeFont, height: f64) -> Text {
        let mut t = text(content, 0.0, false);
        t.height = height;
        t.font = TextFont::Stroke {
            width: height / 10.0,
            font: which,
        };
        t
    }

    fn near(a: f64, b: f64, by: f64) -> bool {
        (a - b).abs() < by
    }

    #[test]
    fn the_times_font_holds_ascii_32_to_127_and_stands_as_high_as_simplex() {
        let times = font(StrokeFont::Serif);
        assert!(times.len() >= 95);
        assert!(times[0].strokes.is_empty());
        let (a, b) = (
            draw(&text_in("H", StrokeFont::Serif, 2.1)).unwrap(),
            draw(&text_in("H", StrokeFont::SansSerif, 2.1)).unwrap(),
        );
        let (ay, by) = (bounds(&a), bounds(&b));
        assert!(close(ay.1, by.1) && close(ay.3, by.3), "{ay:?} {by:?}");
        assert_ne!(a.strokes, b.strokes, "serif letters are not the sans ones");
    }

    #[test]
    fn curves_follow_a_four_hundredth_of_the_height_but_never_stray_past_the_chord() {
        assert!(near(curve_tolerance(1.2), 0.003, 1e-12));
        assert!(near(curve_tolerance(2.0), 0.005, 1e-12));
        // From 8 mm up the share is past the 0.02 mm chord, which stays the limit.
        assert!(near(curve_tolerance(8.0), super::super::CHORD, 1e-12));
        assert!(near(curve_tolerance(500.0), super::super::CHORD, 1e-12));
    }

    #[test]
    fn a_quarter_circle_is_flattened_within_the_tolerance() {
        // A quadratic through (1,0), (1,1), (0,1) stands for a quarter of a curve: its midpoint
        // (0.75, 0.75) is the farthest from the chord; the flattened points must all lie within
        // the tolerance of the curve, whose points are ((1-t)^2 + 2(1-t)t, 2(1-t)t + t^2).
        let tolerance = 0.003;
        let mut f = Flatten {
            tolerance,
            scale: 1.0,
            shift_x: 0.0,
            shift_y: 0.0,
            rings: Vec::new(),
            ring: Vec::new(),
            last: Coord { x: 1.0, y: 0.0 },
            points: 0,
            over: false,
        };
        f.quad_to(1.0, 1.0, 0.0, 1.0);
        // Each flattened segment's midpoint lies within the tolerance of the true curve.
        let curve = |t: f64| {
            let u = 1.0 - t;
            Coord {
                x: u * u + 2.0 * u * t,
                y: 2.0 * u * t + t * t,
            }
        };
        let dense: Vec<Coord> = (0..=20_000)
            .map(|i| curve(f64::from(i) / 20_000.0))
            .collect();
        for w in f.ring.windows(2) {
            let m = Coord {
                x: (w[0].x + w[1].x) / 2.0,
                y: (w[0].y + w[1].y) / 2.0,
            };
            let d = dense
                .iter()
                .map(|c| (c.x - m.x).hypot(c.y - m.y))
                .fold(f64::INFINITY, f64::min);
            assert!(d <= tolerance, "{d}");
        }
        assert!(
            f.ring.len() > 8,
            "a coarser tolerance would need fewer points"
        );
    }

    #[test]
    fn sans_serif_advances_by_the_glyphs_width_times_its_fitted_factor() {
        // 'H' is 22 units wide in Roman Simplex (-11 to 11); at 0.1 mm per unit "HH" starts its
        // second H 2.2 mm times the capitals' factor after the first.
        let one = bounds(&draw(&text_in("H", StrokeFont::SansSerif, 2.1)).unwrap());
        let two = bounds(&draw(&text_in("HH", StrokeFont::SansSerif, 2.1)).unwrap());
        assert!(
            near(two.2 - one.2, 2.2 * SANS_SERIF_TIGHTNESS.upper, 1e-9),
            "{one:?} {two:?}"
        );
    }

    #[test]
    fn default_advance_matches_the_measured_heights() {
        // README.txt of the Altium test boards: "TEST" (one-text) ends at x 17.53097 from a
        // position of 10, "ROT" (one-rotated-text, turned up) at y 20.53162 from 15; both end in
        // the same letter, so the extra character is one advance: 1.99935 at height 2.0.
        let at_2 = (17.53097 - 10.0) - (20.53162 - 15.0);
        assert!(near(default_advance(2.0), at_2, 1e-9));
        // special-strings: "L: Top Overlay" ends at 22.144, "Top Overlay" at 17.895, both from
        // x = 2: three more characters, at height 1.5.
        assert!(near(default_advance(1.5), (22.144 - 17.895) / 3.0, 1e-5));
        // Between and beyond the measured heights it keeps rising.
        assert!(default_advance(1.8) > default_advance(1.5));
        assert!(default_advance(5.0) > default_advance(2.0));
        assert!(default_advance(0.8) < default_advance(1.2));
    }

    /// The text of the stroke-fonts board's nine lines (README.txt) at height 1.2, stroke 0.15,
    /// from x 2.0, with the right end of each line's rectangle: Altium's rectangle reaches one
    /// stroke width past the ink (the pen's centerline, which is what we draw) on each side.
    const STROKE_FONT_LINES: [(StrokeFont, &str, f64); 9] = [
        (StrokeFont::Default, "ABCDEFGHIJKLMNOPQRSTUVWXYZ", 32.740),
        (StrokeFont::Default, "abcdefghijklmnopqrstuvwxyz", 32.140),
        (StrokeFont::Default, "0123456789 .,:;-+=/()[]#%&*?!", 34.340),
        (StrokeFont::SansSerif, "ABCDEFGHIJKLMNOPQRSTUVWXYZ", 29.218),
        (StrokeFont::SansSerif, "abcdefghijklmnopqrstuvwxyz", 24.711),
        (
            StrokeFont::SansSerif,
            "0123456789 .,:;-+=/()[]#%&*?!",
            28.030,
        ),
        (StrokeFont::Serif, "ABCDEFGHIJKLMNOPQRSTUVWXYZ", 31.075),
        (StrokeFont::Serif, "abcdefghijklmnopqrstuvwxyz", 28.133),
        (StrokeFont::Serif, "0123456789 .,:;-+=/()[]#%&*?!", 26.790),
    ];

    fn stroke_fonts_line(font: StrokeFont, content: &str) -> Drawn {
        let mut t = text_in(content, font, 1.2);
        t.at = Point { x: 2.0, y: 10.0 };
        t.font = TextFont::Stroke { width: 0.15, font };
        draw(&t).unwrap()
    }

    #[test]
    fn sans_serif_and_serif_lines_end_where_the_stroke_fonts_board_says() {
        // The factors are fitted to these very lines, so only the rounding of the factors (four
        // digits) and of the README's numbers (0.001) is left: 0.01 mm.
        for (font, content, right) in &STROKE_FONT_LINES[3..] {
            let ink = bounds(&stroke_fonts_line(*font, content)).2;
            assert!(near(ink + 0.15, *right, 0.01), "{content} {ink} {right}");
        }
    }

    #[test]
    fn default_lines_end_within_three_percent_of_the_stroke_fonts_board() {
        // One advance fits all three lines, and our letter shapes end differently from
        // Altium's (the digit line also holds "[]", one box in Altium), so the three lines come
        // out up to 2.3 % apart in width, 0.7 mm of 30.
        for (font, content, right) in &STROKE_FONT_LINES[..3] {
            let ink = bounds(&stroke_fonts_line(*font, content)).2;
            let (ours, theirs) = (ink + 0.15 - 1.85, right - 1.85);
            assert!(
                (ours / theirs - 1.0).abs() < 0.03,
                "{content} {ours} {theirs}"
            );
        }
    }

    #[test]
    fn default_text_runs_on_a_fixed_advance() {
        for h in [1.5, 2.0, 5.0] {
            let advance = default_advance(h);
            // The same letter again moves by exactly one advance, whatever the letter.
            for c in ["T", "i", "W", "."] {
                let one = bounds(&draw(&text_in(c, StrokeFont::Default, h)).unwrap());
                let two = bounds(&draw(&text_in(&c.repeat(2), StrokeFont::Default, h)).unwrap());
                assert!(near(two.2 - one.2, advance, 1e-9), "{c} {h}");
                // And its ink stays inside its own cell.
                assert!(
                    one.0 >= 10.0 - 1e-9 && one.2 <= 10.0 + advance + 1e-9,
                    "{c} {one:?}"
                );
            }
        }
    }

    #[test]
    fn default_text_has_the_width_of_its_character_count() {
        // The right end of a text of n characters ending in the same letter grows by one advance
        // per character, so "TEST" and "ROT" differ by one (see the README rectangles).
        let end = |s: &str| bounds(&draw(&text_in(s, StrokeFont::Default, 2.0)).unwrap()).2;
        assert!(near(end("TEST") - end("ROT"), 1.99935, 1e-9));
    }

    #[test]
    fn a_capital_stands_on_the_baseline_and_is_the_text_height() {
        // 'H' in Roman Simplex runs from y -12 to 9: with a height of 2.1 (0.1 mm per unit) it
        // stands from the baseline at y 10 up to 12.1.
        let d = draw(&text("H", 0.0, false)).unwrap();
        let (x0, y0, _, y1) = bounds(&d);
        assert!(close(y0, 10.0) && close(y1, 12.1), "{y0} {y1}");
        assert!(
            x0 > 10.0,
            "the left bearing keeps the stroke right of the position"
        );
        assert_eq!((d.pen, d.missing, d.cut), (0.2, 0, 0));
    }

    #[test]
    fn a_turned_text_runs_up_and_a_mirrored_one_runs_left() {
        let (_, _, x1, _) = bounds(&draw(&text("HH", 0.0, false)).unwrap());
        let up = bounds(&draw(&text("HH", 90.0, false)).unwrap());
        // Turned 90 degrees counter-clockwise: the text runs up from the position, and the
        // capitals stand to its left.
        assert!(close(up.3 - 10.0, x1 - 10.0), "{up:?}");
        assert!(close(up.0, 10.0 - 2.1), "{up:?}");
        let left = bounds(&draw(&text("HH", 0.0, true)).unwrap());
        assert!(close(10.0 - left.0, x1 - 10.0), "{left:?}");
    }

    #[test]
    fn unknown_characters_become_question_marks_and_long_texts_are_cut() {
        let d = draw(&text("Ω1", 0.0, false)).unwrap();
        assert_eq!(d.missing, 1);
        let long = "A".repeat(MAX_CHARS + 5);
        assert_eq!(draw(&text(&long, 0.0, false)).unwrap().cut, 5);
    }

    #[test]
    fn a_mirrored_text_turned_90_degrees_runs_down_with_its_capitals_to_the_left() {
        // Local coordinates (x along the text, y up from the baseline) of "HH" are, from the
        // unrotated bounds, x in [x0 - 10, x1 - 10] and y in [0, 2.1]. Mirroring negates x. A
        // turn of 90 degrees counter-clockwise maps (x, y) to (-y, x), so a point lands at
        // X = 10 - y and Y = 10 + (-x) = 10 - x. Hence X runs over [10 - 2.1, 10] and Y over
        // [10 - (x1 - 10), 10 - (x0 - 10)] = [20 - x1, 20 - x0].
        let (x0, _, x1, _) = bounds(&draw(&text("HH", 0.0, false)).unwrap());
        let got = bounds(&draw(&text("HH", 90.0, true)).unwrap());
        let want = (10.0 - 2.1, 20.0 - x1, 10.0, 20.0 - x0);
        assert!(
            close(got.0, want.0)
                && close(got.1, want.1)
                && close(got.2, want.2)
                && close(got.3, want.3),
            "{got:?} {want:?}"
        );
    }

    #[test]
    fn a_second_line_sits_below_the_first() {
        let (_, y0, _, _) = bounds(&draw(&text("H\nH", 0.0, false)).unwrap());
        assert!(close(y0, 10.0 - 32.0 * 0.1), "{y0}");
    }

    #[test]
    fn unusable_texts_are_none() {
        let mut t = text("H", 0.0, false);
        t.height = 0.0;
        assert!(draw(&t).is_none());
        let mut t = text("H", f64::NAN, false);
        t.rotation_deg = f64::NAN;
        assert!(draw(&t).is_none());
    }

    #[test]
    fn a_huge_height_or_huge_coordinates_are_none() {
        let mut t = text("H", 0.0, false);
        t.height = 1.0e6;
        assert!(draw(&t).is_none());
        let mut t = text("H", 0.0, false);
        t.at.x = 1.0e9;
        assert!(draw(&t).is_none());
        let mut t = text("H", 0.0, false);
        t.at.y = -1.0e9;
        assert!(draw(&t).is_none());
        let mut t = text("H", 0.0, false);
        t.height = MAX_HEIGHT;
        assert!(draw(&t).is_some());
    }

    #[test]
    fn a_truetype_text_gets_a_pen_a_tenth_of_its_height() {
        let mut t = text("H", 0.0, false);
        t.font = TextFont::TrueType {
            name: "Arial".into(),
            bold: false,
            italic: false,
        };
        assert!(close(draw(&t).unwrap().pen, 0.21));
    }

    // ---- outlines ----

    fn be(out: &mut Vec<u8>, v: u32, bytes: usize) {
        out.extend_from_slice(v.to_be_bytes().get(4 - bytes..).unwrap());
    }

    /// A 100 x 100 square as a simple glyph.
    fn square_glyph() -> Vec<u8> {
        let mut g = Vec::new();
        for v in [1u32, 0, 0, 100, 100] {
            be(&mut g, v, 2); // contours, then the box
        }
        be(&mut g, 3, 2); // last point of the one contour
        be(&mut g, 0, 2); // no instructions
        g.extend_from_slice(&[1, 1, 1, 1]); // on-curve, 16-bit deltas
        for dx in [0i32, 100, 0, -100] {
            g.extend_from_slice(&(dx as i16).to_be_bytes());
        }
        for dy in [0i32, 0, 100, 0] {
            g.extend_from_slice(&(dy as i16).to_be_bytes());
        }
        g
    }

    /// A composite glyph made of `parts` uses of glyph `child`.
    fn composite_glyph(child: u16, parts: usize) -> Vec<u8> {
        let mut g = Vec::new();
        be(&mut g, 0xffff, 2);
        g.extend_from_slice(&[0; 8]);
        for i in 0..parts {
            let more = if i + 1 < parts { 0x20 } else { 0 };
            be(&mut g, 0x02 | more, 2);
            be(&mut g, u32::from(child), 2);
            g.extend_from_slice(&[0, 0]);
        }
        g
    }

    /// A font by hand: glyph 0 (which every character maps to, as there is no `cmap`) is the
    /// first of `glyphs`.
    fn font_of(glyphs: &[Vec<u8>]) -> Vec<u8> {
        font_with(glyphs, false)
    }

    /// [`font_of`] with a short `loca` (offsets halved, 16 bits) when `short`.
    fn font_with(glyphs: &[Vec<u8>], short: bool) -> Vec<u8> {
        let n = glyphs.len() as u32;
        let (mut glyf, mut loca) = (Vec::new(), Vec::new());
        let offset = |loca: &mut Vec<u8>, at: usize| {
            if short {
                be(loca, (at / 2) as u32, 2);
            } else {
                be(loca, at as u32, 4);
            }
        };
        for g in glyphs {
            offset(&mut loca, glyf.len());
            glyf.extend_from_slice(g);
            while glyf.len() % 4 != 0 {
                glyf.push(0);
            }
        }
        offset(&mut loca, glyf.len());
        let mut head = vec![0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        be(&mut head, 0x5f0f3cf5, 4);
        be(&mut head, 0, 2);
        be(&mut head, 1000, 2); // units per em
        head.extend_from_slice(&[0; 16]);
        head.extend_from_slice(&[0; 8]);
        head.extend_from_slice(&[0; 6]); // mac style .. direction hint
        be(&mut head, u32::from(!short), 2); // 1: long loca
        be(&mut head, 0, 2);
        let mut hhea = vec![0, 1, 0, 0];
        for v in [800u32, 0xff38, 0, 1000] {
            be(&mut hhea, v, 2); // ascender 800, descender -200
        }
        hhea.extend_from_slice(&[0; 22]);
        be(&mut hhea, n, 2);
        let mut maxp = vec![0, 1, 0, 0];
        be(&mut maxp, n, 2);
        let mut hmtx = Vec::new();
        for _ in 0..n {
            be(&mut hmtx, 500, 2);
            be(&mut hmtx, 0, 2);
        }
        let tables = [
            (b"glyf", glyf),
            (b"head", head),
            (b"hhea", hhea),
            (b"hmtx", hmtx),
            (b"loca", loca),
            (b"maxp", maxp),
        ];
        let mut file = vec![0, 1, 0, 0];
        be(&mut file, tables.len() as u32, 2);
        file.extend_from_slice(&[0; 6]);
        let mut at = 12 + 16 * tables.len();
        for (tag, data) in &tables {
            file.extend_from_slice(*tag);
            be(&mut file, 0, 4);
            be(&mut file, at as u32, 4);
            be(&mut file, data.len() as u32, 4);
            at += data.len().div_ceil(4) * 4;
        }
        for (_, data) in &tables {
            file.extend_from_slice(data);
            while file.len() % 4 != 0 {
                file.push(0);
            }
        }
        file
    }

    /// Glyph 0 uses glyph 1 twice, glyph 1 glyph 2 twice, and so on down to a square.
    fn chain_font(levels: usize) -> Vec<u8> {
        let mut glyphs: Vec<Vec<u8>> = (0..levels)
            .map(|i| composite_glyph(u16::try_from(i + 1).unwrap(), 2))
            .collect();
        glyphs.push(square_glyph());
        font_of(&glyphs)
    }

    fn text_in_font() -> Text {
        let mut t = text("A", 0.0, false);
        t.font = TextFont::TrueType {
            name: "Test".into(),
            bold: false,
            italic: false,
        };
        t
    }

    #[test]
    fn a_hand_made_font_draws_a_square() {
        let o = outline(&text_in_font(), &font_of(&[square_glyph()]), 1_000)
            .ok()
            .unwrap()
            .unwrap();
        // 100 units of 1000 per em, with a text height of 2.1 over the 1000 unit span.
        assert_eq!((o.areas.len(), o.failed, o.missing), (1, 0, 1));
        let w = o.areas[0]
            .exterior()
            .0
            .iter()
            .map(|c| c.x)
            .fold(0.0, f64::max);
        assert!(close(w - 10.0, 0.21), "{w}");
    }

    #[test]
    fn a_shallow_composite_is_drawn() {
        // Four levels of two parts: 16 squares, 4 deep.
        let o = outline(&text_in_font(), &chain_font(4), 1_000)
            .ok()
            .unwrap()
            .unwrap();
        assert_eq!((o.areas.len(), o.failed), (1, 0));
    }

    #[test]
    fn composites_too_wide_or_too_deep_are_refused_before_they_are_outlined() {
        // Seven levels of two parts: 128 squares, over the 64 allowed.
        let wide = outline(&text_in_font(), &chain_font(7), 1_000);
        let wide = wide.ok().unwrap().unwrap();
        assert_eq!((wide.areas.len(), wide.failed), (0, 1));
        // 31 levels of two parts would take 2^31 steps if outlined; it returns at once.
        let deep = outline(&text_in_font(), &chain_font(31), 1_000);
        let deep = deep.ok().unwrap().unwrap();
        assert_eq!((deep.areas.len(), deep.failed), (0, 1));
        // A refused glyph still costs a point.
        assert_eq!(deep.points, 1);
    }

    #[test]
    fn a_composite_that_uses_itself_is_refused() {
        let glyphs = [composite_glyph(0, 1), square_glyph()];
        let o = outline(&text_in_font(), &font_of(&glyphs), 1_000)
            .ok()
            .unwrap()
            .unwrap();
        assert_eq!((o.areas.len(), o.failed), (0, 1));
    }

    #[test]
    fn a_glyph_table_without_a_location_table_is_unusable() {
        let mut font = font_of(&[square_glyph()]);
        // Rename the `loca` table's tag.
        let at = font.windows(4).position(|w| w == b"loca").unwrap();
        font[at..at + 4].copy_from_slice(b"lopa");
        assert!(outline(&text_in_font(), &font, 1_000).is_err());
    }

    fn ring(points: &[(f64, f64)]) -> Vec<Coord> {
        points.iter().map(|&(x, y)| Coord { x, y }).collect()
    }

    #[test]
    fn two_separate_rings_of_opposite_winding_are_both_filled() {
        let ccw = ring(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]);
        let cw = ring(&[(5.0, 0.0), (5.0, 1.0), (6.0, 1.0), (6.0, 0.0)]);
        let areas = glyph_areas(&[ccw, cw], &|c| c).unwrap();
        assert_eq!(areas.len(), 2, "{areas:?}");
        assert!(areas.iter().all(|a| a.interiors().is_empty()));
    }

    #[test]
    fn a_ring_of_opposite_winding_inside_another_is_a_hole() {
        let outer = ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        let inner = ring(&[(1.0, 1.0), (1.0, 3.0), (3.0, 3.0), (3.0, 1.0)]);
        let areas = glyph_areas(&[outer.clone(), inner], &|c| c).unwrap();
        assert_eq!((areas.len(), areas[0].interiors().len()), (1, 1));
        // The same winding inside is no hole.
        let same = ring(&[(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)]);
        let areas = glyph_areas(&[outer, same], &|c| c).unwrap();
        assert_eq!((areas.len(), areas[0].interiors().len()), (1, 0));
    }

    #[test]
    fn a_glyph_with_too_many_points_is_not_united() {
        let big: Vec<Coord> = (0..=MAX_UNION_POINTS)
            .map(|i| Coord {
                x: i as f64,
                y: (i % 7) as f64,
            })
            .collect();
        assert!(glyph_areas(&[big], &|c| c).is_none());
    }

    /// A composite glyph from records of (flags, part, bytes after the part number).
    fn records_glyph(records: &[(u16, u16, usize)]) -> Vec<u8> {
        let mut g = Vec::new();
        be(&mut g, 0xffff, 2);
        g.extend_from_slice(&[0; 8]);
        for (i, &(flags, part, tail)) in records.iter().enumerate() {
            let more = if i + 1 < records.len() { 0x20 } else { 0 };
            be(&mut g, u32::from(flags | more), 2);
            be(&mut g, u32::from(part), 2);
            g.extend(std::iter::repeat_n(0u8, tail));
        }
        g
    }

    fn draws_one_square(font: &[u8]) -> bool {
        let o = outline(&text_in_font(), font, 1_000).ok().unwrap().unwrap();
        (o.areas.len(), o.failed) == (1, 0)
    }

    fn refuses(font: &[u8]) -> bool {
        let o = outline(&text_in_font(), font, 1_000).ok().unwrap().unwrap();
        (o.areas.len(), o.failed) == (0, 1)
    }

    fn measure(font: &[u8], gid: u16) -> Option<(u32, u64)> {
        let face = Face::parse(font, 0).ok().unwrap();
        let mut table = Composites::new(&face).ok().unwrap().unwrap();
        table.visit(gid, 0).ok()
    }

    #[test]
    fn short_location_tables_are_read() {
        let mut glyphs: Vec<Vec<u8>> = (0..4)
            .map(|i| composite_glyph(u16::try_from(i + 1).unwrap(), 2))
            .collect();
        glyphs.push(square_glyph());
        let font = font_with(&glyphs, true);
        assert!(draws_one_square(&font));
        assert_eq!(measure(&font, 0), Some((4, 16)));
    }

    #[test]
    fn every_kind_of_component_record_is_skipped_by_its_own_length() {
        // (flags, bytes after the part number): 2-byte arguments; 4-byte (words); a scale;
        // an x and y scale; a two by two. The second record, found only if the first is
        // skipped by exactly its length, uses glyph 1; a misread one would read zero flags
        // and part 0, the glyph itself.
        for (flags, tail) in [
            (0x0002u16, 2usize),
            (0x0003, 4),
            (0x000a, 4),
            (0x0042, 6),
            (0x0082, 10),
        ] {
            let glyphs = [
                records_glyph(&[(flags, 1, tail), (0x0002, 1, 2)]),
                square_glyph(),
            ];
            let font = font_of(&glyphs);
            assert!(draws_one_square(&font), "flags {flags:#06x}");
            assert_eq!(measure(&font, 0), Some((1, 2)), "flags {flags:#06x}");
        }
    }

    #[test]
    fn a_part_used_twice_through_two_paths_is_counted_each_time() {
        // 0 uses 1 and 2; both use 3, a square: a diamond of two parts, two levels deep.
        let glyphs = [
            records_glyph(&[(0x0002, 1, 2), (0x0002, 2, 2)]),
            records_glyph(&[(0x0002, 3, 2)]),
            records_glyph(&[(0x0002, 3, 2)]),
            square_glyph(),
        ];
        let font = font_of(&glyphs);
        assert_eq!(measure(&font, 0), Some((2, 2)));
        assert!(draws_one_square(&font));
    }

    #[test]
    fn a_part_that_does_not_exist_is_refused() {
        let font = font_of(&[records_glyph(&[(0x0002, 99, 2)]), square_glyph()]);
        assert!(refuses(&font));
        // A record cut short by the end of the glyph.
        let mut cut = records_glyph(&[(0x0002, 1, 2)]);
        cut.truncate(cut.len() - 3);
        assert!(refuses(&font_of(&[cut, square_glyph()])));
    }

    #[test]
    fn nine_levels_of_composites_are_too_deep_and_eight_are_not() {
        let chain = |levels: usize| {
            let mut glyphs: Vec<Vec<u8>> = (0..levels)
                .map(|i| composite_glyph(u16::try_from(i + 1).unwrap(), 1))
                .collect();
            glyphs.push(square_glyph());
            font_of(&glyphs)
        };
        assert!(draws_one_square(&chain(8)));
        assert_eq!(measure(&chain(8), 0), Some((8, 1)));
        assert!(refuses(&chain(9)));
        assert_eq!(measure(&chain(9), 0), None);
    }
}

/// Where KiCad's own Gerber export puts the ink of the texts on the `text-justify` board, against
/// where we draw them: the justification (and its mirroring and turning) is resolved from our
/// glyph advances, so the anchor-relevant edge of each text must land where KiCad's does.
#[cfg(all(test, feature = "kicad"))]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod kicad_gerber {
    use super::*;
    use board_model::InputFormat;

    /// A box of pen center lines `[min_x, min_y, max_x, max_y]` and the pen width.
    type Ink = ([f64; 4], f64);

    fn dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../kicad-pcb/tests")
    }

    /// The texts' boxes `gerber_text_extents.py` read from KiCad's export of one silkscreen.
    fn gerber(side: &str) -> Vec<Ink> {
        let path = dir().join(format!("gerber/text-justify-{side}_Silkscreen.extents.txt"));
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|l| {
                let v: Vec<f64> = l.split_whitespace().map(|n| n.parse().unwrap()).collect();
                ([v[0], v[1], v[2], v[3]], v[4])
            })
            .collect()
    }

    /// The box of the center lines of everything we draw of `t`.
    fn ours(t: &Text) -> [f64; 4] {
        let d = draw(t).unwrap();
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for p in d.strokes.iter().flatten() {
            b = [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)];
        }
        b
    }

    /// The distance from `p` to the box `b`, zero inside it.
    fn gap(p: Point, b: &[f64; 4]) -> f64 {
        (b[0] - p.x)
            .max(p.x - b[2])
            .max(0.0)
            .hypot((b[1] - p.y).max(p.y - b[3]).max(0.0))
    }

    /// Ink edges are compared at the pen's center line (both sides draw the same pen). Where we
    /// differ from KiCad: its letter shapes (we draw Roman Simplex; the other letters of a text
    /// can be wider or narrower), so the test text is "HIH", whose outer edges are the Hs'
    /// outer strokes, and only the edges the anchor decides are compared: the left or right
    /// edge (the middle) of a text anchored by its left or right end (middle), and the top or
    /// bottom edge (the middle) of one anchored at the top or bottom (middle). A turned text's
    /// box edges all follow from the corners of its first and last H, so all four are compared.
    ///
    /// The tolerance, 0.02 mm, is the largest residual of the constants fitted to this board
    /// (0.015 mm, vertically at 3 mm with a 0.15 mm pen, where the pen's small share in the
    /// text box is not modelled), plus the Gerber's own rounding (1 nm). Every turned text lands
    /// within 0.003 mm of KiCad's; every horizontal edge within 0.001 mm.
    const TOLERANCE: f64 = 0.02;

    #[test]
    fn every_justification_lands_where_kicads_own_gerber_has_it() {
        let bytes = std::fs::read(dir().join("boards/text-justify.kicad_pcb")).unwrap();
        let o = kicad_pcb::KiCad.read(&bytes).unwrap();
        let (top, bottom) = (gerber("F"), gerber("B"));
        assert_eq!(o.board.texts.len(), top.len() + bottom.len());
        let mut checked = 0;
        for t in &o.board.texts {
            let side = if o.board.layers[t.layer].name == "B.SilkS" {
                &bottom
            } else {
                &top
            };
            let (theirs, _) = side
                .iter()
                .min_by(|a, b| gap(t.at, &a.0).total_cmp(&gap(t.at, &b.0)))
                .unwrap();
            let mine = ours(t);
            // (our value, KiCad's) for each edge or middle compared.
            let [l, b, r, tp] = [0, 1, 2, 3];
            let mid = |v: &[f64; 4], i: usize, j: usize| (v[i] + v[j]) / 2.0;
            let mut pairs: Vec<(&str, f64, f64)> = Vec::new();
            if t.rotation_deg == 0.0 {
                // A mirrored text's left end is on the board's right.
                let (start, end) = if t.mirrored { (r, l) } else { (l, r) };
                pairs.push(match t.anchor.horizontal {
                    HorizontalAnchor::Left => ("start", mine[start], theirs[start]),
                    HorizontalAnchor::Center => ("middle", mid(&mine, l, r), mid(theirs, l, r)),
                    HorizontalAnchor::Right => ("end", mine[end], theirs[end]),
                });
                pairs.push(match t.anchor.vertical {
                    VerticalAnchor::Top => ("top", mine[tp], theirs[tp]),
                    VerticalAnchor::Bottom => ("bottom", mine[b], theirs[b]),
                    _ => ("middle", mid(&mine, b, tp), mid(theirs, b, tp)),
                });
            } else {
                for (name, i) in [("left", l), ("bottom", b), ("right", r), ("top", tp)] {
                    pairs.push((name, mine[i], theirs[i]));
                }
            }
            for (name, m, k) in pairs {
                assert!(
                    (m - k).abs() <= TOLERANCE,
                    "{:?} turned {} mirrored {} height {}: {name} edge {m:.3} against KiCad's {k:.3}",
                    t.anchor,
                    t.rotation_deg,
                    t.mirrored,
                    t.height
                );
                checked += 1;
            }
        }
        assert!(checked >= 2 * (top.len() + bottom.len()), "{checked}");
    }
}
