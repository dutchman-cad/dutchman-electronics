//! A rotated part, bottom-side parts, an off-center extruded body and a net, checked against the numbers in
//! tests/boards/README.txt (to 0.001 mm, as AGENTS.md asks).

use altium_pcbdoc::Altium;
use board_model::{
    CopperItem, InputFormat, ModelSource, Placement, ReadOutcome, Segment, Severity, Side,
};

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

fn only_part(out: &ReadOutcome) -> Option<&Placement> {
    let [part] = out.board.components.as_slice() else {
        return None;
    };
    Some(part)
}

/// The board's pads as (layer name, x, y, rotation), sorted by x.
fn pads(out: &ReadOutcome) -> Vec<(String, f64, f64, f64)> {
    let mut found: Vec<(String, f64, f64, f64)> = out
        .board
        .copper
        .iter()
        .filter_map(|item| match item {
            CopperItem::Pad {
                layer,
                at,
                rotation_deg,
                ..
            } => Some((
                out.board
                    .layers
                    .get(*layer)
                    .map(|l| l.name.clone())
                    .unwrap_or_default(),
                at.x,
                at.y,
                *rotation_deg,
            )),
            _ => None,
        })
        .collect();
    found.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)));
    found
}

/// The corners of an extruded model, sorted.
fn extruded_corners(part: &Placement) -> Option<(Vec<(f64, f64)>, f64)> {
    let ModelSource::Extruded {
        outline, height, ..
    } = &part.models.first()?.source
    else {
        return None;
    };
    let mut corners: Vec<(f64, f64)> = outline
        .segments
        .iter()
        .filter_map(|s| match s {
            Segment::Line { from, .. } => Some((from.x, from.y)),
            Segment::Arc { .. } => None,
        })
        .collect();
    corners.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    Some((corners, *height))
}

fn is_the_resistor_box(corners: &[(f64, f64)]) -> bool {
    let want = [(-0.8, -0.4), (-0.8, 0.4), (0.8, -0.4), (0.8, 0.4)];
    corners.len() == 4
        && corners
            .iter()
            .zip(want)
            .all(|(g, w)| near(g.0, w.0) && near(g.1, w.1))
}

#[test]
fn the_rotated_resistor_is_at_90_degrees() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-rotated-resistor.PcbDoc")?;
    let r1 = only_part(&out).ok_or("expected one part")?;
    assert_eq!((r1.designator.as_str(), r1.side), ("R1", Side::Top));
    assert!(near(r1.at.x, 25.0) && near(r1.at.y, 15.0), "{:?}", r1.at);
    assert!(near(r1.rotation_deg, 90.0), "{}", r1.rotation_deg);
    let p = pads(&out);
    assert_eq!(p.len(), 2, "{p:?}");
    assert!(
        p[0].0 == "Top Layer" && near(p[0].1, 25.0) && near(p[0].2, 14.2) && near(p[0].3, 90.0)
    );
    assert!(
        p[1].0 == "Top Layer" && near(p[1].1, 25.0) && near(p[1].2, 15.8) && near(p[1].3, 90.0)
    );
    // The box, turned back into R1's own coordinates, is the same box as on one-resistor.
    let (corners, height) = extruded_corners(r1).ok_or("R1 has no extruded model")?;
    assert!(is_the_resistor_box(&corners), "{corners:?}");
    assert!(near(height, 0.45));
    assert!(
        !out.notes.iter().any(|n| n.severity == Severity::Warning),
        "{:?}",
        out.notes
    );
    Ok(())
}

#[test]
fn the_bottom_resistor_is_on_the_bottom_at_180_degrees() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-bottom-resistor.PcbDoc")?;
    let r1 = only_part(&out).ok_or("expected one part")?;
    assert_eq!((r1.designator.as_str(), r1.side), ("R1", Side::Bottom));
    assert!(near(r1.at.x, 25.0) && near(r1.at.y, 15.0), "{:?}", r1.at);
    assert!(near(r1.rotation_deg, 180.0), "{}", r1.rotation_deg);
    let p = pads(&out);
    assert_eq!(p.len(), 2, "{p:?}");
    assert!(
        p[0].0 == "Bottom Layer" && near(p[0].1, 24.2) && near(p[0].2, 15.0),
        "{p:?}"
    );
    assert!(
        p[1].0 == "Bottom Layer" && near(p[1].1, 25.8) && near(p[1].2, 15.0),
        "{p:?}"
    );
    let (corners, height) = extruded_corners(r1).ok_or("R1 has no extruded model")?;
    assert!(is_the_resistor_box(&corners), "{corners:?}");
    assert!(near(height, 0.45));
    assert!(
        !out.notes.iter().any(|n| n.severity == Severity::Warning),
        "{:?}",
        out.notes
    );
    Ok(())
}

/// The lopsided outline of the sixth batch, in R1's own coordinates, sorted.
const OFFSET_OUTLINE: [(f64, f64); 4] = [(0.0, -0.4), (0.0, 0.4), (1.6, -0.4), (1.6, 0.0)];

/// Checks R1's lopsided extruded body on one of the two sixth-batch boards.
fn check_offset_extruded(
    name: &str,
    at: (f64, f64),
    rotation: f64,
    side: Side,
) -> Result<(), Box<dyn std::error::Error>> {
    let out = read(name)?;
    let r1 = only_part(&out).ok_or("expected one part")?;
    assert_eq!((r1.designator.as_str(), r1.side), ("R1", side), "{name}");
    assert!(
        near(r1.at.x, at.0) && near(r1.at.y, at.1),
        "{name}: {:?}",
        r1.at
    );
    assert!(
        near(r1.rotation_deg, rotation),
        "{name}: {}",
        r1.rotation_deg
    );
    let (corners, height) = extruded_corners(r1).ok_or("R1 has no extruded model")?;
    assert_eq!(corners.len(), 4, "{name}: {corners:?}");
    for (g, w) in corners.iter().zip(OFFSET_OUTLINE) {
        assert!(near(g.0, w.0) && near(g.1, w.1), "{name}: {corners:?}");
    }
    assert!(near(height, 0.45), "{name}: {height}");
    assert!(
        !out.notes.iter().any(|n| n.severity == Severity::Warning),
        "{name}: {:?}",
        out.notes
    );
    Ok(())
}

#[test]
fn the_offset_extruded_body_on_a_top_side_part() -> Result<(), Box<dyn std::error::Error>> {
    check_offset_extruded(
        "one-offset-extruded-resistor.PcbDoc",
        (25.0, 15.0),
        0.0,
        Side::Top,
    )
}

#[test]
fn the_offset_extruded_body_is_the_same_on_the_flipped_part(
) -> Result<(), Box<dyn std::error::Error>> {
    // Flipping moved R1's center to (25.4, 15) and gave it rotation 180 (see README.txt).
    check_offset_extruded(
        "one-bottom-extruded-resistor.PcbDoc",
        (25.4, 15.0),
        180.0,
        Side::Bottom,
    )
}

#[test]
fn the_offset_extruded_body_is_the_same_on_a_flipped_part_at_90(
) -> Result<(), Box<dyn std::error::Error>> {
    check_offset_extruded(
        "one-bottom-rotated-extruded-resistor.PcbDoc",
        (25.4, 15.0),
        90.0,
        Side::Bottom,
    )
}

#[test]
fn the_track_on_one_net_belongs_to_gnd() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-net.PcbDoc")?;
    let [CopperItem::Track { net, .. }] = out.board.copper.as_slice() else {
        return Err(format!("expected one track, got {:?}", out.board.copper).into());
    };
    assert_eq!(net.as_deref(), Some("GND"));
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn a_track_without_a_net_has_none() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-track.PcbDoc")?;
    let [CopperItem::Track { net, .. }] = out.board.copper.as_slice() else {
        return Err("expected one track".into());
    };
    assert_eq!(*net, None);
    Ok(())
}
