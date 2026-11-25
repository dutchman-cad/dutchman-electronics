//! Text as pen strokes, in the Hershey Roman Simplex font (`fonts/hershey/rowmans.jhf`, free to
//! use with the acknowledgement in `fonts/hershey/README.txt`).
//!
//! A stroke text is drawn in this font at the text's height and pen width, and so is a TrueType
//! text whose font file the board does not embed (or that does not parse). Letters and spacing
//! then differ a little from the CAD program's, which the package says in a Degraded note.
//!
//! A TrueType text whose font file is embedded is drawn with the real letter outlines instead
//! ([`outline`]): filled areas, no pen.
//!
//! A Hershey glyph is a list of pen strokes in font units, y down, with the baseline at 9 and
//! the top of a capital at -12. The text's height is taken as the height of a capital, and its
//! position as the left end of its baseline, before it is mirrored and turned.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::OnceLock;

use board_model::{Point, Text, TextFont};
use geo::orient::{Direction, Orient};
use geo::{BooleanOps, Coord, MultiPolygon, Polygon};
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

fn font() -> &'static [Glyph] {
    static FONT: OnceLock<Vec<Glyph>> = OnceLock::new();
    FONT.get_or_init(|| parse(include_str!("../../fonts/hershey/rowmans.jhf")))
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
    let pen = match &text.font {
        TextFont::Stroke { width } => *width,
        TextFont::TrueType { .. } => text.height * TRUETYPE_PEN,
    };
    if !usable(text) || !positive(pen) {
        return None;
    }
    let glyphs = font();
    let question = glyphs.get(QUESTION)?;
    let scale = text.height / CAP_UNITS;
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
    let (mut pen_x, mut line) = (0.0f64, 0.0f64);
    for c in text.content.chars().take(MAX_CHARS) {
        match c {
            '\n' => {
                pen_x = 0.0;
                line += 1.0;
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
        for s in &glyph.strokes {
            drawn.strokes.push(
                s.iter()
                    .map(|&(x, y)| {
                        place(
                            (pen_x + x - glyph.left) * scale,
                            (BASELINE - y - line * LINE_UNITS) * scale,
                        )
                    })
                    .collect(),
            );
        }
        pen_x += glyph.right - glyph.left;
    }
    Some(drawn)
}

/// The most points one glyph may flatten to; a glyph with more is not drawn.
const MAX_GLYPH_POINTS: usize = 20_000;
/// The most contours one glyph may have; a glyph with more is not drawn.
const MAX_GLYPH_CONTOURS: usize = 64;
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

/// Flattens a glyph outline into rings of points (in mm, y up), within [`super::CHORD`].
struct Flatten {
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
    /// within the chord: the error of `n` equal pieces is `factor * d / n^2`.
    fn steps(d: f64, factor: f64) -> usize {
        let n = (factor * d / super::CHORD).sqrt().ceil();
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
        let n = Self::steps(d, 0.25);
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
        let n = Self::steps(a.max(b), 0.75);
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

/// One glyph's rings as filled areas. The rings of the glyph's main winding (the winding of its
/// biggest ring: clockwise in a TrueType font with y up, counter-clockwise in a CFF one) are
/// added and those of the other winding are taken away, biggest first, so that an island inside
/// a hole (the inner part of an "@") stays. `place` maps a ring point from the text's own
/// frame to the board. `None` when a point lands past the overlay limit or the cut fails.
fn glyph_areas(rings: &[Vec<Coord>], place: &dyn Fn(Coord) -> Coord) -> Option<Vec<Polygon>> {
    let mut rings: Vec<(f64, Vec<Coord>)> = rings
        .iter()
        .map(|r| (twice_area(r), r.iter().map(|&c| place(c)).collect()))
        .filter(|(a, _)| a.abs() > 1e-12)
        .collect();
    rings.sort_by(|a, b| b.0.abs().total_cmp(&a.0.abs()));
    let outer = rings.first()?.0.is_sign_negative();
    let shape = catch_unwind(AssertUnwindSafe(|| {
        let mut shape = MultiPolygon::new(Vec::new());
        for (area, ring) in rings {
            let poly = super::polygon(ring)?;
            shape = if area.is_sign_negative() == outer {
                shape.union(&poly)
            } else {
                shape.difference(&poly)
            };
        }
        Some(shape.orient(Direction::Default))
    }))
    .ok()??;
    Some(shape.0)
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
    let (mut pen_x, mut line) = (0.0f64, 0.0f64);
    let mut ink_left = f64::INFINITY;
    for c in text.content.chars().take(MAX_CHARS) {
        match c {
            '\n' => {
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
        if let Some(b) = face.glyph_bounding_box(gid) {
            ink_left = ink_left.min(pen_x + f64::from(b.x_min));
        }
        placed.push((gid, pen_x, line));
        pen_x += face.glyph_hor_advance(gid).map_or(0.0, f64::from);
    }
    if !ink_left.is_finite() {
        ink_left = 0.0;
    }
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
            if face.glyph_bounding_box(gid).is_some() {
                out.over_budget = out.over_budget.saturating_add(1);
            }
            continue;
        }
        let mut flat = Flatten {
            scale,
            shift_x: x - ink_left,
            shift_y: -line * line_step,
            rings: Vec::new(),
            ring: Vec::new(),
            last: Coord { x: 0.0, y: 0.0 },
            points: 0,
            over: false,
        };
        if face.outline_glyph(gid, &mut flat).is_none() {
            continue;
        }
        flat.finish();
        if flat.over {
            out.failed = out.failed.saturating_add(1);
            continue;
        }
        if flat.points > left {
            exhausted = true;
            out.over_budget = out.over_budget.saturating_add(1);
            continue;
        }
        left = left.saturating_sub(flat.points);
        out.points = out.points.saturating_add(flat.points);
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
            font: TextFont::Stroke { width: 0.2 },
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
        let glyphs = font();
        assert_eq!(glyphs.len(), 96);
        assert!(glyphs[0].strokes.is_empty(), "a space draws nothing");
        // 'H' (72) is "9G]KFK[ RYFY[ RKPYP": two uprights and a bar, so three strokes.
        assert_eq!(glyphs[72 - 32].strokes.len(), 3);
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
}
