//! Features seen on public sample boards, rebuilt on our own CC0 boards (tests/boards/README.txt,
//! "Features seen on other people's boards"): filled shapes on the silkscreen, a moved board
//! origin and a solid 3D mask. Numbers to 0.001 mm, as AGENTS.md asks.

use altium_pcbdoc::Altium;
use board_model::{CopperItem, InputFormat, ModelSource, PadShape, ReadOutcome, Segment, Severity};

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
    let m = r1.models.first().ok_or("R1 has no model")?;
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
    assert!(r1.models.is_empty(), "{:?}", r1.models);
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

/// arc-region: the copper region on Top Layer is a rounded slot, x 8 to 18 and y 13 to 17: two
/// straight sides and two half circles of radius 2 about (10, 15) and (16, 15), read from
/// `/ShapeBasedRegions6/Data` instead of the 38-corner polygon of `/Regions6/Data`.
#[test]
fn a_region_keeps_its_half_circles() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("arc-region.PcbDoc")?;
    let regions: Vec<_> = out
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Region {
                layer,
                contour,
                holes,
            } => Some((layer, contour, holes)),
            _ => None,
        })
        .collect();
    let [(layer, contour, holes)] = regions.as_slice() else {
        return Err(format!("{regions:?}").into());
    };
    let name = out.board.layers.get(**layer).map(|l| l.name.as_str());
    assert_eq!(name, Some("Top Layer"));
    assert!(holes.is_empty());
    let mut centers = Vec::new();
    let mut lines = 0;
    for s in &contour.segments {
        match s {
            Segment::Arc {
                center,
                radius,
                start_deg,
                end_deg,
                ..
            } => {
                assert!((radius - 2.0).abs() <= TOLERANCE, "{s:?}");
                let sweep = s.sweep_deg().ok_or("no sweep")?;
                assert!((sweep.abs() - 180.0).abs() <= TOLERANCE, "{s:?}");
                assert!(start_deg.is_finite() && end_deg.is_finite());
                centers.push((center.x, center.y));
            }
            Segment::Line { from, to } => {
                lines += 1;
                assert!((from.y - to.y).abs() <= TOLERANCE, "{s:?}");
                let y = from.y;
                assert!((y - 13.0).abs() <= TOLERANCE || (y - 17.0).abs() <= TOLERANCE);
            }
        }
    }
    assert_eq!(lines, 2, "{contour:?}");
    centers.sort_by(|a, b| a.0.total_cmp(&b.0));
    let [(x0, y0), (x1, y1)] = centers.as_slice() else {
        return Err(format!("{contour:?}").into());
    };
    assert!((x0 - 10.0).abs() <= TOLERANCE && (y0 - 15.0).abs() <= TOLERANCE);
    assert!((x1 - 16.0).abs() <= TOLERANCE && (y1 - 15.0).abs() <= TOLERANCE);
    assert!(
        out.notes
            .iter()
            .all(|n| !n.message.contains("have arcs (in")),
        "{:?}",
        out.notes
    );
    Ok(())
}

/// body-arc-outline: R1's extruded body is a rounded slot, box (24.2, 14.6) to (25.8, 15.4), ends
/// of radius 0.4 about (24.6, 15) and (25.4, 15); in R1's own frame two lines at y -0.4 and 0.4
/// and two half circles about (-0.4, 0) and (0.4, 0), standoff 0, height 0.5.
#[test]
fn a_component_body_keeps_its_half_circles() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("body-arc-outline.PcbDoc")?;
    let r1 = out.board.components.first().ok_or("no R1")?;
    let m = r1.models.first().ok_or("R1 has no body")?;
    let ModelSource::Extruded {
        outline,
        standoff,
        height,
    } = &m.source
    else {
        return Err(format!("{m:?}").into());
    };
    assert!(standoff.abs() <= TOLERANCE && (height - 0.5).abs() <= TOLERANCE);
    let (mut lines, mut centers) = (Vec::new(), Vec::new());
    for s in &outline.segments {
        match s {
            Segment::Line { from, to } => lines.push((from.x, from.y, to.x, to.y)),
            Segment::Arc { center, radius, .. } => {
                assert!((radius - 0.4).abs() <= TOLERANCE, "{s:?}");
                let sweep = s.sweep_deg().ok_or("no sweep")?;
                assert!((sweep.abs() - 180.0).abs() <= TOLERANCE, "{s:?}");
                centers.push((center.x, center.y));
            }
        }
    }
    assert_eq!(lines.len(), 2, "{outline:?}");
    for (x0, y0, x1, y1) in &lines {
        assert!((y0 - y1).abs() <= TOLERANCE && (y0.abs() - 0.4).abs() <= TOLERANCE);
        assert!((x0.abs() - 0.4).abs() <= TOLERANCE && (x1.abs() - 0.4).abs() <= TOLERANCE);
    }
    centers.sort_by(|a, b| a.0.total_cmp(&b.0));
    let [(x0, y0), (x1, y1)] = centers.as_slice() else {
        return Err(format!("{outline:?}").into());
    };
    assert!((x0 + 0.4).abs() <= TOLERANCE && y0.abs() <= TOLERANCE);
    assert!((x1 - 0.4).abs() <= TOLERANCE && y1.abs() <= TOLERANCE);
    assert!(
        out.notes.iter().all(|n| !n.message.contains("have arcs")),
        "{:?}",
        out.notes
    );
    Ok(())
}

/// pad-stack-tmb: one plated pad, top round 2.000, middle round 1.400, bottom rectangle
/// 2.400 x 1.600, hole 0.800. The board model has one shape per pad, so it is read with the top
/// size and shape, and a note says so.
#[test]
fn a_top_middle_bottom_pad_is_read_with_its_top_size() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("pad-stack-tmb.PcbDoc")?;
    let pads: Vec<_> = out
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Pad {
                at,
                shape,
                size_x,
                size_y,
                drill,
                plated,
                ..
            } => Some((at, shape, size_x, size_y, drill, plated)),
            _ => None,
        })
        .collect();
    let [(at, shape, size_x, size_y, drill, plated)] = pads.as_slice() else {
        return Err(format!("{pads:?}").into());
    };
    assert!((at.x - 25.0).abs() <= TOLERANCE && (at.y - 15.0).abs() <= TOLERANCE);
    assert_eq!(**shape, PadShape::Round);
    assert!((**size_x - 2.0).abs() <= TOLERANCE && (**size_y - 2.0).abs() <= TOLERANCE);
    assert!(drill.is_some_and(|d| (d - 0.8).abs() <= TOLERANCE) && **plated);
    assert!(
        out.notes.iter().any(|n| n.message
            == "1 pad(s) differ between top, middle and bottom; read with the top size and shape"),
        "{:?}",
        out.notes
    );
    Ok(())
}

/// solder-mask-region: a solid region on Top Solder, (22, 12) to (28, 18), is a mask opening
/// drawn by hand. The board model has no place for one, so it is left out with a note that says
/// what it is.
#[test]
fn a_hand_drawn_mask_opening_is_left_out_with_a_note() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("solder-mask-region.PcbDoc")?;
    let dropped: Vec<&str> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Dropped)
        .map(|n| n.message.as_str())
        .collect();
    assert_eq!(
        dropped,
        [
            "1 region(s) on Top Solder left out: a solder mask opening drawn by hand has no place \
          in the board model yet"
        ]
    );
    // one-fill's copper fill is still there, the only region.
    let regions = out
        .board
        .copper
        .iter()
        .filter(|c| matches!(c, CopperItem::Region { .. }))
        .count();
    assert_eq!(regions, 1);
    Ok(())
}

/// overlapping-holes: two plated pads, round 1.600 with holes of 1.000, at (24.6, 15) and
/// (25.4, 15), so the holes overlap by 0.200. Both are read with their holes; cutting two
/// overlapping holes into the board is the board writer's (`board-step`), which today cuts one.
#[test]
fn overlapping_holes_are_both_read() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("overlapping-holes.PcbDoc")?;
    let mut holes: Vec<(f64, f64, f64)> = out
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Pad {
                at,
                drill: Some(d),
                plated: true,
                ..
            } => Some((at.x, at.y, *d)),
            _ => None,
        })
        .collect();
    holes.sort_by(|a, b| a.0.total_cmp(&b.0));
    let want = [(24.6, 15.0, 1.0), (25.4, 15.0, 1.0)];
    assert_eq!(holes.len(), want.len(), "{holes:?}");
    for (h, w) in holes.iter().zip(want) {
        assert!(
            (h.0 - w.0).abs() <= TOLERANCE
                && (h.1 - w.1).abs() <= TOLERANCE
                && (h.2 - w.2).abs() <= TOLERANCE,
            "{holes:?}"
        );
    }
    Ok(())
}
