//! Roadmap step 2: the board outline, its cutouts and the thickness, checked against the numbers
//! in tests/boards/README.txt (to 0.001 mm, as AGENTS.md asks).

use altium_pcbdoc::Altium;
use board_model::{Contour, InputFormat, ReadOutcome, Segment};

const TOLERANCE: f64 = 0.001;

fn read(name: &str) -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards")
        .join(name);
    Ok(Altium.read(&std::fs::read(path)?)?)
}

type Xy = (f64, f64);

/// The (from, to) ends of each segment, or `None` if any segment is not a straight line.
fn lines(contour: &Contour) -> Option<Vec<(Xy, Xy)>> {
    contour
        .segments
        .iter()
        .map(|s| match s {
            Segment::Line { from, to } => Some(((from.x, from.y), (to.x, to.y))),
            Segment::Arc { .. } => None,
        })
        .collect()
}

/// True when the contour is closed, straight-edged and has exactly these corners, in either
/// direction and starting anywhere.
fn same_polygon(contour: &Contour, expected: &[(f64, f64)]) -> bool {
    let Some(lines) = lines(contour) else {
        return false;
    };
    let next = lines.iter().skip(1).chain(lines.first());
    let closed = lines.iter().zip(next).all(|(a, b)| a.1 == b.0);
    let got: Vec<(f64, f64)> = lines.iter().map(|l| l.0).collect();
    let near = |a: (f64, f64), b: (f64, f64)| {
        (a.0 - b.0).abs() <= TOLERANCE && (a.1 - b.1).abs() <= TOLERANCE
    };
    let matches = |order: &[(f64, f64)]| {
        got.len() == order.len()
            && (0..order.len()).any(|shift| {
                let rotated = order.iter().cycle().skip(shift);
                got.iter().zip(rotated).all(|(p, q)| near(*p, *q))
            })
    };
    let reversed: Vec<(f64, f64)> = expected.iter().rev().copied().collect();
    closed && !got.is_empty() && (matches(expected) || matches(&reversed))
}

const BOARD_50_BY_30: [(f64, f64); 4] = [(0.0, 0.0), (50.0, 0.0), (50.0, 30.0), (0.0, 30.0)];

#[test]
fn empty_outline_is_50_by_30_and_1_6_thick() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("empty-outline.PcbDoc")?;
    assert!(
        same_polygon(&out.board.outline.outer, &BOARD_50_BY_30),
        "{:?}",
        out.board.outline
    );
    assert!(out.board.outline.cutouts.is_empty());
    let t = out.board.thickness.ok_or("no thickness")?;
    assert!((t - 1.6).abs() <= TOLERANCE, "thickness {t}");
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn outline_with_hole_has_the_10_by_10_cutout() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("outline-with-hole.PcbDoc")?;
    assert!(same_polygon(&out.board.outline.outer, &BOARD_50_BY_30));
    assert_eq!(out.board.outline.cutouts.len(), 1);
    assert!(same_polygon(
        &out.board.outline.cutouts[0],
        &[(30.0, 10.0), (40.0, 10.0), (40.0, 20.0), (30.0, 20.0)]
    ));
    // The 3 mm hole is a pad; tests/copper.rs checks it. The only thing not read is the
    // shape-based copy of the region, which is reported.
    assert!(
        out.notes
            .iter()
            .all(|n| n.message.contains("/ShapeBasedRegions6/Data")),
        "{:?}",
        out.notes
    );
    Ok(())
}

#[test]
fn every_board_has_the_same_outline_and_thickness() -> Result<(), Box<dyn std::error::Error>> {
    for name in [
        "empty-outline.PcbDoc",
        "outline-with-hole.PcbDoc",
        "one-track.PcbDoc",
        "one-resistor.PcbDoc",
        "one-via.PcbDoc",
        "one-plated-pad.PcbDoc",
        "one-rotated-pad.PcbDoc",
        "one-oval-pad.PcbDoc",
        "one-bottom-track.PcbDoc",
        "one-arc.PcbDoc",
        "one-fill.PcbDoc",
        "one-region.PcbDoc",
        "one-pour.PcbDoc",
        "one-text.PcbDoc",
        "one-rotated-text.PcbDoc",
        "one-bottom-text.PcbDoc",
        "one-step-resistor.PcbDoc",
        "one-rotated-resistor.PcbDoc",
        "one-bottom-resistor.PcbDoc",
        "one-net.PcbDoc",
        "one-offset-step-resistor.PcbDoc",
        "one-rotated-step-resistor.PcbDoc",
        "one-bottom-step-resistor.PcbDoc",
        "one-rotated-fill.PcbDoc",
        "one-ttf-text.PcbDoc",
        "one-dejavu-text.PcbDoc",
        "one-dejavu-bold-text.PcbDoc",
        "one-dejavu-italic-text.PcbDoc",
        "one-offset-extruded-resistor.PcbDoc",
        "one-colored-body-resistor.PcbDoc",
        "one-bottom-extruded-resistor.PcbDoc",
        "shared-step-model.PcbDoc",
        "two-step-models.PcbDoc",
        "one-turned-step-resistor.PcbDoc",
        "one-bottom-turned-step-resistor.PcbDoc",
        "one-bottom-rotated-turned-step-resistor.PcbDoc",
        "one-bottom-rotated-extruded-resistor.PcbDoc",
        "one-overlay-line.PcbDoc",
        "one-overlay-arc.PcbDoc",
        "one-bottom-overlay-line.PcbDoc",
        "one-resistor-silkscreen.PcbDoc",
        "one-tilted-step-resistor.PcbDoc",
        "one-bottom-tilted-step-resistor.PcbDoc",
        "one-turned-tilted-step-resistor.PcbDoc",
        "one-y-tilted-step-resistor.PcbDoc",
        "one-bottom-y-tilted-step-resistor.PcbDoc",
        "one-turned-y-tilted-step-resistor.PcbDoc",
        "small-board.PcbDoc",
        "one-blue-mask-resistor.PcbDoc",
        "one-yellow-silkscreen-resistor.PcbDoc",
        "one-enig-resistor.PcbDoc",
        "one-hasl-resistor.PcbDoc",
        "one-step-header.PcbDoc",
    ] {
        let out = read(name)?;
        assert!(
            same_polygon(&out.board.outline.outer, &BOARD_50_BY_30),
            "{name}"
        );
        let t = out.board.thickness.ok_or("no thickness")?;
        assert!((t - 1.6).abs() <= TOLERANCE, "{name}: thickness {t}");
    }
    Ok(())
}

/// An arc as (centre x, centre y, radius, start, end), angles in degrees.
type ArcXy = (f64, f64, f64, f64, f64);

/// The arcs of a contour, and the number of lines.
fn arcs_and_lines(contour: &Contour) -> (Vec<ArcXy>, usize) {
    let mut arcs = Vec::new();
    let mut lines = 0;
    for s in &contour.segments {
        match *s {
            Segment::Line { .. } => lines += 1,
            Segment::Arc {
                center,
                radius,
                start_deg,
                end_deg,
                ..
            } => arcs.push((center.x, center.y, radius, start_deg, end_deg)),
        }
    }
    (arcs, lines)
}

/// The start and end point of a segment.
fn ends(s: &Segment) -> (Xy, Xy) {
    match *s {
        Segment::Line { from, to } => ((from.x, from.y), (to.x, to.y)),
        Segment::Arc {
            center,
            radius,
            start_deg,
            end_deg,
            ..
        } => {
            let at = |deg: f64| {
                let (sin, cos) = deg.to_radians().sin_cos();
                (center.x + radius * cos, center.y + radius * sin)
            };
            (at(start_deg), at(end_deg))
        }
    }
}

/// True when every segment starts where the one before it ends.
fn chained(contour: &Contour) -> bool {
    let segments = &contour.segments;
    let before = segments
        .iter()
        .cycle()
        .skip(segments.len().saturating_sub(1));
    segments.iter().zip(before).all(|(s, prev)| {
        let ((start, _), (_, end)) = (ends(s), ends(prev));
        (end.0 - start.0).abs() <= TOLERANCE && (end.1 - start.1).abs() <= TOLERANCE
    })
}

/// True when the arc (centre, radius, start, end) is the expected one; angles are compared
/// modulo 360.
fn same_arc(got: ArcXy, want: ArcXy) -> bool {
    let angle = |a: f64, b: f64| {
        let d = (a - b).rem_euclid(360.0);
        d.min(360.0 - d) <= 1e-4
    };
    (got.0 - want.0).abs() <= TOLERANCE
        && (got.1 - want.1).abs() <= TOLERANCE
        && (got.2 - want.2).abs() <= TOLERANCE
        && angle(got.3, want.3)
        && angle(got.4, want.4)
}

#[test]
fn outline_with_arcs_has_rounded_corners_and_a_round_cutout(
) -> Result<(), Box<dyn std::error::Error>> {
    let out = read("outline-with-arcs.PcbDoc")?;
    let outer = &out.board.outline.outer;
    let (arcs, lines) = arcs_and_lines(outer);
    assert_eq!((arcs.len(), lines), (4, 4), "{outer:?}");
    assert!(chained(outer), "{outer:?}");
    for want in [
        (47.0, 3.0, 3.0, 270.0, 360.0),
        (47.0, 27.0, 3.0, 0.0, 90.0),
        (3.0, 27.0, 3.0, 90.0, 180.0),
        (3.0, 3.0, 3.0, 180.0, 270.0),
    ] {
        assert!(
            arcs.iter().any(|&got| same_arc(got, want)),
            "{want:?} in {arcs:?}"
        );
    }
    let [cutout] = out.board.outline.cutouts.as_slice() else {
        return Err(format!("expected one cutout: {:?}", out.board.outline.cutouts).into());
    };
    let (arcs, lines) = arcs_and_lines(cutout);
    assert_eq!((arcs.len(), lines), (2, 0), "{cutout:?}");
    assert!(chained(cutout), "{cutout:?}");
    for want in [
        (40.0, 15.0, 2.0, 0.0, 180.0),
        (40.0, 15.0, 2.0, 180.0, 360.0),
    ] {
        assert!(
            arcs.iter().any(|&got| same_arc(got, want)),
            "{want:?} in {arcs:?}"
        );
    }
    let t = out.board.thickness.ok_or("no thickness")?;
    assert!((t - 1.6).abs() <= TOLERANCE, "thickness {t}");
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn outline_with_notch_has_convex_and_concave_arcs() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("outline-with-notch.PcbDoc")?;
    let outer = &out.board.outline.outer;
    let (_, lines) = arcs_and_lines(outer);
    assert_eq!(lines, 8, "{outer:?}");
    assert!(chained(outer), "{outer:?}");
    // The outline comes counter-clockwise: the notch's mouth corners as counter-clockwise arcs,
    // its concave bottom corners as clockwise ones, each from where the outline reaches it.
    let arcs: Vec<(ArcXy, bool)> = outer
        .segments
        .iter()
        .filter_map(|s| match *s {
            Segment::Arc {
                center,
                radius,
                start_deg,
                end_deg,
                clockwise,
            } => Some(((center.x, center.y, radius, start_deg, end_deg), clockwise)),
            Segment::Line { .. } => None,
        })
        .collect();
    assert_eq!(arcs.len(), 4, "{arcs:?}");
    for (want, clockwise) in [
        ((31.0, 29.0, 1.0, 90.0, 180.0), false),
        ((28.0, 22.0, 2.0, 0.0, 270.0), true),
        ((22.0, 22.0, 2.0, 270.0, 180.0), true),
        ((19.0, 29.0, 1.0, 0.0, 90.0), false),
    ] {
        assert!(
            arcs.iter()
                .any(|&(got, cw)| cw == clockwise && same_arc(got, want)),
            "{want:?} {clockwise} in {arcs:?}"
        );
    }
    assert!(out.board.outline.cutouts.is_empty());
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}
