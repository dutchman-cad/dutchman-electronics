//! Roadmap step 5, second part: component 3D models, checked against the numbers in
//! tests/boards/README.txt (to 0.001 mm, as AGENTS.md asks).

use altium_pcbdoc::Altium;
use board_model::{InputFormat, ModelRef, ModelSource, ReadOutcome, Segment};

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

/// R1's model on a board with exactly one component.
fn r1_model(out: &ReadOutcome) -> Option<&ModelRef> {
    let [r1] = out.board.components.as_slice() else {
        return None;
    };
    (r1.designator == "R1")
        .then_some(r1.model.as_ref())
        .flatten()
}

#[test]
fn one_resistor_has_an_extruded_1_6_by_0_8_by_0_45_box() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-resistor.PcbDoc")?;
    let model = r1_model(&out).ok_or("R1 has no model")?;
    let ModelSource::Extruded {
        outline,
        standoff,
        height,
    } = &model.source
    else {
        return Err(format!("expected an extruded body, got {model:?}").into());
    };
    assert!(
        near(*standoff, 0.0) && near(*height, 0.45),
        "{standoff} {height}"
    );
    // In R1's own coordinates: corners (-0.8, -0.4), (0.8, -0.4), (0.8, 0.4), (-0.8, 0.4).
    let mut corners: Vec<(f64, f64)> = outline
        .segments
        .iter()
        .filter_map(|s| match s {
            Segment::Line { from, .. } => Some((from.x, from.y)),
            Segment::Arc { .. } => None,
        })
        .collect();
    corners.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let want = [(-0.8, -0.4), (-0.8, 0.4), (0.8, -0.4), (0.8, 0.4)];
    assert_eq!(corners.len(), 4, "{corners:?}");
    for (g, w) in corners.iter().zip(want) {
        assert!(near(g.0, w.0) && near(g.1, w.1), "{corners:?}");
    }
    Ok(())
}

#[test]
fn one_step_resistor_has_its_embedded_step_box() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-step-resistor.PcbDoc")?;
    let model = r1_model(&out).ok_or("R1 has no model")?;
    assert_eq!(model.name, "box-1608.step");
    assert_eq!(
        model.source,
        ModelSource::Embedded {
            stream: "/Models/0".into()
        }
    );
    // The model's origin is R1's centre, not rotated, not raised.
    assert!(
        model.offset.iter().all(|v| near(*v, 0.0)),
        "{:?}",
        model.offset
    );
    assert!(
        model.rotation_deg.iter().all(|v| near(*v, 0.0)),
        "{:?}",
        model.rotation_deg
    );
    Ok(())
}

#[test]
fn the_embedded_model_stream_exists() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards/one-step-resistor.PcbDoc");
    let streams = altium_pcbdoc::list_streams(&std::fs::read(path)?)?;
    let model = streams
        .iter()
        .find(|s| s.path == "/Models/0")
        .ok_or("no /Models/0 stream")?;
    assert!(model.len > 0);
    Ok(())
}
