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

/// The one pad on a board.
fn only_pad(out: &ReadOutcome) -> Option<&CopperItem> {
    match out.board.copper.as_slice() {
        [one] => Some(one),
        _ => None,
    }
}

#[test]
fn one_via_is_1_mm_with_a_0_5_mm_hole() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-via.PcbDoc")?;
    assert_eq!(out.board.copper.len(), 1, "{:?}", out.board.copper);
    let CopperItem::Via {
        at,
        diameter,
        drill,
    } = &out.board.copper[0]
    else {
        panic!("expected a via, got {:?}", out.board.copper[0]);
    };
    assert!(near(at.x, 25.0) && near(at.y, 15.0), "{at:?}");
    assert!(near(*diameter, 1.0), "{diameter}");
    assert!(near(*drill, 0.5), "{drill}");
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn one_plated_pad_is_plated_and_the_hole_is_not() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-plated-pad.PcbDoc")?;
    let Some(CopperItem::Pad {
        layer,
        at,
        shape,
        size_x,
        size_y,
        rotation_deg,
        drill,
        plated,
        ..
    }) = only_pad(&out)
    else {
        panic!("expected a pad");
    };
    assert_eq!(layer_name(&out, *layer), "Multi-Layer");
    assert!(near(at.x, 25.0) && near(at.y, 15.0), "{at:?}");
    assert_eq!(*shape, PadShape::Round);
    assert!(near(*size_x, 2.0) && near(*size_y, 2.0));
    assert_eq!(*rotation_deg, 0.0);
    assert!(drill.is_some_and(|d| near(d, 1.0)), "{drill:?}");
    assert!(*plated);
    assert!(out.notes.is_empty(), "{:?}", out.notes);

    let hole = read("outline-with-hole.PcbDoc")?;
    let Some(CopperItem::Pad { plated, .. }) = only_pad(&hole) else {
        panic!("expected a pad");
    };
    assert!(!*plated);
    Ok(())
}

#[test]
fn one_rotated_pad_is_2_by_1_at_30_degrees() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-rotated-pad.PcbDoc")?;
    let Some(CopperItem::Pad {
        layer,
        at,
        shape,
        size_x,
        size_y,
        rotation_deg,
        drill,
        plated,
        ..
    }) = only_pad(&out)
    else {
        panic!("expected a pad");
    };
    assert_eq!(layer_name(&out, *layer), "Top Layer");
    assert!(near(at.x, 25.0) && near(at.y, 15.0), "{at:?}");
    assert_eq!(*shape, PadShape::Rectangle);
    // Sizes are before rotation, as typed.
    assert!(
        near(*size_x, 2.0) && near(*size_y, 1.0),
        "{size_x} x {size_y}"
    );
    assert!(near(*rotation_deg, 30.0), "{rotation_deg}");
    assert_eq!((*drill, *plated), (None, false));
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn one_oval_pad_is_an_oval_2_by_1() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-oval-pad.PcbDoc")?;
    let Some(CopperItem::Pad {
        layer,
        at,
        shape,
        size_x,
        size_y,
        rotation_deg,
        drill,
        ..
    }) = only_pad(&out)
    else {
        panic!("expected a pad");
    };
    assert_eq!(layer_name(&out, *layer), "Top Layer");
    assert!(near(at.x, 25.0) && near(at.y, 15.0), "{at:?}");
    assert_eq!(*shape, PadShape::Oval);
    assert!(
        near(*size_x, 2.0) && near(*size_y, 1.0),
        "{size_x} x {size_y}"
    );
    assert_eq!(*rotation_deg, 0.0);
    assert_eq!(*drill, None);
    Ok(())
}

#[test]
fn one_bottom_track_is_on_the_bottom_layer() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-bottom-track.PcbDoc")?;
    assert_eq!(out.board.copper.len(), 1, "{:?}", out.board.copper);
    let CopperItem::Track {
        layer,
        from,
        to,
        width,
        ..
    } = &out.board.copper[0]
    else {
        panic!("expected a track, got {:?}", out.board.copper[0]);
    };
    assert_eq!(layer_name(&out, *layer), "Bottom Layer");
    let bottom = &out.board.layers[*layer];
    assert_eq!(
        (bottom.kind, bottom.side),
        (LayerKind::Copper, Some(Side::Bottom))
    );
    assert!(near(from.x, 10.0) && near(from.y, 15.0), "{from:?}");
    assert!(near(to.x, 20.0) && near(to.y, 15.0), "{to:?}");
    assert!(near(*width, 0.5), "{width}");
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn one_arc_is_a_quarter_circle_of_radius_5() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-arc.PcbDoc")?;
    assert_eq!(out.board.copper.len(), 1, "{:?}", out.board.copper);
    let CopperItem::TrackArc {
        layer,
        center,
        radius,
        start_deg,
        end_deg,
        width,
        net,
    } = &out.board.copper[0]
    else {
        panic!("expected an arc, got {:?}", out.board.copper[0]);
    };
    assert_eq!(layer_name(&out, *layer), "Top Layer");
    assert!(near(center.x, 25.0) && near(center.y, 15.0), "{center:?}");
    assert!(near(*radius, 5.0), "{radius}");
    assert!(near(*start_deg, 0.0) && near(*end_deg, 90.0));
    assert!(near(*width, 0.5), "{width}");
    assert_eq!(net, &None);
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}
