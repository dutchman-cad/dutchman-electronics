//! The board-wide solder mask expansion rule (tests/boards/README.txt, "Solder mask expansion"):
//! 0, 0.05, 0.15 and 0.2 mm on the `mask-expansion-*` boards, 0.1 mm on every other board but
//! `one-linked-step-resistor`, which keeps 0. Altium stores the value in mil, so 0.1 mm reads as
//! 3.937 mil = 0.0999998 mm: within 0.001 mm, as AGENTS.md asks.

use altium_pcbdoc::Altium;
use board_model::InputFormat;
use std::path::{Path, PathBuf};

const TOLERANCE: f64 = 0.001;

fn boards() -> Result<Vec<PathBuf>, std::io::Error> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards");
    let mut out = Vec::new();
    let mut folders = vec![dir];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|e| e == "PcbDoc") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

#[test]
fn every_board_reads_its_mask_expansion_rule() -> Result<(), Box<dyn std::error::Error>> {
    let mut wrong = Vec::new();
    let mut seen = 0;
    for path in boards()? {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let want = match stem.as_str() {
            "mask-expansion-0" | "one-linked-step-resistor" => 0.0,
            "mask-expansion-0-05" => 0.05,
            "mask-expansion-0-15" => 0.15,
            "mask-expansion-0-2" => 0.2,
            _ => 0.1,
        };
        let out = Altium.read(&std::fs::read(&path)?)?;
        match out.board.solder_mask_expansion {
            Some(got) if (got - want).abs() <= TOLERANCE => {}
            got => wrong.push(format!("{stem}: {got:?}, expected {want}")),
        }
        seen += 1;
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert!(seen >= 71, "only {seen} boards");
    Ok(())
}
