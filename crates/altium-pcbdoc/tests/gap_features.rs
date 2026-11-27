//! Features no board had before (tests/boards/README.txt, "Feature gaps, first batch"): keepouts
//! and pad holes that are not round. Numbers to 0.001 mm, as AGENTS.md asks.

use altium_pcbdoc::Altium;
use board_model::{CopperItem, InputFormat, ReadOutcome, Severity};

const TOLERANCE: f64 = 0.001;

fn read(name: &str) -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards")
        .join(name);
    Ok(Altium.read(&std::fs::read(path)?)?)
}

/// keepouts: a track, arc, fill and region on the Keep-Out layer, and a track, fill and region on
/// Top Layer with the Keepout box ticked. None of them is copper: the only copper is the plain
/// Top Layer track (5, 15) to (15, 15), width 0.300. One Info note per kind and layer.
#[test]
fn keepouts_are_not_copper() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("keepouts.PcbDoc")?;
    let [CopperItem::Track {
        from, to, width, ..
    }] = out.board.copper.as_slice()
    else {
        return Err(format!("{:?}", out.board.copper).into());
    };
    let near = |a: f64, b: f64| (a - b).abs() <= TOLERANCE;
    assert!(near(from.x, 5.0) && near(from.y, 15.0), "{from:?}");
    assert!(near(to.x, 15.0) && near(to.y, 15.0), "{to:?}");
    assert!(near(*width, 0.3), "{width}");
    assert!(out.board.graphics.is_empty(), "{:?}", out.board.graphics);

    let mut info: Vec<&str> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Info && n.message.contains("keepout"))
        .map(|n| n.message.as_str())
        .collect();
    info.sort_unstable();
    let layer = "on Keep-Out Layer left out: a keepout is a rule for placing and routing, not \
                 part of the board";
    let flagged = "on Top Layer are keepouts (Altium's Keepout box), not copper; left out";
    assert_eq!(
        info,
        [
            format!("1 arc(s) {layer}"),
            format!("1 fill(s) {layer}"),
            format!("1 fill(s) {flagged}"),
            format!("1 region(s) {layer}"),
            format!("1 region(s) {flagged}"),
            format!("1 track(s) {layer}"),
            format!("1 track(s) {flagged}"),
        ]
    );
    assert!(
        out.notes
            .iter()
            .all(|n| n.severity != Severity::Dropped && n.severity != Severity::Warning),
        "{:?}",
        out.notes
    );
    Ok(())
}
