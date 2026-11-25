//! Small boards written by hand from KiCad's published file-format documentation, for the parts
//! of the format none of our KiCad-made boards use yet (the stack-up table, old versions, items
//! the reader does not read). Real boards that hold these are still wanted; see README.md.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::indexing_slicing
)]

use board_model::{InputFormat, ReadError, Severity};
use kicad_pcb::KiCad;

fn read(text: &str) -> Result<board_model::ReadOutcome, ReadError> {
    KiCad.read(text.as_bytes())
}

const HEAD: &str = "(kicad_pcb (version 20241229) (generator \"pcbnew\") (generator_version \"9.0\")\n\
  (general (thickness 1.6))\n\
  (layers (0 \"F.Cu\" signal) (2 \"B.Cu\" signal) (25 \"Edge.Cuts\" user) (17 \"Dwgs.User\" user))\n";

#[test]
fn the_stackup_gives_the_thickness() {
    let text = format!(
        "{HEAD}(setup (stackup\n\
          (layer \"F.Mask\" (type \"Top Solder Mask\") (thickness 0.01))\n\
          (layer \"F.Cu\" (type \"copper\") (thickness 0.035))\n\
          (layer \"dielectric 1\" (type \"core\") (thickness 1.51) (material \"FR4\"))\n\
          (layer \"B.Cu\" (type \"copper\") (thickness 0.035))\n\
          (layer \"B.Mask\" (type \"Bottom Solder Mask\") (thickness 0.01))\n\
          (layer \"F.SilkS\" (type \"Top Silk Screen\"))))\n)"
    );
    let o = read(&text).unwrap();
    assert!((o.board.thickness.unwrap() - 1.6).abs() < 1e-9);
    assert!(!o.notes.iter().any(|n| n.message.contains("stack-up adds up")));
}

#[test]
fn a_stackup_that_disagrees_with_the_general_thickness_wins_with_a_warning() {
    let text = format!(
        "{HEAD}(setup (stackup\n\
          (layer \"F.Cu\" 1 (type \"copper\") (thickness 0.035))\n\
          (layer \"dielectric 1\" 2 (type \"core\") (thickness 0.5))\n\
          (layer \"B.Cu\" 3 (type \"copper\") (thickness 0.035))))\n)"
    );
    let o = read(&text).unwrap();
    assert!((o.board.thickness.unwrap() - 0.57).abs() < 1e-9);
    assert!(o
        .notes
        .iter()
        .any(|n| n.severity == Severity::Warning && n.message.contains("stack-up adds up to 0.570")));
}

#[test]
fn an_older_file_gets_a_note_and_no_board() {
    let o = read("(kicad_pcb (version 20240108) (generator \"pcbnew\") (layers (0 \"F.Cu\" signal)))")
        .unwrap();
    assert!(o.board.outline.outer.segments.is_empty());
    assert!(o.board.layers.is_empty());
    assert_eq!(o.notes.len(), 1);
    assert_eq!(o.notes[0].severity, Severity::Dropped);
    assert!(o.notes[0].message.contains("older than 9"));
    let o = read("(kicad_pcb (generator \"pcbnew\"))").unwrap();
    assert!(o.notes[0].message.contains("no version"));
}

#[test]
fn a_newer_version_is_read() {
    let text = "(kicad_pcb (version 20261231) (generator \"pcbnew\") (generator_version \"10.0\")\n\
      (layers (0 \"F.Cu\" signal) (25 \"Edge.Cuts\" user))\n\
      (gr_rect (start 0 0) (end 10 5) (stroke (width 0.1) (type default)) (fill no) (layer \"Edge.Cuts\")))";
    let o = read(text).unwrap();
    assert_eq!(o.board.outline.outer.segments.len(), 4);
}

#[test]
fn what_is_not_read_is_named() {
    let text = format!(
        "{HEAD}(dimension (type aligned))\n\
         (gr_line (start 0 0) (end 1 1) (layer \"Dwgs.User\") (stroke (width 0.1) (type default)))\n\
         (zone (net 0) (net_name \"\") (layer \"F.Cu\") (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1))))\n\
         (zone (keepout (tracks not_allowed)) (layer \"F.Cu\"))\n)"
    );
    let o = read(&text).unwrap();
    let dropped: Vec<&str> = o
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Dropped)
        .map(|n| n.message.as_str())
        .collect();
    let has = |w: &str| dropped.iter().any(|m| m.contains(w));
    assert!(has("\"dimension\""), "{dropped:?}");
    assert!(has("layer Dwgs.User"), "{dropped:?}");
    assert!(has("no filled copper"), "{dropped:?}");
    assert!(has("keep-out"), "{dropped:?}");
    assert!(has("no closed outline") || o.notes.iter().any(|n| n.message.contains("no closed outline")));
}

#[test]
fn an_outline_that_does_not_close_is_left_out_with_a_note() {
    let text = format!(
        "{HEAD}(gr_line (start 0 0) (end 10 0) (layer \"Edge.Cuts\") (stroke (width 0.1) (type default)))\n\
         (gr_line (start 10 0) (end 10 10) (layer \"Edge.Cuts\") (stroke (width 0.1) (type default)))\n)"
    );
    let o = read(&text).unwrap();
    assert!(o.board.outline.outer.segments.is_empty());
    assert!(o.notes.iter().any(|n| n.message.contains("do not close")));
}

#[test]
fn net_names_given_as_strings_are_read() {
    let text = format!(
        "{HEAD}(segment (start 0 0) (end 1 0) (width 0.2) (layer \"F.Cu\") (net \"GND\"))\n)"
    );
    let o = read(&text).unwrap();
    assert!(matches!(
        o.board.copper.first(),
        Some(board_model::CopperItem::Track { net: Some(n), .. }) if n == "GND"
    ));
}

#[test]
fn text_that_is_not_a_board_is_an_error() {
    assert!(matches!(read("(kicad_sch (version 1))"), Err(ReadError::Invalid(_))));
    assert!(matches!(read("hello"), Err(ReadError::Invalid(_))));
    assert!(matches!(read(""), Err(ReadError::Invalid(_))));
}

#[test]
fn a_model_turns_the_other_way_about_each_axis_and_is_not_scaled_silently() {
    let text = format!(
        "{HEAD}(footprint \"lib:U\" (layer \"F.Cu\") (at 10 10)\n\
           (property \"Reference\" \"U1\" (at 0 0 0) (layer \"F.SilkS\") (hide yes))\n\
           (model \"C:\\\\models\\\\box.step\" (offset (xyz 1 2 3)) (scale (xyz 2 1 1)) (rotate (xyz 90 0 -45))))\n)"
    );
    let o = read(&text).unwrap();
    let m = o.board.components[0].model.as_ref().unwrap();
    assert_eq!(m.name, "box.step");
    assert_eq!(m.offset, [1.0, 2.0, 3.0]);
    assert_eq!(m.rotation_deg, [-90.0, 0.0, 45.0]);
    assert!(o.notes.iter().any(|n| n.message.contains("scale")));
    // A hidden reference is not drawn.
    assert!(o.board.texts.is_empty());
}
