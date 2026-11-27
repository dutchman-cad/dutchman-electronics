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
        // The generator is the only program named, and no user name or company is set. The one
        // exception is k10-text-vars, whose title block (made-up values) is the feature under test.
        if board.ends_with("k10-text-vars.kicad_pcb") {
            assert_eq!(text.matches("(company").count(), 1);
            assert!(text.contains("(company \"Dutchman Test\")"));
        } else {
            assert!(!text.contains("(company"));
            assert!(!text.contains("(title_block"));
        }
    }
}

// Boards made by others under an open license live in tests/open-hardware, one folder each.
// Only these folders may carry a license line other than the CC0 one above, and only one of the
// licenses we accept (CONTRIBUTING.md): CC0-1.0, CC-BY-4.0 and CC-BY-SA-4.0, version 4.0 of the
// two attribution licenses. Each folder holds the board, the license text as LICENSE-<id>.txt,
// and a README.txt whose first line is `Openly licensed board by others, not CC0. License: <id>,
// <url>` and that names the source, the commit and (for BY and BY-SA) the attribution.

/// The licenses accepted for boards by others: (id, the URL after the id in the README.txt line).
const ACCEPTED: [(&str, &str); 3] = [
    (
        "CC0-1.0",
        "https://creativecommons.org/publicdomain/zero/1.0/",
    ),
    ("CC-BY-4.0", "https://creativecommons.org/licenses/by/4.0/"),
    (
        "CC-BY-SA-4.0",
        "https://creativecommons.org/licenses/by-sa/4.0/",
    ),
];

const OPEN_LINE: &str = "Openly licensed board by others, not CC0. License: ";

fn open_hardware_folders() -> Vec<std::path::PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/open-hardware");
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    v
}

fn boards_in(folder: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(folder)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("kicad_pcb"))
        .collect()
}

#[test]
fn there_is_at_least_one_open_hardware_board() {
    assert!(!open_hardware_folders().is_empty());
}

#[test]
fn every_open_hardware_folder_names_an_accepted_license_its_source_and_its_attribution() {
    for folder in open_hardware_folders() {
        let name = folder.display();
        assert_eq!(boards_in(&folder).len(), 1, "{name}: exactly one board");
        let readme = std::fs::read_to_string(folder.join("README.txt"))
            .unwrap_or_else(|_| panic!("{name} has no README.txt"));
        let first = readme.lines().next().unwrap_or_default();
        let license = first
            .strip_prefix(OPEN_LINE)
            .unwrap_or_else(|| panic!("{name}/README.txt must start with: {OPEN_LINE}<id>, <url>"));
        let (id, _) = ACCEPTED
            .iter()
            .find(|(id, url)| license == format!("{id}, {url}"))
            .unwrap_or_else(|| panic!("{name}: license line {license:?} is not an accepted one"));
        // The license text, as the project published it.
        let text = std::fs::read_to_string(folder.join(format!("LICENSE-{id}.txt")))
            .unwrap_or_else(|_| panic!("{name} has no LICENSE-{id}.txt"));
        assert!(text.len() > 1000, "{name}: the license text looks empty");
        // The source and the exact commit the file came from.
        assert!(
            readme.lines().any(|l| l.starts_with("Source: https://")),
            "{name}: no `Source: https://...` line"
        );
        let has_commit = readme
            .split(|c: char| !c.is_ascii_hexdigit())
            .any(|w| w.len() == 40);
        assert!(
            has_commit,
            "{name}: the README.txt must name the commit (40 hex digits)"
        );
        // CC BY and CC BY-SA ask for attribution; the README.txt must give it.
        if *id != "CC0-1.0" {
            assert!(
                readme.lines().any(|l| l.starts_with("Attribution")),
                "{name}: no `Attribution` line"
            );
        }
        // Key numbers, from KiCad itself.
        for key in ["Outline", "Thickness", "Copper layers", "Footprints"] {
            assert!(
                readme.lines().any(|l| l.starts_with(&format!("- {key}"))),
                "{name}: no `- {key}` line"
            );
        }
        // Written for the KiCad 9 file format.
        let board = std::fs::read_to_string(&boards_in(&folder)[0]).unwrap();
        assert!(
            board
                .strip_prefix("(kicad_pcb")
                .is_some_and(|rest| rest.trim_start().starts_with("(version 2024")),
            "{name}: not a KiCad 9 file"
        );
    }
}

#[test]
fn no_open_hardware_board_names_a_path_an_address_or_a_computer() {
    for folder in open_hardware_folders() {
        let board = &boards_in(&folder)[0];
        let text = std::fs::read_to_string(board).unwrap();
        for (n, line) in text.lines().enumerate() {
            let l = line.to_ascii_lowercase();
            // An e-mail address has a word character on both sides of the @ (a footprint named
            // "@HOLE0" does not).
            let bytes = l.as_bytes();
            let email = bytes.iter().enumerate().any(|(i, b)| {
                *b == b'@'
                    && i > 0
                    && bytes.get(i + 1).is_some_and(|c| c.is_ascii_alphanumeric())
                    && bytes.get(i - 1).is_some_and(|c| c.is_ascii_alphanumeric())
            });
            let bad = email
                || l.contains("/home/")
                || l.contains("/users/")
                || l.contains(":\\")
                || l.contains("\\\\")
                || l.contains("/var/");
            assert!(!bad, "{}:{}: {line}", board.display(), n + 1);
        }
    }
}
