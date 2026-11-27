//! The whole bytes of an Altium `.PcbDoc` (an OLE container) into the reader. It must return a
//! board or an error: never panic, never run away.
#![no_main]

use board_model::InputFormat;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = altium_pcbdoc::list_streams(data);
    let _ = altium_pcbdoc::Altium.read(data);
});
