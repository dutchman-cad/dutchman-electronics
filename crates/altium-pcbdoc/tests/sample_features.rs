//! Features seen on public sample boards, rebuilt on our own CC0 boards (tests/boards/README.txt,
//! "Features seen on other people's boards"): filled shapes on the silkscreen, a moved board
//! origin and a solid 3D mask. Numbers to 0.001 mm, as AGENTS.md asks.

use altium_pcbdoc::Altium;
use board_model::{CopperItem, InputFormat, ReadOutcome, Severity};

const TOLERANCE: f64 = 0.001;

fn read(name: &str) -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards")
        .join(name);
    Ok(Altium.read(&std::fs::read(path)?)?)
}

/// overlay-region-fill: a region and two fills on each overlay. The board model has no filled
/// silkscreen shape yet, so they are left out, each layer with a note that says why (not, as
/// before, "not a copper or overlay layer").
#[test]
fn filled_shapes_on_the_overlay_are_left_out_with_the_right_reason(
) -> Result<(), Box<dyn std::error::Error>> {
    let out = read("overlay-region-fill.PcbDoc")?;
    assert!(out.board.graphics.is_empty(), "{:?}", out.board.graphics);
    assert!(
        !out.board
            .copper
            .iter()
            .any(|c| matches!(c, CopperItem::Region { .. })),
        "{:?}",
        out.board.copper
    );
    let mut dropped: Vec<&str> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Dropped)
        .map(|n| n.message.as_str())
        .collect();
    dropped.sort_unstable();
    let why = "left out: a filled silkscreen shape has no place in the board model yet";
    assert_eq!(
        dropped,
        [
            format!("1 region(s) on Bottom Overlay {why}"),
            format!("1 region(s) on Top Overlay {why}"),
            format!("2 fill(s) on Bottom Overlay {why}"),
            format!("2 fill(s) on Top Overlay {why}"),
        ]
    );
    Ok(())
}
