//! Roadmap step 4: texts, checked against the numbers in tests/boards/README.txt (to 0.001 mm,
//! as AGENTS.md asks).

use altium_pcbdoc::Altium;
use board_model::{InputFormat, LayerKind, ReadOutcome, Severity, Side, Text, TextFont};

const TOLERANCE: f64 = 0.001;

fn read(name: &str) -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards")
        .join(name);
    Ok(Altium.read(&std::fs::read(path)?)?)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOLERANCE
}

/// The board's only text and the name, kind and side of its layer.
fn only_text(out: &ReadOutcome) -> Option<(&Text, String, LayerKind, Option<Side>)> {
    let [text] = out.board.texts.as_slice() else {
        return None;
    };
    let layer = out.board.layers.get(text.layer)?;
    Some((text, layer.name.clone(), layer.kind, layer.side))
}

#[test]
fn one_text_is_test_on_top_overlay() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-text.PcbDoc")?;
    let (t, layer, kind, side) = only_text(&out).ok_or("expected exactly one text")?;
    assert_eq!(t.content, "TEST");
    assert_eq!(
        (layer.as_str(), kind, side),
        ("Top Overlay", LayerKind::Silkscreen, Some(Side::Top))
    );
    assert!(near(t.at.x, 10.0) && near(t.at.y, 10.0), "{:?}", t.at);
    assert!(near(t.height, 2.0), "{}", t.height);
    assert_eq!(t.rotation_deg, 0.0);
    assert!(!t.mirrored);
    // README.txt: Altium's stroke font, stroke width 0.200.
    assert!(
        matches!(t.font, TextFont::Stroke { width } if near(width, 0.2)),
        "{:?}",
        t.font
    );
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn one_rotated_text_is_turned_90_degrees() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-rotated-text.PcbDoc")?;
    let (t, layer, _, _) = only_text(&out).ok_or("expected exactly one text")?;
    assert_eq!((t.content.as_str(), layer.as_str()), ("ROT", "Top Overlay"));
    assert!(near(t.at.x, 25.0) && near(t.at.y, 15.0), "{:?}", t.at);
    assert!(near(t.height, 2.0));
    assert!(near(t.rotation_deg, 90.0), "{}", t.rotation_deg);
    assert!(!t.mirrored);
    Ok(())
}

#[test]
fn one_bottom_text_is_mirrored_on_bottom_overlay() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-bottom-text.PcbDoc")?;
    let (t, layer, kind, side) = only_text(&out).ok_or("expected exactly one text")?;
    assert_eq!(t.content, "BOT");
    assert_eq!(
        (layer.as_str(), kind, side),
        ("Bottom Overlay", LayerKind::Silkscreen, Some(Side::Bottom))
    );
    assert!(near(t.at.x, 40.0) && near(t.at.y, 10.0), "{:?}", t.at);
    assert_eq!(t.rotation_deg, 0.0);
    assert!(t.mirrored);
    Ok(())
}

#[test]
fn the_resistor_shows_its_designator_but_not_its_comment() -> Result<(), Box<dyn std::error::Error>>
{
    let out = read("one-resistor.PcbDoc")?;
    let contents: Vec<&str> = out.board.texts.iter().map(|t| t.content.as_str()).collect();
    assert_eq!(contents, ["R1"], "{:?}", out.board.texts);
    Ok(())
}

#[test]
fn boards_without_texts_have_none() -> Result<(), Box<dyn std::error::Error>> {
    for name in ["empty-outline.PcbDoc", "one-track.PcbDoc"] {
        assert!(read(name)?.board.texts.is_empty(), "{name}");
    }
    Ok(())
}

#[test]
fn one_ttf_text_is_read_like_a_stroke_text_with_a_font_note(
) -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-ttf-text.PcbDoc")?;
    let (t, layer, _, _) = only_text(&out).ok_or("expected exactly one text")?;
    assert_eq!((t.content.as_str(), layer.as_str()), ("TTF", "Top Overlay"));
    assert!(near(t.at.x, 10.0) && near(t.at.y, 10.0), "{:?}", t.at);
    assert!(near(t.height, 2.0), "{}", t.height);
    assert_eq!(t.rotation_deg, 0.0);
    assert!(!t.mirrored);
    assert_eq!(
        t.font,
        TextFont::TrueType {
            name: "Arial".into(),
            bold: false,
            italic: false,
        }
    );
    assert!(out.fonts.is_empty());
    let [note] = out.notes.as_slice() else {
        return Err(format!("expected one note, got {:?}", out.notes).into());
    };
    assert_eq!(note.severity, Severity::Info);
    assert!(
        note.message
            .contains("TrueType font \"Arial\", which the board does not embed"),
        "{}",
        note.message
    );
    Ok(())
}

/// The sizes of the public DejaVu Sans Mono 2.37 files (tests/boards/README.txt has their SHA-256).
const DEJAVU: [(&str, usize); 3] = [
    ("DejaVuSansMono.ttf", 340712),
    ("DejaVuSansMono-Bold.ttf", 331992),
    ("DejaVuSansMono-Oblique.ttf", 251932),
];

/// The sum of a TrueType file's big-endian 32-bit words (the last one padded with zeros), which
/// the OpenType specification makes 0xB1B0AFBA for a whole, unchanged file.
fn truetype_checksum(file: &[u8]) -> u32 {
    let (words, rest) = file.as_chunks::<4>();
    let mut last = [0u8; 4];
    for (to, from) in last.iter_mut().zip(rest) {
        *to = *from;
    }
    words
        .iter()
        .chain(std::iter::once(&last))
        .fold(0u32, |sum, w| sum.wrapping_add(u32::from_be_bytes(*w)))
}

/// Reads a DejaVu board and checks its one text, the font file handed over with it, and that the
/// file gives the text the bounding rectangle in README.txt (`expected_box`).
fn check_dejavu(
    board: &str,
    content: &str,
    bold: bool,
    italic: bool,
    style: (&str, &str),
    file: &str,
    expected_box: [f64; 4],
) -> Result<(), Box<dyn std::error::Error>> {
    let out = read(board)?;
    let (t, layer, _, _) = only_text(&out).ok_or("expected exactly one text")?;
    assert_eq!(
        (t.content.as_str(), layer.as_str()),
        (content, "Top Overlay")
    );
    assert!(near(t.at.x, 10.0) && near(t.at.y, 10.0), "{:?}", t.at);
    assert!(near(t.height, 2.0), "{}", t.height);
    assert_eq!((t.rotation_deg, t.mirrored), (0.0, false));
    assert_eq!(
        t.font,
        TextFont::TrueType {
            name: "DejaVu Sans Mono".into(),
            bold,
            italic,
        }
    );
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    let [font] = out.fonts.as_slice() else {
        return Err(format!("expected one font, got {}", out.fonts.len()).into());
    };
    assert_eq!(out.font(&t.font), Some(font));
    assert_eq!(
        (
            font.family.as_str(),
            font.full_name.as_str(),
            font.style.as_str()
        ),
        ("DejaVu Sans Mono", style.0, style.1)
    );
    assert_eq!((font.bold, font.italic), (bold, italic));
    let size = DEJAVU
        .iter()
        .find(|(name, _)| *name == file)
        .map(|(_, size)| *size)
        .ok_or("unknown font file")?;
    assert_eq!(font.data.len(), size);
    assert_eq!(font.data.get(..4), Some([0u8, 1, 0, 0].as_slice()));
    assert_eq!(truetype_checksum(&font.data), 0xB1B0_AFBA);
    let measured = ink_box(&font.data, &t.content, t.at.x, t.at.y, t.height)
        .ok_or("the font file could not be measured")?;
    assert!(
        measured
            .iter()
            .zip(expected_box)
            .all(|(m, e)| (m - e).abs() <= BOX_TOLERANCE),
        "measured {measured:?}, README.txt {expected_box:?}"
    );
    Ok(())
}

#[test]
fn one_dejavu_text_hands_over_the_regular_font_file() -> Result<(), Box<dyn std::error::Error>> {
    check_dejavu(
        "one-dejavu-text.PcbDoc",
        "DejaVu",
        false,
        false,
        ("DejaVu Sans Mono", "Book"),
        "DejaVuSansMono.ttf",
        [10.000, 9.643, 15.936, 11.305],
    )
}

#[test]
fn one_dejavu_bold_text_hands_over_the_bold_font_file() -> Result<(), Box<dyn std::error::Error>> {
    check_dejavu(
        "one-dejavu-bold-text.PcbDoc",
        "Bold",
        true,
        false,
        ("DejaVu Sans Mono Bold", "Bold"),
        "DejaVuSansMono-Bold.ttf",
        [10.000, 9.977, 13.905, 11.305],
    )
}

#[test]
fn one_dejavu_italic_text_hands_over_the_oblique_font_file(
) -> Result<(), Box<dyn std::error::Error>> {
    check_dejavu(
        "one-dejavu-italic-text.PcbDoc",
        "Italic",
        false,
        true,
        ("DejaVu Sans Mono Oblique", "Oblique"),
        "DejaVuSansMono-Oblique.ttf",
        [10.000, 9.977, 16.075, 11.313],
    )
}

/// How close the box measured from the font file comes to the one Altium reports. Altium's box
/// comes out about 0.05 % smaller than the rule in [`ink_box`] gives (0.003 mm over 6 mm).
const BOX_TOLERANCE: f64 = 0.005;

/// A text's bounding rectangle (min x, min y, max x, max y), measured from its TrueType file
/// with the rule the three DejaVu boards show: the font is scaled so that ascender minus
/// descender (`hhea`) is the text's height; the box holds the glyphs' ink (each glyph's `glyf`
/// box, placed by the advance widths in `hmtx`); its left edge is at the text's x and the
/// baseline at its y. Reads only what this needs: `head`, `hhea`, `hmtx`, `loca`, `glyf` and a
/// format 4 `cmap` (the OpenType specification).
fn ink_box(file: &[u8], content: &str, x: f64, y: f64, height: f64) -> Option<[f64; 4]> {
    let u16_at = |at: usize| Some(u16::from_be_bytes(file.get(at..at + 2)?.try_into().ok()?));
    let i16_at = |at: usize| Some(i16::from_be_bytes(file.get(at..at + 2)?.try_into().ok()?));
    let u32_at = |at: usize| Some(u32::from_be_bytes(file.get(at..at + 4)?.try_into().ok()?));
    let table = |tag: &[u8; 4]| {
        (0..usize::from(u16_at(4)?)).find_map(|i| {
            let at = 12 + 16 * i;
            (file.get(at..at + 4)? == tag).then(|| usize::try_from(u32_at(at + 8)?).ok())?
        })
    };
    let (head, hhea, hmtx) = (table(b"head")?, table(b"hhea")?, table(b"hmtx")?);
    let (loca, glyf, cmap) = (table(b"loca")?, table(b"glyf")?, table(b"cmap")?);
    let long_loca = i16_at(head + 50)? != 0;
    let em = f64::from(i16_at(hhea + 4)?) - f64::from(i16_at(hhea + 6)?);
    let metrics = usize::from(u16_at(hhea + 34)?);
    let sub = (0..usize::from(u16_at(cmap + 2)?)).find_map(|i| {
        let at = cmap + 4 + 8 * i;
        (u16_at(at)? == 3 && u16_at(at + 2)? == 1)
            .then(|| Some(cmap + usize::try_from(u32_at(at + 4)?).ok()?))?
    })?;
    let segments = usize::from(u16_at(sub + 6)?) / 2;
    let glyph = |c: char| -> Option<usize> {
        let c = u16::try_from(u32::from(c)).ok()?;
        let i = (0..segments).find(|&i| u16_at(sub + 14 + 2 * i) >= Some(c))?;
        let start = u16_at(sub + 16 + 2 * segments + 2 * i)?;
        let delta = u16_at(sub + 16 + 4 * segments + 2 * i)?;
        let range_at = sub + 16 + 6 * segments + 2 * i;
        let range = usize::from(u16_at(range_at)?);
        let id = if range == 0 {
            c.wrapping_add(delta)
        } else {
            let g = u16_at(range_at + range + 2 * usize::from(c.checked_sub(start)?))?;
            g.wrapping_add(delta)
        };
        Some(usize::from(id))
    };
    let (mut pen, mut ink) = (0.0, [f64::MAX, f64::MAX, f64::MIN, f64::MIN]);
    for c in content.chars() {
        let g = glyph(c)?;
        let at = if long_loca {
            usize::try_from(u32_at(loca + 4 * g)?).ok()?
        } else {
            2 * usize::from(u16_at(loca + 2 * g)?)
        };
        let [x0, y0, x1, y1] = [2, 4, 6, 8].map(|o| i16_at(glyf + at + o).map(f64::from));
        ink = [
            ink[0].min(pen + x0?),
            ink[1].min(y0?),
            ink[2].max(pen + x1?),
            ink[3].max(y1?),
        ];
        pen += f64::from(u16_at(hmtx + 4 * g.min(metrics - 1))?);
    }
    let scale = height / em;
    Some([
        x,
        y + ink[1] * scale,
        x + (ink[2] - ink[0]) * scale,
        y + ink[3] * scale,
    ])
}
