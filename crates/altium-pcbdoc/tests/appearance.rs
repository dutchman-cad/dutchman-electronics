//! Solder mask and silkscreen colours, checked against tests/boards/README.txt.

use altium_pcbdoc::Altium;
use board_model::{Appearance, Color, InputFormat, ReadOutcome, SurfaceFinish};

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
        ..Appearance::default()
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

/// The finish both sides of a board have.
fn finish_on_both_sides(
    name: &str,
    want: &SurfaceFinish,
) -> Result<(), Box<dyn std::error::Error>> {
    let out = read(name)?;
    let a = &out.board.appearance;
    assert_eq!(a.top_finish.as_ref(), Some(want), "{name}");
    assert_eq!(a.bottom_finish.as_ref(), Some(want), "{name}");
    assert!(out.notes.is_empty(), "{name}: {:?}", out.notes);
    Ok(())
}

#[test]
fn an_enig_surface_finish() -> Result<(), Box<dyn std::error::Error>> {
    finish_on_both_sides(
        "one-enig-resistor.PcbDoc",
        &SurfaceFinish {
            process: "ENIG".into(),
            material: "Nickel, Gold".into(),
            color: rgba(255, 196, 0, 1.0),
            thickness: Some(0.004),
        },
    )
}

#[test]
fn a_hasl_surface_finish() -> Result<(), Box<dyn std::error::Error>> {
    finish_on_both_sides(
        "one-hasl-resistor.PcbDoc",
        &SurfaceFinish {
            process: "HASL".into(),
            material: "PbSn".into(),
            color: rgba(242, 242, 242, 1.0),
            thickness: Some(0.02),
        },
    )
}

#[test]
fn boards_without_a_finish_layer_have_none() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-resistor.PcbDoc")?;
    assert_eq!(out.board.appearance.top_finish, None);
    assert_eq!(out.board.appearance.bottom_finish, None);
    Ok(())
}
