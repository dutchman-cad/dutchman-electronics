//! Roadmap step 4, second part: lines and arcs on the overlay (silkscreen) layers, checked
//! against the numbers in tests/boards/README.txt (to 0.001 mm, as AGENTS.md asks).

use altium_pcbdoc::Altium;
use board_model::{Graphic, InputFormat, LayerKind, ReadOutcome, Side};

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

/// The name, kind and side of a graphic's layer.
fn layer_of(out: &ReadOutcome, layer: usize) -> Option<(String, LayerKind, Option<Side>)> {
    let l = out.board.layers.get(layer)?;
    Some((l.name.clone(), l.kind, l.side))
}

/// Checks a board whose only graphic is a line, and that nothing else is on it.
fn check_line(
    name: &str,
    want: ((f64, f64), (f64, f64), f64),
    want_layer: (&str, Side),
    want_owner: Option<&str>,
) -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let out = read(name)?;
    let [Graphic::Line {
        layer,
        from,
        to,
        width,
        owner,
    }] = out.board.graphics.as_slice()
    else {
        return Err(format!("{name}: expected one line, got {:?}", out.board.graphics).into());
    };
    let ((x1, y1), (x2, y2), w) = want;
    assert!(
        near(from.x, x1) && near(from.y, y1) && near(to.x, x2) && near(to.y, y2),
        "{name}: {from:?} to {to:?}"
    );
    assert!(near(*width, w), "{name}: {width}");
    assert_eq!(
        layer_of(&out, *layer).ok_or("no layer")?,
        (
            want_layer.0.to_string(),
            LayerKind::Silkscreen,
            Some(want_layer.1)
        ),
        "{name}"
    );
    assert_eq!(owner.as_deref(), want_owner, "{name}");
    assert!(
        !out.notes.iter().any(|n| n.message.contains("Overlay")),
        "{name}: {:?}",
        out.notes
    );
    Ok(out)
}

#[test]
fn a_line_on_top_overlay() -> Result<(), Box<dyn std::error::Error>> {
    let out = check_line(
        "one-overlay-line.PcbDoc",
        ((10.0, 10.0), (20.0, 10.0), 0.2),
        ("Top Overlay", Side::Top),
        None,
    )?;
    assert!(out.board.copper.is_empty());
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn a_line_on_bottom_overlay() -> Result<(), Box<dyn std::error::Error>> {
    let out = check_line(
        "one-bottom-overlay-line.PcbDoc",
        ((30.0, 20.0), (40.0, 25.0), 0.15),
        ("Bottom Overlay", Side::Bottom),
        None,
    )?;
    assert!(out.board.copper.is_empty());
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn a_component_line_belongs_to_r1() -> Result<(), Box<dyn std::error::Error>> {
    let out = check_line(
        "one-resistor-silkscreen.PcbDoc",
        ((24.0, 15.8), (26.0, 15.8), 0.15),
        ("Top Overlay", Side::Top),
        Some("R1"),
    )?;
    // R1's designator text shares the overlay layer with its line.
    let [text] = out.board.texts.as_slice() else {
        return Err("expected R1's designator text".into());
    };
    assert_eq!(text.content, "R1");
    assert!(matches!(
        out.board.graphics.first(),
        Some(Graphic::Line { layer, .. }) if *layer == text.layer
    ));
    Ok(())
}

#[test]
fn an_arc_on_top_overlay_runs_from_30_to_300_degrees() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-overlay-arc.PcbDoc")?;
    let [Graphic::Arc {
        layer,
        center,
        radius,
        start_deg,
        end_deg,
        width,
        owner,
    }] = out.board.graphics.as_slice()
    else {
        return Err(format!("expected one arc, got {:?}", out.board.graphics).into());
    };
    assert!(near(center.x, 25.0) && near(center.y, 15.0), "{center:?}");
    assert!(near(*radius, 3.0) && near(*width, 0.2), "{radius} {width}");
    assert!(
        near(*start_deg, 30.0) && near(*end_deg, 300.0),
        "{start_deg} {end_deg}"
    );
    assert_eq!(
        layer_of(&out, *layer).ok_or("no layer")?,
        (
            "Top Overlay".to_string(),
            LayerKind::Silkscreen,
            Some(Side::Top)
        )
    );
    assert_eq!(*owner, None);
    assert!(out.board.copper.is_empty());
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn boards_without_silkscreen_lines_have_no_graphics() -> Result<(), Box<dyn std::error::Error>> {
    for name in [
        "empty-outline.PcbDoc",
        "one-track.PcbDoc",
        "one-resistor.PcbDoc",
    ] {
        assert!(read(name)?.board.graphics.is_empty(), "{name}");
    }
    Ok(())
}
