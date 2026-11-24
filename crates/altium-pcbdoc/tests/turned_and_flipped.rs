//! An off-centre STEP model on a part that is turned or flipped, and a rotated fill, checked
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
        matches!(&model.source, ModelSource::Embedded { stream } if stream == "/Models/0"),
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
    // Flipping moved R1's centre to (25.4, 15) and gave it rotation 180 (see README.txt).
    check_r1(
        "one-bottom-step-resistor.PcbDoc",
        (25.4, 15.0),
        180.0,
        Side::Bottom,
    )
}

#[test]
fn the_rotated_fill_is_turned_30_degrees_about_its_centre() -> Result<(), Box<dyn std::error::Error>>
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
