//! Roadmap step 4: texts, checked against the numbers in tests/boards/README.txt (to 0.001 mm,
//! as AGENTS.md asks).

use altium_pcbdoc::Altium;
use board_model::{InputFormat, LayerKind, ReadOutcome, Severity, Side, Text};

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
    let [note] = out.notes.as_slice() else {
        return Err(format!("expected one note, got {:?}", out.notes).into());
    };
    assert_eq!(note.severity, Severity::Info);
    assert!(
        note.message.contains("TrueType font \"Arial\""),
        "{}",
        note.message
    );
    Ok(())
}
