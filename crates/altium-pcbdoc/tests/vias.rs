//! The via test boards (tests/boards/README.txt, "Vias and stroke fonts"): sizes and places as
//! Altium reports them, to 0.001 mm as AGENTS.md asks. What the reader does not read yet (via
//! tenting, blind and buried spans; issue #130) is checked by the notes it gives.

use altium_pcbdoc::Altium;
use board_model::{CopperItem, InputFormat, ReadOutcome, Severity};

const TOLERANCE: f64 = 0.001;

fn read(name: &str) -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards")
        .join(name);
    Ok(Altium.read(&std::fs::read(path)?)?)
}

/// Every via of a board as (x, y, diameter, drill), sorted by x then y.
fn vias(out: &ReadOutcome) -> Vec<(f64, f64, f64, f64)> {
    let mut v: Vec<(f64, f64, f64, f64)> = out
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Via {
                at,
                diameter,
                drill,
            } => Some((at.x, at.y, *diameter, *drill)),
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    v
}

fn same(got: &[(f64, f64, f64, f64)], want: &[(f64, f64, f64, f64)]) -> bool {
    got.len() == want.len()
        && got.iter().zip(want).all(|(g, w)| {
            (g.0 - w.0).abs() <= TOLERANCE
                && (g.1 - w.1).abs() <= TOLERANCE
                && (g.2 - w.2).abs() <= TOLERANCE
                && (g.3 - w.3).abs() <= TOLERANCE
        })
}

#[test]
fn via_sizes_are_read_as_altium_reports_them() -> Result<(), Box<dyn std::error::Error>> {
    let want = [
        (8.0, 15.0, 0.4, 0.2),
        (16.0, 15.0, 0.6, 0.3),
        (24.0, 15.0, 0.8, 0.4),
        (32.0, 15.0, 1.0, 0.5),
        (41.0, 15.0, 1.5, 0.8),
    ];
    for board in ["vias-sizes.PcbDoc", "vias-tented-rule.PcbDoc"] {
        let got = vias(&read(board)?);
        assert!(same(&got, &want), "{board}: {got:?}");
    }
    Ok(())
}

#[test]
fn the_tenting_board_has_its_five_vias() -> Result<(), Box<dyn std::error::Error>> {
    let got = vias(&read("vias-tenting.PcbDoc")?);
    let want: Vec<(f64, f64, f64, f64)> = [8.0, 16.0, 24.0, 32.0, 40.0]
        .iter()
        .map(|&x| (x, 15.0, 0.6, 0.3))
        .collect();
    assert!(same(&got, &want), "{got:?}");
    Ok(())
}

#[test]
fn blind_and_buried_vias_are_named_in_a_note() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("vias-blind-buried.PcbDoc")?;
    let got = vias(&out);
    let want = [
        (10.0, 25.0, 0.6, 0.3),
        (20.0, 25.0, 0.6, 0.3),
        (30.0, 15.0, 0.6, 0.3),
        (30.0, 25.0, 0.6, 0.3),
        (40.0, 25.0, 0.6, 0.3),
    ];
    assert!(same(&got, &want), "{got:?}");
    // Today the four that do not run from top to bottom are read as through vias, with a note.
    assert!(
        out.notes.iter().any(|n| n.severity == Severity::Warning
            && n.message
                .starts_with("4 via(s) do not run from the top to the bottom")),
        "{:?}",
        out.notes
    );
    Ok(())
}
