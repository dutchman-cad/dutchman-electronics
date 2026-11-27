//! Pads with mask settings of their own (tests/boards/README.txt, "Pad mask overrides"): the
//! reader puts them on the pads. P2 to P6 and P8 have a manual solder mask expansion (P8 from the
//! hole's edge), P4 and P6 are tented and P7 has a manual paste expansion: seven pads. No other
//! test board has a pad like that.

use altium_pcbdoc::Altium;
use board_model::{CopperItem, InputFormat, Severity};
use std::path::Path;

const NOTES: [&str; 2] = [
    "measure their solder mask expansion",
    "paste mask expansion of their own",
];

#[test]
fn pads_carry_their_own_mask_settings() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards/pad-mask-overrides.PcbDoc");
    let out = Altium.read(&std::fs::read(path)?)?;
    let mut tented = 0;
    let mut expanded = 0;
    for item in &out.board.copper {
        if let CopperItem::Pad {
            tented: t,
            mask_expansion: e,
            ..
        } = item
        {
            tented += usize::from(t.contains(&Some(true)));
            expanded += usize::from(e.iter().any(Option::is_some));
        }
    }
    assert_eq!((tented, expanded), (2, 6), "{:?}", out.board.copper);
    let hole_edge: Vec<_> = out
        .notes
        .iter()
        .filter(|n| n.message.contains("from the hole's edge"))
        .collect();
    let [hole_edge] = hole_edge.as_slice() else {
        return Err(format!("{:?}", out.notes).into());
    };
    assert_eq!(hole_edge.severity, Severity::Warning);
    assert_eq!(
        hole_edge.message,
        "1 pad(s) measure their solder mask expansion from the hole's edge; it is applied from \
         the pad's edge"
    );
    assert!(out.notes.iter().any(|n| n.severity == Severity::Info
        && n.message
            == "1 pad(s) have a paste mask expansion of their own; the paste mask is not drawn"));
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
                out.notes
                    .iter()
                    .all(|n| NOTES.iter().all(|k| !n.message.contains(k))),
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
