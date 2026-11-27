//! Features seen on public sample boards, rebuilt on our own CC0 boards (tests/boards/README.txt,
//! "Features seen on other people's boards"): filled shapes on the silkscreen, a moved board
//! origin and a solid 3D mask. Numbers to 0.001 mm, as AGENTS.md asks.

use altium_pcbdoc::Altium;
use board_model::{CopperItem, InputFormat, ModelSource, ReadOutcome, Segment, Severity};

const TOLERANCE: f64 = 0.001;

fn read(name: &str) -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards")
        .join(name);
    Ok(Altium.read(&std::fs::read(path)?)?)
}

/// overlay-region-fill: a region and two fills on each overlay. The board model has no filled
/// silkscreen shape yet, so they are left out, each layer with a note that says why (not, as
/// before, "not a copper or overlay layer").
#[test]
fn filled_shapes_on_the_overlay_are_left_out_with_the_right_reason(
) -> Result<(), Box<dyn std::error::Error>> {
    let out = read("overlay-region-fill.PcbDoc")?;
    assert!(out.board.graphics.is_empty(), "{:?}", out.board.graphics);
    assert!(
        !out.board
            .copper
            .iter()
            .any(|c| matches!(c, CopperItem::Region { .. })),
        "{:?}",
        out.board.copper
    );
    let mut dropped: Vec<&str> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Dropped)
        .map(|n| n.message.as_str())
        .collect();
    dropped.sort_unstable();
    let why = "left out: a filled silkscreen shape has no place in the board model yet";
    assert_eq!(
        dropped,
        [
            format!("1 region(s) on Bottom Overlay {why}"),
            format!("1 region(s) on Top Overlay {why}"),
            format!("2 fill(s) on Bottom Overlay {why}"),
            format!("2 fill(s) on Top Overlay {why}"),
        ]
    );
    Ok(())
}

/// origin-moved: one-resistor with the origin 10 right and 5 up. Nothing on the board moved, so
/// it reads exactly as one-resistor does (the outline's corner at (0, 0), R1 at (25, 15), its
/// pads at (24.2, 15) and (25.8, 15)); only an Info note names the origin.
#[test]
fn a_moved_origin_moves_nothing() -> Result<(), Box<dyn std::error::Error>> {
    let moved = read("origin-moved.PcbDoc")?;
    let plain = read("one-resistor.PcbDoc")?;
    assert!(moved.board == plain.board, "the boards differ");

    let starts: Vec<(f64, f64)> = moved
        .board
        .outline
        .outer
        .segments
        .iter()
        .filter_map(|s| match s {
            Segment::Line { from, .. } => Some((from.x, from.y)),
            _ => None,
        })
        .collect();
    let corner = starts
        .iter()
        .fold((f64::MAX, f64::MAX), |(x, y), p| (x.min(p.0), y.min(p.1)));
    assert!(
        corner.0.abs() <= TOLERANCE && corner.1.abs() <= TOLERANCE,
        "{starts:?}"
    );
    let r1 = moved.board.components.first().ok_or("no R1")?;
    assert!((r1.at.x - 25.0).abs() <= TOLERANCE && (r1.at.y - 15.0).abs() <= TOLERANCE);
    let mut pads: Vec<f64> = moved
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Pad { at, .. } if (at.y - 15.0).abs() <= TOLERANCE => Some(at.x),
            _ => None,
        })
        .collect();
    pads.sort_by(f64::total_cmp);
    assert!(
        pads.len() == 2
            && (pads[0] - 24.2).abs() <= TOLERANCE
            && (pads[1] - 25.8).abs() <= TOLERANCE,
        "{pads:?}"
    );

    let extra: Vec<_> = moved
        .notes
        .iter()
        .filter(|n| !plain.notes.contains(n))
        .collect();
    let [origin] = extra.as_slice() else {
        return Err(format!("{:?}", moved.notes).into());
    };
    assert_eq!(origin.severity, Severity::Info);
    assert_eq!(
        origin.message,
        "the board's origin is at (10.000, 5.000) mm; Altium shows positions from there, here \
         they are kept as stored (nothing on the board moves)"
    );
    assert_eq!(moved.notes.len(), plain.notes.len() + 1);
    Ok(())
}

/// mask-opacity-100: the 3D view's mask transparency at 0 stores an opacity of 1.000000 on both
/// sides; the mask color stays (36, 120, 50).
#[test]
fn a_solid_3d_mask_reads_opacity_one() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("mask-opacity-100.PcbDoc")?;
    let a = &out.board.appearance;
    for mask in [a.top_solder_mask, a.bottom_solder_mask] {
        let m = mask.ok_or("no mask color")?;
        assert_eq!((m.r, m.g, m.b), (36, 120, 50));
        assert!((m.opacity - 1.0).abs() < 1e-9, "{m:?}");
    }
    Ok(())
}

/// body-cylinder: R1's body as an upright cylinder, radius 0.5, height 1, standoff 0, its axis
/// at R1's center. Read as an extruded whole circle in R1's own frame, in R1's gray.
#[test]
fn a_cylinder_body_is_an_extruded_circle() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("body-cylinder.PcbDoc")?;
    let r1 = out.board.components.first().ok_or("no R1")?;
    let m = r1.model.as_ref().ok_or("R1 has no model")?;
    let ModelSource::Extruded {
        outline,
        standoff,
        height,
    } = &m.source
    else {
        return Err(format!("{m:?}").into());
    };
    let [Segment::Arc {
        center,
        radius,
        start_deg,
        end_deg,
        ..
    }] = outline.segments.as_slice()
    else {
        return Err(format!("{outline:?}").into());
    };
    assert!(
        center.x.abs() <= TOLERANCE && center.y.abs() <= TOLERANCE,
        "{center:?}"
    );
    assert!((radius - 0.5).abs() <= TOLERANCE, "{radius}");
    assert_eq!(start_deg, end_deg, "a whole circle");
    assert!(standoff.abs() <= TOLERANCE && (height - 1.0).abs() <= TOLERANCE);
    assert_eq!((m.offset, m.rotation_deg), ([0.0; 3], [0.0; 3]));
    let c = m.color.ok_or("no color")?;
    assert_eq!((c.r, c.g, c.b, c.opacity), (128, 128, 128, 1.0));
    assert!(
        out.notes.iter().all(|n| !n.message.contains("3D bod")),
        "{:?}",
        out.notes
    );
    Ok(())
}

/// body-sphere: the board model has no sphere, so R1's body is left out with a note.
#[test]
fn a_sphere_body_is_left_out_with_a_note() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("body-sphere.PcbDoc")?;
    let r1 = out.board.components.first().ok_or("no R1")?;
    assert!(r1.model.is_none(), "{:?}", r1.model);
    assert!(
        out.notes.iter().any(|n| {
            n.severity == Severity::Dropped
            && n.message
                == "1 component 3D bod(ies) not read: a sphere has no place in the board model yet"
        }),
        "{:?}",
        out.notes
    );
    Ok(())
}
