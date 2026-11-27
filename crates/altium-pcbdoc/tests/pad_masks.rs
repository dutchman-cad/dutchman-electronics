//! Pads with mask settings of their own (tests/boards/README.txt, "Pad mask overrides"): the
//! board model has no place for them yet, so the reader counts them in one note. P2 to P6 and P8
//! have a manual solder mask expansion (P8 from the hole's edge), P4 and P6 are tented and P7 has
//! a manual paste expansion: seven pads. No other test board has a pad like that.

use altium_pcbdoc::Altium;
use board_model::{InputFormat, Severity};
use std::path::Path;

const NOTE: &str = "pad(s) have their own mask settings";

#[test]
fn pads_with_their_own_mask_are_counted_in_a_note() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards/pad-mask-overrides.PcbDoc");
    let out = Altium.read(&std::fs::read(path)?)?;
    let notes: Vec<_> = out
        .notes
        .iter()
        .filter(|n| n.message.contains(NOTE))
        .collect();
    let [note] = notes.as_slice() else {
        return Err(format!("{:?}", out.notes).into());
    };
    assert_eq!(note.severity, Severity::Warning);
    assert_eq!(
        note.message,
        "7 pad(s) have their own mask settings (2 tented; 6 with a solder mask expansion of their \
         own, 1 of them from the hole's edge; 1 with a paste mask expansion of their own); the \
         board model has no place for them yet, so these pads are drawn by the board-wide rule"
    );
    Ok(())
}

#[test]
fn no_other_board_has_a_pad_with_its_own_mask() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards");
    let mut folders = vec![dir];
    let mut read = 0;
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "PcbDoc")
                || path
                    .file_name()
                    .is_some_and(|n| n == "pad-mask-overrides.PcbDoc")
            {
                continue;
            }
            let out = Altium.read(&std::fs::read(&path)?)?;
            assert!(
                out.notes.iter().all(|n| !n.message.contains(NOTE)),
                "{}: {:?}",
                path.display(),
                out.notes
            );
            read += 1;
        }
    }
    assert!(read > 50, "only {read} boards read");
    Ok(())
}
