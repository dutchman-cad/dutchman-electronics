//! A TrueType font file into the text outline code (`overlay/text.rs`): a real test board whose
//! texts are drawn in an embedded font, with the font's bytes replaced by the fuzz input.
#![no_main]

use std::sync::OnceLock;

use board_model::{Board, InputFormat, TextFont};
use dutchman_electronics::overlay::{overlays, Copper};
use libfuzzer_sys::fuzz_target;

/// A board with a TrueType text and the (unchanged) font it embeds.
fn seed() -> &'static (Board, board_model::EmbeddedFont) {
    static SEED: OnceLock<(Board, board_model::EmbeddedFont)> = OnceLock::new();
    SEED.get_or_init(|| {
        let bytes = include_bytes!("../../crates/altium-pcbdoc/tests/boards/one-dejavu-text.PcbDoc");
        let outcome = altium_pcbdoc::Altium.read(bytes).expect("seed board reads");
        let mut board = outcome.board;
        for text in &mut board.texts {
            text.content = "Hg8@\u{e9}\u{4e2d}fi\nWi,.".into();
            text.height = 2.0;
        }
        let font = outcome.fonts.into_iter().next().expect("seed font");
        (board, font)
    })
}

fuzz_target!(|data: &[u8]| {
    quiet_caught_panics();
    let (board, font) = seed();
    let mut font = font.clone();
    font.data = data.to_vec();
    let mut board = board.clone();
    // Draw every text in this font, whatever the board named, and also with a missing one.
    for text in &mut board.texts {
        text.font = TextFont::TrueType {
            name: font.family.clone(),
            bold: font.bold,
            italic: font.italic,
        };
    }
    let result = std::panic::catch_unwind(|| {
        let _ = overlays(&board, &[font], 1.6, "Board", Copper::Outer);
    });
    if result.is_err() {
        std::process::abort();
    }
});

/// See `package_build.rs`.
fn quiet_caught_panics() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| eprintln!("panic: {info}")));
    });
}
