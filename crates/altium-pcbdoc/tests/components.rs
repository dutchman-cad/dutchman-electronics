//! Roadmap step 5, first part: component placements, checked against the numbers in
//! tests/boards/README.txt (to 0.001 mm, as AGENTS.md asks).

use altium_pcbdoc::Altium;
use board_model::{CopperItem, InputFormat, ReadOutcome, Severity, Side};

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

#[test]
fn one_resistor_is_r1_at_25_15_on_top() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-resistor.PcbDoc")?;
    assert_eq!(out.board.components.len(), 1, "{:?}", out.board.components);
    let r1 = &out.board.components[0];
    assert_eq!(r1.designator, "R1");
    assert_eq!(r1.footprint, "R0603-TEST");
    assert!(near(r1.at.x, 25.0) && near(r1.at.y, 15.0), "{:?}", r1.at);
    assert!(near(r1.rotation_deg, 0.0), "{}", r1.rotation_deg);
    assert_eq!(r1.side, Side::Top);
    // The 3D body is read; tests/models.rs checks it.
    assert!(r1.model.is_some());
    assert!(
        !out.notes
            .iter()
            .any(|n| n.severity == Severity::Dropped && n.message.contains("3D bod")),
        "{:?}",
        out.notes
    );
    assert!(
        !out.notes.iter().any(|n| n.message.contains("/Components6")),
        "{:?}",
        out.notes
    );
    Ok(())
}

#[test]
fn the_resistor_pads_belong_to_r1() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-resistor.PcbDoc")?;
    let owners: Vec<Option<&str>> = out
        .board
        .copper
        .iter()
        .filter_map(|item| match item {
            CopperItem::Pad { owner, .. } => Some(owner.as_deref()),
            _ => None,
        })
        .collect();
    assert_eq!(owners, vec![Some("R1"), Some("R1")]);
    Ok(())
}

#[test]
fn boards_without_components_have_none() -> Result<(), Box<dyn std::error::Error>> {
    for name in [
        "empty-outline.PcbDoc",
        "outline-with-hole.PcbDoc",
        "one-track.PcbDoc",
    ] {
        let out = read(name)?;
        assert!(out.board.components.is_empty(), "{name}");
        // The free pad in outline-with-hole belongs to no component.
        for item in &out.board.copper {
            if let CopperItem::Pad { owner, .. } = item {
                assert_eq!(owner, &None, "{name}");
            }
        }
    }
    Ok(())
}
