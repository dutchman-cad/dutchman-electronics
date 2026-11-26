//! An off-center STEP model on a part that is turned or flipped, and a rotated fill, checked
//! against the numbers in tests/boards/README.txt (to 0.001 mm, as AGENTS.md asks).

use altium_pcbdoc::Altium;
use board_model::{CopperItem, InputFormat, ModelSource, ReadOutcome, Segment, Severity, Side};

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

/// Checks R1 and its STEP model on one of the three boards.
fn check_r1(
    name: &str,
    at: (f64, f64),
    rotation: f64,
    side: Side,
) -> Result<(), Box<dyn std::error::Error>> {
    let out = read(name)?;
    let [r1] = out.board.components.as_slice() else {
        return Err(format!("{name}: expected one part").into());
    };
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
    let model = r1.model.as_ref().ok_or("R1 has no model")?;
    assert_eq!(model.name, "box-offset.step", "{name}");
    assert!(
        matches!(&model.source, ModelSource::Embedded { id }
            if out.models.iter().any(|m| &m.id == id && m.name == "box-offset.step")),
        "{name}: {:?}",
        model.source
    );
    // On the part itself the model sits the same way on all three boards.
    assert!(
        model.offset.iter().all(|v| near(*v, 0.0)),
        "{name}: {:?}",
        model.offset
    );
    assert!(
        model.rotation_deg.iter().all(|v| near(*v, 0.0)),
        "{name}: {:?}",
        model.rotation_deg
    );
    assert!(
        !out.notes.iter().any(|n| n.severity == Severity::Warning),
        "{name}: {:?}",
        out.notes
    );
    Ok(())
}

#[test]
fn the_offset_step_model_on_a_top_side_part() -> Result<(), Box<dyn std::error::Error>> {
    check_r1(
        "one-offset-step-resistor.PcbDoc",
        (25.0, 15.0),
        0.0,
        Side::Top,
    )
}

#[test]
fn the_offset_step_model_on_a_part_turned_90_degrees() -> Result<(), Box<dyn std::error::Error>> {
    check_r1(
        "one-rotated-step-resistor.PcbDoc",
        (25.0, 15.0),
        90.0,
        Side::Top,
    )
}

#[test]
fn the_offset_step_model_on_a_flipped_part() -> Result<(), Box<dyn std::error::Error>> {
    // Flipping moved R1's center to (25.4, 15) and gave it rotation 180 (see README.txt).
    check_r1(
        "one-bottom-step-resistor.PcbDoc",
        (25.4, 15.0),
        180.0,
        Side::Bottom,
    )
}

#[test]
fn the_rotated_fill_is_turned_30_degrees_about_its_center() -> Result<(), Box<dyn std::error::Error>>
{
    let out = read("one-rotated-fill.PcbDoc")?;
    let [CopperItem::Region { contour, .. }] = out.board.copper.as_slice() else {
        return Err(format!("expected one region, got {:?}", out.board.copper).into());
    };
    let (sin, cos) = 30f64.to_radians().sin_cos();
    let turn = |x: f64, y: f64| {
        (
            25.0 + (x - 25.0) * cos - (y - 15.0) * sin,
            15.0 + (x - 25.0) * sin + (y - 15.0) * cos,
        )
    };
    let mut want = [
        turn(20.0, 10.0),
        turn(30.0, 10.0),
        turn(30.0, 20.0),
        turn(20.0, 20.0),
    ];
    let mut got: Vec<(f64, f64)> = contour
        .segments
        .iter()
        .filter_map(|s| match s {
            Segment::Line { from, .. } => Some((from.x, from.y)),
            Segment::Arc { .. } => None,
        })
        .collect();
    let order = |a: &(f64, f64), b: &(f64, f64)| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1));
    want.sort_by(order);
    got.sort_by(order);
    assert_eq!(got.len(), 4, "{got:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(near(g.0, w.0) && near(g.1, w.1), "{got:?} vs {want:?}");
    }
    // The bounding box Altium reported: x 18.170 to 31.830.
    assert!(near(
        got.iter().map(|p| p.0).fold(f64::MAX, f64::min),
        18.170
    ));
    assert!(near(
        got.iter().map(|p| p.0).fold(f64::MIN, f64::max),
        31.830
    ));
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

/// Checks R1's turned, off-center STEP model on one of the eighth-batch boards: on the part it is
/// always at (-0.350, 0.606), turned 30 degrees.
fn check_turned_model(
    name: &str,
    at: (f64, f64),
    rotation: f64,
    side: Side,
) -> Result<(), Box<dyn std::error::Error>> {
    let out = read(name)?;
    let [r1] = out.board.components.as_slice() else {
        return Err(format!("{name}: expected one part").into());
    };
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
    let model = r1.model.as_ref().ok_or("R1 has no model")?;
    assert_eq!(model.name, "box-offset.step", "{name}");
    let want = [-0.35, 0.7 * 30f64.to_radians().cos(), 0.0];
    assert!(
        model.offset.iter().zip(want).all(|(g, w)| near(*g, w)),
        "{name}: {:?}",
        model.offset
    );
    assert!(
        near(model.rotation_deg[0], 0.0)
            && near(model.rotation_deg[1], 0.0)
            && near(model.rotation_deg[2], 30.0),
        "{name}: {:?}",
        model.rotation_deg
    );
    assert!(
        !out.notes.iter().any(|n| n.severity == Severity::Warning),
        "{name}: {:?}",
        out.notes
    );
    Ok(())
}

#[test]
fn a_model_turned_on_a_top_side_part() -> Result<(), Box<dyn std::error::Error>> {
    check_turned_model(
        "one-turned-step-resistor.PcbDoc",
        (25.0, 15.0),
        0.0,
        Side::Top,
    )
}

#[test]
fn a_model_turned_on_a_flipped_part_keeps_its_place() -> Result<(), Box<dyn std::error::Error>> {
    // Flipping moved R1's center to (25.036, 15) and gave it rotation 180 (see README.txt).
    check_turned_model(
        "one-bottom-turned-step-resistor.PcbDoc",
        (25.0356, 15.0),
        180.0,
        Side::Bottom,
    )
}

#[test]
fn a_model_turned_on_a_flipped_part_at_90_keeps_its_place() -> Result<(), Box<dyn std::error::Error>>
{
    check_turned_model(
        "one-bottom-rotated-turned-step-resistor.PcbDoc",
        (23.7474, 15.0),
        90.0,
        Side::Bottom,
    )
}

/// Checks R1's tilted STEP model on one of the tenth- or thirteenth-batch boards: in the part's
/// own frame it is tilted by `tilt` (degrees about X and Y), turned `turn` about Z, its origin at
/// `offset` from R1's center, with no warning.
fn check_tilted_model(
    name: &str,
    at: (f64, f64),
    rotation: f64,
    side: Side,
    offset: (f64, f64),
    tilt: (f64, f64),
    turn: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let out = read(name)?;
    let [r1] = out.board.components.as_slice() else {
        return Err(format!("{name}: expected one part").into());
    };
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
    let model = r1.model.as_ref().ok_or("R1 has no model")?;
    assert_eq!(model.name, "box-offset.step", "{name}");
    assert!(
        near(model.offset[0], offset.0)
            && near(model.offset[1], offset.1)
            && near(model.offset[2], 0.0),
        "{name}: {:?}",
        model.offset
    );
    assert!(
        near(model.rotation_deg[0], tilt.0)
            && near(model.rotation_deg[1], tilt.1)
            && near(model.rotation_deg[2], turn),
        "{name}: {:?}",
        model.rotation_deg
    );
    assert!(
        !out.notes.iter().any(|n| n.severity == Severity::Warning),
        "{name}: {:?}",
        out.notes
    );
    Ok(())
}

#[test]
fn a_model_tilted_about_x_on_a_top_side_part() -> Result<(), Box<dyn std::error::Error>> {
    check_tilted_model(
        "one-tilted-step-resistor.PcbDoc",
        (25.0, 15.0),
        0.0,
        Side::Top,
        (0.0, 0.05),
        (90.0, 0.0),
        0.0,
    )
}

#[test]
fn a_model_tilted_about_x_keeps_its_tilt_on_a_flipped_part(
) -> Result<(), Box<dyn std::error::Error>> {
    check_tilted_model(
        "one-bottom-tilted-step-resistor.PcbDoc",
        (25.4, 15.0),
        180.0,
        Side::Bottom,
        (0.0, 0.05),
        (90.0, 0.0),
        0.0,
    )
}

#[test]
fn a_model_tilted_about_x_and_turned_on_its_part() -> Result<(), Box<dyn std::error::Error>> {
    // The origin (0, 0.05) turned by 30 degrees with the model.
    let (sin, cos) = 30f64.to_radians().sin_cos();
    check_tilted_model(
        "one-turned-tilted-step-resistor.PcbDoc",
        (25.0, 15.0),
        0.0,
        Side::Top,
        (-0.05 * sin, 0.05 * cos),
        (90.0, 0.0),
        30.0,
    )
}

#[test]
fn a_model_tilted_about_y_on_a_top_side_part() -> Result<(), Box<dyn std::error::Error>> {
    check_tilted_model(
        "one-y-tilted-step-resistor.PcbDoc",
        (25.0, 15.0),
        0.0,
        Side::Top,
        (0.0, 0.4),
        (0.0, 90.0),
        0.0,
    )
}

#[test]
fn a_model_tilted_about_y_keeps_its_tilt_on_a_flipped_part(
) -> Result<(), Box<dyn std::error::Error>> {
    // In the part's own frame (turned 180, then flipped about its X axis) this is the model of
    // the top-side board; a tilt of -90 would put the body at x 25 to 25.45, not Altium's
    // x 24.55 to 25.
    check_tilted_model(
        "one-bottom-y-tilted-step-resistor.PcbDoc",
        (25.0, 15.0),
        180.0,
        Side::Bottom,
        (0.0, 0.4),
        (0.0, 90.0),
        0.0,
    )
}

#[test]
fn a_model_tilted_about_y_and_turned_on_its_part() -> Result<(), Box<dyn std::error::Error>> {
    // The origin (0, 0.4) turned by 30 degrees with the model.
    let (sin, cos) = 30f64.to_radians().sin_cos();
    check_tilted_model(
        "one-turned-y-tilted-step-resistor.PcbDoc",
        (25.0, 15.0),
        0.0,
        Side::Top,
        (-0.4 * sin, 0.4 * cos),
        (0.0, 90.0),
        30.0,
    )
}
