//! Roadmap step 5, second part: component 3D models, checked against the numbers in
//! tests/boards/README.txt (to 0.001 mm, as AGENTS.md asks).

use altium_pcbdoc::Altium;
use board_model::{Color, InputFormat, ModelRef, ModelSource, ReadOutcome, Segment, Severity};

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

/// The MODELIDs Altium gave our two STEP boxes (see tests/boards/README.txt).
const BOX_1608_ID: &str = "{2F7959BF-FEB0-48A0-A2AE-FD0E192F11AA}";
const BOX_OFFSET_ID: &str = "{46A6A6F0-03C2-4649-BD81-931DF1E87BA7}";

/// The smallest and largest x of the points in a STEP file, rounded to 0.001: shows which of
/// our two boxes it is (x -0.8 to 0.8, or 0 to 1.6) and that it was unpacked whole.
fn x_range(step: &[u8]) -> Option<(f64, f64)> {
    let text = std::str::from_utf8(step).ok()?;
    if !text.starts_with("ISO-10303-21;") || !text.trim_end().ends_with("END-ISO-10303-21;") {
        return None;
    }
    let xs: Vec<f64> = text
        .split("CARTESIAN_POINT(")
        .skip(1)
        .filter_map(|p| p.split_once(",(")?.1.split(',').next()?.trim().parse().ok())
        .collect();
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    Some((
        round(xs.iter().copied().fold(f64::MAX, f64::min)),
        round(xs.iter().copied().fold(f64::MIN, f64::max)),
    ))
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
            id: BOX_1608_ID.into()
        }
    );
    // The board hands over the unpacked STEP file once.
    let [stored] = out.models.as_slice() else {
        return Err(format!("expected one model, got {}", out.models.len()).into());
    };
    assert_eq!(
        (stored.id.as_str(), stored.name.as_str()),
        (BOX_1608_ID, "box-1608.step")
    );
    assert_eq!(x_range(&stored.step).ok_or("no points")?, (-0.8, 0.8));
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

/// The smallest and largest of one coordinate (0 x, 1 y, 2 z) of the points in a STEP text,
/// rounded to 0.001.
fn axis_range(step: &[u8], axis: usize) -> Option<(f64, f64)> {
    let text = std::str::from_utf8(step).ok()?;
    let values: Vec<f64> = text
        .split("CARTESIAN_POINT(")
        .skip(1)
        .filter_map(|p| {
            let inner = p.split_once(",(")?.1.split_once(')')?.0;
            inner.split(',').nth(axis)?.trim().parse().ok()
        })
        .collect();
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    (!values.is_empty()).then(|| {
        (
            round(values.iter().copied().fold(f64::MAX, f64::min)),
            round(values.iter().copied().fold(f64::MIN, f64::max)),
        )
    })
}

/// `one-step-header`: J1, a 1x4 through-hole header whose body is an embedded STEP model of a
/// housing and four pins that reach 3 mm below it (tests/boards/README.txt).
#[test]
fn one_step_header_is_a_through_hole_part_with_its_step_model(
) -> Result<(), Box<dyn std::error::Error>> {
    use board_model::{CopperItem, PadShape};
    let out = read("one-step-header.PcbDoc")?;
    let [j1] = out.board.components.as_slice() else {
        return Err(format!("expected one part, got {:?}", out.board.components).into());
    };
    assert_eq!(
        (j1.designator.as_str(), j1.footprint.as_str()),
        ("J1", "HDR1X4-STEP")
    );
    assert!(near(j1.at.x, 25.0) && near(j1.at.y, 15.0), "{:?}", j1.at);
    // Four plated pads 1.7 on a 2.54 pitch, hole 1.0; pin 1 square at the top.
    let mut pads: Vec<(f64, PadShape)> = Vec::new();
    for item in &out.board.copper {
        let CopperItem::Pad {
            at,
            shape,
            size_x,
            size_y,
            drill,
            plated,
            owner,
            ..
        } = item
        else {
            return Err(format!("expected only pads, got {item:?}").into());
        };
        assert!(near(at.x, 25.0), "{at:?}");
        assert!(
            near(*size_x, 1.7) && near(*size_y, 1.7),
            "{size_x} {size_y}"
        );
        assert!(drill.is_some_and(|d| near(d, 1.0)) && *plated, "{drill:?}");
        assert_eq!(owner.as_deref(), Some("J1"));
        pads.push((at.y, *shape));
    }
    pads.sort_by(|a, b| b.0.total_cmp(&a.0));
    let want = [
        (18.81, PadShape::Rectangle),
        (16.27, PadShape::Round),
        (13.73, PadShape::Round),
        (11.19, PadShape::Round),
    ];
    assert_eq!(pads.len(), 4, "{pads:?}");
    for (got, want) in pads.iter().zip(want) {
        assert!(near(got.0, want.0) && got.1 == want.1, "{pads:?}");
    }
    // The model: embedded, at J1's centre, not turned or raised.
    let model = j1.model.as_ref().ok_or("J1 has no model")?;
    assert_eq!(model.name, "header-1x4.step");
    assert_eq!(
        model.source,
        ModelSource::Embedded {
            id: "{AFE25888-7D73-4AF3-9861-6835E8C67860}".into()
        }
    );
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
    // Unpacked whole: the housing and four pins, the pins from 3 below the base to 8.5 up.
    let [stored] = out.models.as_slice() else {
        return Err(format!("expected one model, got {}", out.models.len()).into());
    };
    assert_eq!(stored.name, "header-1x4.step");
    let text = std::str::from_utf8(&stored.step)?;
    assert_eq!(text.matches("MANIFOLD_SOLID_BREP(").count(), 5);
    assert_eq!(
        axis_range(&stored.step, 0).ok_or("no points")?,
        (-1.27, 1.27)
    );
    assert_eq!(
        axis_range(&stored.step, 1).ok_or("no points")?,
        (-5.08, 5.08)
    );
    assert_eq!(axis_range(&stored.step, 2).ok_or("no points")?, (-3.0, 8.5));
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

/// `linked-model/`: R1's body links to `box-1608.step` next to the board instead of holding it
/// (tests/boards/linked-model/README.txt).
#[test]
fn one_linked_step_resistor_names_its_file_and_checksum() -> Result<(), Box<dyn std::error::Error>>
{
    let out = read("linked-model/one-linked-step-resistor.PcbDoc")?;
    let model = r1_model(&out).ok_or("R1 has no model")?;
    assert_eq!(model.name, "box-1608.step");
    assert_eq!(
        model.source,
        ModelSource::External {
            file: "box-1608.step".into(),
            checksum: Some(1_259_363_632),
        }
    );
    // Nothing is embedded, and nothing is left unexplained.
    assert!(out.models.is_empty());
    assert!(
        out.notes.iter().all(|n| n.severity == Severity::Info),
        "{:?}",
        out.notes
    );
    // Placed as on one-step-resistor: R1's centre, not rotated, not raised.
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
    // The file next to the board is the one the board was linked to.
    let step = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/boards/linked-model/box-1608.step"),
    )?;
    assert_eq!(Altium.model_checksum(&step), Some(1_259_363_632));
    assert_eq!(x_range(&step).ok_or("no points")?, (-0.8, 0.8));
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

/// The designator, centre and model ID of each part, sorted by designator.
fn parts(out: &ReadOutcome) -> Vec<(String, f64, f64, Option<String>)> {
    let mut found: Vec<_> = out
        .board
        .components
        .iter()
        .map(|p| {
            let id = match p.model.as_ref().map(|m| &m.source) {
                Some(ModelSource::Embedded { id }) => Some(id.clone()),
                _ => None,
            };
            (p.designator.clone(), p.at.x, p.at.y, id)
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

#[test]
fn two_parts_share_one_embedded_model() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("shared-step-model.PcbDoc")?;
    let p = parts(&out);
    assert_eq!(p.len(), 2, "{p:?}");
    for (part, (name, x)) in p.iter().zip([("R1", 25.0), ("R2", 35.0)]) {
        assert_eq!(part.0, name);
        assert!(near(part.1, x) && near(part.2, 15.0), "{p:?}");
        assert_eq!(part.3.as_deref(), Some(BOX_1608_ID), "{p:?}");
    }
    let [stored] = out.models.as_slice() else {
        return Err(format!("expected one model, got {}", out.models.len()).into());
    };
    assert_eq!(stored.name, "box-1608.step");
    assert_eq!(x_range(&stored.step).ok_or("no points")?, (-0.8, 0.8));
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn two_parts_with_two_embedded_models() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("two-step-models.PcbDoc")?;
    let p = parts(&out);
    assert_eq!(p.len(), 2, "{p:?}");
    assert_eq!(p[0].3.as_deref(), Some(BOX_1608_ID), "{p:?}");
    assert_eq!(p[1].3.as_deref(), Some(BOX_OFFSET_ID), "{p:?}");
    assert_eq!(out.models.len(), 2);
    let model = |id: &str| {
        out.models
            .iter()
            .find(|m| m.id == id)
            .ok_or("model missing")
    };
    let (centred, offset) = (model(BOX_1608_ID)?, model(BOX_OFFSET_ID)?);
    assert_eq!(centred.name, "box-1608.step");
    assert_eq!(x_range(&centred.step).ok_or("no points")?, (-0.8, 0.8));
    assert_eq!(offset.name, "box-offset.step");
    assert_eq!(x_range(&offset.step).ok_or("no points")?, (0.0, 1.6));
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

/// Every board with a 3D body also has /ShapeBasedComponentBodies6/Data, which only repeats the
/// bodies: no note about it.
#[test]
fn the_shape_based_copy_of_the_bodies_raises_no_note() -> Result<(), Box<dyn std::error::Error>> {
    for name in [
        "one-resistor.PcbDoc",
        "one-rotated-resistor.PcbDoc",
        "one-bottom-extruded-resistor.PcbDoc",
        "one-step-resistor.PcbDoc",
        "shared-step-model.PcbDoc",
        "one-bottom-rotated-turned-step-resistor.PcbDoc",
        "one-step-header.PcbDoc",
    ] {
        let out = read(name)?;
        assert!(
            !out.notes.iter().any(|n| n.message.contains("ShapeBased")),
            "{name}: {:?}",
            out.notes
        );
    }
    Ok(())
}

#[test]
fn an_extruded_body_has_its_colour_and_opacity() -> Result<(), Box<dyn std::error::Error>> {
    let grey = read("one-resistor.PcbDoc")?;
    let colored = read("one-colored-body-resistor.PcbDoc")?;
    let grey_model = r1_model(&grey).ok_or("R1 has no model")?;
    let model = r1_model(&colored).ok_or("R1 has no model")?;
    assert_eq!(
        grey_model.color,
        Some(Color {
            r: 128,
            g: 128,
            b: 128,
            opacity: 1.0
        })
    );
    assert_eq!(
        model.color,
        Some(Color {
            r: 32,
            g: 128,
            b: 224,
            opacity: 0.5
        })
    );
    // Only the colour differs: the same box in the same place, and no notes.
    assert_eq!(model.source, grey_model.source);
    assert_eq!(
        (model.offset, model.rotation_deg),
        (grey_model.offset, grey_model.rotation_deg)
    );
    assert!(colored.notes.is_empty(), "{:?}", colored.notes);
    Ok(())
}

#[test]
fn a_step_model_has_no_body_colour() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-step-resistor.PcbDoc")?;
    let model = r1_model(&out).ok_or("R1 has no model")?;
    assert_eq!(model.color, None);
    Ok(())
}
