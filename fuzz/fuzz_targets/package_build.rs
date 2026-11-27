//! A board file (either format) through the whole conversion: read, link models, write the
//! substrate STEP and build every overlay (copper on all layers, mask, silkscreen, plating).
//! Seeds are valid boards, so the mutations reach the code after the reader.
#![no_main]

use std::path::Path;

use dutchman_electronics::overlay::Copper;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    quiet_caught_panics();
    // No extension: the format is detected from the bytes. The folder does not exist, so no
    // linked model file is found.
    let path = Path::new("/nonexistent-fuzz-folder/board");
    for copper in [Copper::Outer, Copper::All] {
        let result = std::panic::catch_unwind(|| {
            let _ = dutchman_electronics::package::build(path, data, &[], &[], copper);
        });
        if result.is_err() {
            std::process::abort();
        }
    }
});

/// libFuzzer's hook aborts on any panic, even one the code catches on purpose (the glyph union
/// guards against a panic in its geometry library). Replace it with one that only prints, and
/// let the harness abort on a panic that gets out.
fn quiet_caught_panics() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| eprintln!("panic: {info}")));
    });
}
