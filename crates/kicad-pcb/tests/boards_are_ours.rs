//! Every test board is original and carries the CC0 line (CONTRIBUTING.md), and holds no path,
//! e-mail address or computer name.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::indexing_slicing
)]

use std::path::Path;

const DECLARATION: &str = "Original test board made for Dutchman Electronics. \
Released under CC0 1.0 (public domain): https://creativecommons.org/publicdomain/zero/1.0/";

fn boards() -> Vec<std::path::PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards");
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("kicad_pcb"))
        .collect();
    v.sort();
    v
}

#[test]
fn every_board_has_a_readme_with_the_declaration() {
    for board in boards() {
        let readme = board.with_extension("README.txt");
        let text = std::fs::read_to_string(&readme)
            .unwrap_or_else(|_| panic!("{} has no README.txt", board.display()));
        assert!(
            text.starts_with(DECLARATION),
            "{} must start with the CC0 line",
            readme.display()
        );
    }
}

#[test]
fn no_board_names_a_path_an_address_or_a_computer() {
    for board in boards() {
        let text = std::fs::read_to_string(&board).unwrap();
        for (n, line) in text.lines().enumerate() {
            let l = line.to_ascii_lowercase();
            let bad = l.contains("/home/")
                || l.contains("/users/")
                || l.contains(":\\")
                || l.contains("\\\\")
                || l.contains("@")
                || l.contains("/var/");
            assert!(!bad, "{}:{}: {line}", board.display(), n + 1);
        }
        // The generator is the only program named, and no user name or company is set.
        assert!(!text.contains("(company"));
        assert!(!text.contains("(title_block"));
    }
}
