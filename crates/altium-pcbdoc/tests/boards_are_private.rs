//! Every test board in the repository must be free of paths, e-mail addresses and computer
//! names. Personal words (names, companies) are checked locally with
//! `dutchman-electronics privacy-check --watch`, never written into the repository.

use std::path::Path;

use altium_pcbdoc::privacy::{check, Options};

#[test]
fn committed_boards_carry_no_identifying_text() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards");
    let mut leaks = Vec::new();
    for entry in walk(&dir)? {
        let is_board = entry
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pcbdoc"));
        if !is_board {
            continue;
        }
        let report = check(&std::fs::read(&entry)?, &Options::default())?;
        for f in report.findings {
            leaks.push(format!(
                "{}: {} at {}: {:?} {}",
                entry.display(),
                f.stream,
                f.offset,
                f.kind,
                f.text
            ));
        }
    }
    assert!(leaks.is_empty(), "identifying text in test boards:\n{}", leaks.join("\n"));
    Ok(())
}

fn walk(dir: &Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    if !dir.exists() {
        return Ok(files);
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            files.extend(walk(&path)?);
        } else {
            files.push(path);
        }
    }
    Ok(files)
}
