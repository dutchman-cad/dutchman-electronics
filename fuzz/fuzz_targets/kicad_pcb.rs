//! The text of a KiCad `.kicad_pcb` (an s-expression) into the reader.
#![no_main]

use board_model::InputFormat;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = kicad_pcb::KiCad.read(data);
});
