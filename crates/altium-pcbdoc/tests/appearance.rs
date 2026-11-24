//! Solder mask and silkscreen colours, checked against tests/boards/README.txt.

use altium_pcbdoc::Altium;
use board_model::{Appearance, Color, InputFormat, ReadOutcome};

fn read(name: &str) -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards")
        .join(name);
    Ok(Altium.read(&std::fs::read(path)?)?)
}

fn rgba(r: u8, g: u8, b: u8, opacity: f64) -> Option<Color> {
    Some(Color { r, g, b, opacity })
}

/// The colours a board has, with the same solder mask and the same silkscreen on both sides.
fn both_sides(mask: Option<Color>, silk: Option<Color>) -> Appearance {
    Appearance {
        top_solder_mask: mask,
        bottom_solder_mask: mask,
        top_silkscreen: silk,
        bottom_silkscreen: silk,
    }
}

const GREEN_MASK: (u8, u8, u8) = (36, 120, 50);
const WHITE_SILK: (u8, u8, u8) = (250, 250, 250);

#[test]
fn altiums_default_colours() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-resistor.PcbDoc")?;
    let (m, s) = (GREEN_MASK, WHITE_SILK);
    assert_eq!(
        out.board.appearance,
        both_sides(rgba(m.0, m.1, m.2, 0.6), rgba(s.0, s.1, s.2, 1.0))
    );
    Ok(())
}

#[test]
fn a_blue_solder_mask() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-blue-mask-resistor.PcbDoc")?;
    let s = WHITE_SILK;
    assert_eq!(
        out.board.appearance,
        both_sides(rgba(17, 85, 204, 0.6), rgba(s.0, s.1, s.2, 1.0))
    );
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn a_yellow_silkscreen() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-yellow-silkscreen-resistor.PcbDoc")?;
    let m = GREEN_MASK;
    assert_eq!(
        out.board.appearance,
        both_sides(rgba(m.0, m.1, m.2, 0.6), rgba(255, 255, 0, 1.0))
    );
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}
