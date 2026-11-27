//! The text of an Altium `.PrjPcb` project into its reader, and its link check.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(project) = altium_pcbdoc::project::read(data) {
        for board in &project.boards {
            let _ = altium_pcbdoc::project::link_problem(board);
        }
    }
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = altium_pcbdoc::project::link_problem(text);
    }
});
