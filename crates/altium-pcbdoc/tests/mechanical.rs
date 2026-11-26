//! The mechanical-layer boards (tests/boards/README.txt, "objects on mechanical layers"). The
//! reader does not read mechanical layers yet: these tests pin what it does today, that each
//! object on one is left out with a note naming the layer, and that the rest of the board is
//! unchanged. When mechanical layers are read, these tests change with it.

use altium_pcbdoc::Altium;
use board_model::{InputFormat, ReadOutcome, Severity, Side};

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

/// The messages of the Dropped notes, sorted.
fn dropped(out: &ReadOutcome) -> Vec<&str> {
    let mut d: Vec<&str> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Dropped)
        .map(|n| n.message.as_str())
        .collect();
    d.sort_unstable();
    d
}

#[test]
fn objects_on_mechanical_layers_are_left_out_with_a_note_each(
) -> Result<(), Box<dyn std::error::Error>> {
    // mechanical-layers: a track and two arcs on Mechanical 1, a fill and a text on
    // Mechanical 2, a triangular region on Mechanical 13; nothing else.
    let out = read("mechanical-layers.PcbDoc")?;
    assert!(out.board.copper.is_empty(), "{:?}", out.board.copper);
    assert!(out.board.graphics.is_empty(), "{:?}", out.board.graphics);
    assert!(out.board.texts.is_empty(), "{:?}", out.board.texts);
    assert!(out.board.components.is_empty());
    let d = dropped(&out);
    assert_eq!(d.len(), 5, "{d:?}");
    for (kind, count, layer) in [
        ("track", 1, "Mechanical 1"),
        ("arc", 2, "Mechanical 1"),
        ("fill", 1, "Mechanical 2"),
        ("text", 1, "Mechanical 2"),
        ("region", 1, "Mechanical 13"),
    ] {
        assert!(
            d.iter()
                .any(|m| m.starts_with(&format!("{count} {kind}(s)")) && m.contains(layer)),
            "no note for {count} {kind} on {layer}: {d:?}"
        );
    }
    Ok(())
}

#[test]
fn a_parts_outline_on_a_mechanical_pair_follows_it_to_the_bottom(
) -> Result<(), Box<dyn std::error::Error>> {
    // mechanical-pair-resistor: Mechanical 13 and 15 are a pair; R1 (one-resistor's) owns five
    // tracks on Mechanical 13. On mechanical-pair-bottom-resistor R1 was then flipped, and Altium
    // moved the five tracks to Mechanical 15.
    for (board, side, rotation, layer) in [
        (
            "mechanical-pair-resistor.PcbDoc",
            Side::Top,
            0.0,
            "Mechanical 13",
        ),
        (
            "mechanical-pair-bottom-resistor.PcbDoc",
            Side::Bottom,
            180.0,
            "Mechanical 15",
        ),
    ] {
        let out = read(board)?;
        let [r1] = out.board.components.as_slice() else {
            return Err(format!("{board}: {:?}", out.board.components).into());
        };
        assert_eq!(r1.designator, "R1");
        assert!(
            near(r1.at.x, 25.0) && near(r1.at.y, 15.0),
            "{board}: {:?}",
            r1.at
        );
        assert_eq!(r1.side, side, "{board}");
        assert!(
            near(r1.rotation_deg, rotation),
            "{board}: {}",
            r1.rotation_deg
        );
        // The two pads are read as on one-resistor; only the outline is left out.
        assert_eq!(out.board.copper.len(), 2, "{board}");
        let d = dropped(&out);
        assert_eq!(d.len(), 1, "{board}: {d:?}");
        assert!(
            d[0].starts_with("5 track(s)") && d[0].contains(layer),
            "{board}: {d:?}"
        );
    }
    Ok(())
}
