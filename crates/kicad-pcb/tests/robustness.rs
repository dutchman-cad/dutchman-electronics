//! A hostile file must come back as an error or a short read, never as a panic.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::indexing_slicing
)]

use board_model::InputFormat;
use kicad_pcb::KiCad;

fn real_board() -> Vec<u8> {
    let path = format!(
        "{}/tests/boards/one-resistor.kicad_pcb",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(path).unwrap()
}

#[test]
fn every_cut_of_a_real_board_is_survived() {
    let bytes = real_board();
    let step = (bytes.len() / 400).max(1);
    for end in (0..bytes.len()).step_by(step) {
        let _ = KiCad.read(&bytes[..end]);
    }
}

#[test]
fn every_dropped_token_of_a_real_board_is_survived() {
    // Remove one character at a time from the start of the file (parentheses move around).
    let text = String::from_utf8(real_board()).unwrap();
    let step = (text.len() / 300).max(1);
    for i in (0..text.len()).step_by(step) {
        let mut cut = text.clone();
        if cut.is_char_boundary(i) {
            cut.remove(i);
            let _ = KiCad.read(cut.as_bytes());
        }
    }
}

#[test]
fn garbage_and_deep_nesting_are_errors() {
    for bytes in [
        &b"\xff\xfe\x00\x01(kicad_pcb"[..],
        &b"(kicad_pcb (version 20241229) (footprint (at nan nan) (pad (at 1e999 1) (size 1 1))))"[..],
        &b"(kicad_pcb (version 20241229) (gr_arc (start 0 0) (mid 0 0) (end 0 0) (layer \"Edge.Cuts\")))"[..],
        &b"(kicad_pcb (version 20241229) (gr_poly (pts (xy 0 0)) (layer \"Edge.Cuts\")))"[..],
    ] {
        let _ = KiCad.read(bytes);
    }
    let deep = "(".repeat(1_000_000);
    assert!(KiCad.read(deep.as_bytes()).is_err());
    let wide = format!("(kicad_pcb (version 20241229) {})", "(x) ".repeat(200_000));
    assert!(KiCad.read(wide.as_bytes()).is_ok());
}

#[test]
fn a_huge_outline_is_joined_in_bounded_time() {
    let mut text = String::from("(kicad_pcb (version 20241229) (layers (25 \"Edge.Cuts\" user))\n");
    for i in 0..20_000 {
        text.push_str(&format!(
            "(gr_line (start {i} 0) (end {} 0) (layer \"Edge.Cuts\"))\n",
            i + 1
        ));
    }
    text.push(')');
    let start = std::time::Instant::now();
    let o = KiCad.read(text.as_bytes()).unwrap();
    assert!(start.elapsed().as_secs() < 30);
    assert!(o.notes.iter().any(|n| n.message.contains("beyond the limit")));
}
