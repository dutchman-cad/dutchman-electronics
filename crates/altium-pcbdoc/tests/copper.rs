//! Roadmap step 3, first part: tracks and pads, checked against the numbers in
//! tests/boards/README.txt (to 0.001 mm, as AGENTS.md asks).

use altium_pcbdoc::Altium;
use board_model::{CopperItem, InputFormat, LayerKind, PadShape, ReadOutcome, Side};

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

/// The name of the layer at `index` in the board's layer list.
fn layer_name(out: &ReadOutcome, index: usize) -> &str {
    out.board
        .layers
        .get(index)
        .map(|l| l.name.as_str())
        .unwrap_or("(no such layer)")
}

#[test]
fn one_track_has_one_top_layer_track() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-track.PcbDoc")?;
    assert_eq!(out.board.copper.len(), 1, "{:?}", out.board.copper);
    let CopperItem::Track {
        layer,
        from,
        to,
        width,
        net,
    } = &out.board.copper[0]
    else {
        panic!("expected a track, got {:?}", out.board.copper[0]);
    };
    assert_eq!(layer_name(&out, *layer), "Top Layer");
    let top = &out.board.layers[*layer];
    assert_eq!((top.kind, top.side), (LayerKind::Copper, Some(Side::Top)));
    assert!(near(from.x, 10.0) && near(from.y, 15.0), "{from:?}");
    assert!(near(to.x, 20.0) && near(to.y, 15.0), "{to:?}");
    assert!(near(*width, 0.5), "{width}");
    assert_eq!(net, &None);
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn the_hole_is_an_unplated_3_mm_pad_on_all_layers() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("outline-with-hole.PcbDoc")?;
    assert_eq!(out.board.copper.len(), 1, "{:?}", out.board.copper);
    let CopperItem::Pad {
        layer,
        at,
        shape,
        size_x,
        size_y,
        rotation_deg,
        drill,
        ..
    } = &out.board.copper[0]
    else {
        panic!("expected a pad, got {:?}", out.board.copper[0]);
    };
    assert_eq!(layer_name(&out, *layer), "Multi-Layer");
    assert!(near(at.x, 10.0) && near(at.y, 15.0), "{at:?}");
    assert_eq!(*shape, PadShape::Round);
    assert!(near(*size_x, 3.0) && near(*size_y, 3.0));
    assert_eq!(*rotation_deg, 0.0);
    assert!(drill.is_some_and(|d| near(d, 3.0)), "{drill:?}");
    Ok(())
}

#[test]
fn the_resistor_has_two_0_8_by_1_0_smd_pads() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-resistor.PcbDoc")?;
    let mut centres = Vec::new();
    for item in &out.board.copper {
        let CopperItem::Pad {
            layer,
            at,
            shape,
            size_x,
            size_y,
            drill,
            ..
        } = item
        else {
            panic!("expected only pads, got {item:?}");
        };
        assert_eq!(layer_name(&out, *layer), "Top Layer");
        assert_eq!(*shape, PadShape::Rectangle);
        assert!(
            near(*size_x, 0.8) && near(*size_y, 1.0),
            "{size_x} x {size_y}"
        );
        assert_eq!(*drill, None);
        centres.push((at.x, at.y));
    }
    centres.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(centres.len(), 2, "{centres:?}");
    assert!(
        near(centres[0].0, 24.2) && near(centres[0].1, 15.0),
        "{centres:?}"
    );
    assert!(
        near(centres[1].0, 25.8) && near(centres[1].1, 15.0),
        "{centres:?}"
    );
    Ok(())
}

#[test]
fn a_board_without_copper_has_none() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("empty-outline.PcbDoc")?;
    assert!(out.board.copper.is_empty());
    Ok(())
}
