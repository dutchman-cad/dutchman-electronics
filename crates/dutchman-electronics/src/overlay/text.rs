//! Text as pen strokes, in the Hershey Roman Simplex font (`fonts/hershey/rowmans.jhf`, free to
//! use with the acknowledgement in `fonts/hershey/README.txt`).
//!
//! The board file's own font is not used: a stroke text is drawn in this font at the text's
//! height and pen width, and a TrueType text the same way (its outlines would need the font
//! file). Letters and spacing therefore differ a little from the CAD program's, which the
//! package says in a Degraded note.
//!
//! A Hershey glyph is a list of pen strokes in font units, y down, with the baseline at 9 and
//! the top of a capital at -12. The text's height is taken as the height of a capital, and its
//! position as the left end of its baseline, before it is mirrored and turned.

use std::sync::OnceLock;

use board_model::{Point, Text, TextFont};

/// Font units from the top of a capital down to the baseline.
const CAP_UNITS: f64 = 21.0;
/// The baseline, in font units (y down).
const BASELINE: f64 = 9.0;
/// Font units from one line's baseline to the next.
const LINE_UNITS: f64 = 32.0;
/// The first character the font holds; the rest follow in order.
const FIRST: u32 = 32;
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
    let chars: Vec<u8> = source.bytes().filter(|&b| b != b'\n' && b != b'\r').collect();
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

/// The pen strokes of `text`, or `None` when its height, pen width, position or rotation is not
/// usable.
pub(super) fn draw(text: &Text) -> Option<Drawn> {
    let pen = match &text.font {
        TextFont::Stroke { width } => *width,
        TextFont::TrueType { .. } => text.height * TRUETYPE_PEN,
    };
    let usable = |v: f64| v.is_finite() && v > 0.0;
    if !usable(text.height)
        || !usable(pen)
        || !text.at.x.is_finite()
        || !text.at.y.is_finite()
        || !text.rotation_deg.is_finite()
    {
        return None;
    }
    let glyphs = font();
    let question = glyphs.get(usize::from(b'?') - FIRST as usize)?;
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
        assert!(x0 > 10.0, "the left bearing keeps the stroke right of the position");
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
    fn a_truetype_text_gets_a_pen_a_tenth_of_its_height() {
        let mut t = text("H", 0.0, false);
        t.font = TextFont::TrueType {
            name: "Arial".into(),
        };
        assert!(close(draw(&t).unwrap().pen, 0.21));
    }
}
