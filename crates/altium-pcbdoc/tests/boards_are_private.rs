//! Every test board and test project in the repository must be free of paths, e-mail
//! addresses and computer names (and a project of printer names and links out of its folder). Personal words (names, companies) are checked locally with
//! `dutchman-electronics privacy-check --watch`, never written into the repository.

use std::path::Path;

use altium_pcbdoc::privacy::{check, check_project, Options};

#[test]
fn committed_boards_carry_no_identifying_text() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards");
    let mut leaks = Vec::new();
    for entry in walk(&dir)? {
        let extension = entry
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        let bytes = std::fs::read(&entry)?;
        let report = if extension.eq_ignore_ascii_case("pcbdoc") {
            check(&bytes, &Options::default())?
        } else if extension.eq_ignore_ascii_case("prjpcb") {
            check_project(&bytes, &Options::default())?
        } else {
            continue;
        };
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
    assert!(
        leaks.is_empty(),
        "identifying text in test boards:\n{}",
        leaks.join("\n")
    );
    Ok(())
}

/// The line every test board's README.txt starts with (CONTRIBUTING.md).
const DECLARATION: &str = "Original test board made for Dutchman Electronics. \
Released under CC0 1.0 (public domain): https://creativecommons.org/publicdomain/zero/1.0/";

#[test]
fn committed_boards_carry_the_declaration() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards");
    let problems: Vec<String> = walk(&dir)?
        .iter()
        .filter(|p| is_board(p))
        .filter_map(|p| declaration_problem(p))
        .collect();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    Ok(())
}

/// The second line of tests/boards/README.txt names the boards whose embedded font files are not
/// CC0 (they keep their own license, LICENSE-DejaVu.txt). It must name exactly the boards that
/// hand over an embedded font, so a new board with a font cannot slip past the exception.
#[test]
fn the_font_exception_names_every_board_with_an_embedded_font(
) -> Result<(), Box<dyn std::error::Error>> {
    use board_model::InputFormat;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards");
    let readme = std::fs::read_to_string(dir.join("README.txt"))?;
    let exception = readme.lines().nth(1).unwrap_or_default();
    assert!(exception.starts_with("Exception: "), "{exception}");
    let named = |stem: &str| {
        exception
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .any(|word| word == stem)
    };
    let mut wrong = Vec::new();
    for path in walk(&dir)?.iter().filter(|p| is_board(p)) {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let has_font = !altium_pcbdoc::Altium
            .read(&std::fs::read(path)?)?
            .fonts
            .is_empty();
        if has_font != named(&stem) {
            wrong.push(format!(
                "{stem}: embedded font {has_font}, named {}",
                !has_font
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert!(named("one-dejavu-text"));
    assert!(dir.join("LICENSE-DejaVu.txt").is_file());
    Ok(())
}

#[test]
fn the_declaration_check_tells_good_from_missing() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("de-declaration-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let good = dir.join("good.PcbDoc");
    let bad = dir.join("bad.PcbDoc");
    std::fs::write(&good, b"")?;
    std::fs::write(&bad, b"")?;
    std::fs::write(
        dir.join("good.README.txt"),
        format!("{DECLARATION}\n\n50 x 30 mm, 1.6 mm thick.\n"),
    )?;
    std::fs::write(dir.join("bad.README.txt"), "50 x 30 mm\n")?;
    let good_result = declaration_problem(&good);
    let bad_result = declaration_problem(&bad);
    std::fs::remove_dir_all(&dir)?;
    assert_eq!(good_result, None);
    assert!(bad_result.is_some_and(|p| p.contains("does not start with")));
    Ok(())
}

fn is_board(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pcbdoc"))
}

/// None when the board has a README.txt (`<name>.README.txt` beside it, or `README.txt` in
/// its folder) whose first line is the declaration; otherwise what is wrong.
fn declaration_problem(board: &Path) -> Option<String> {
    let beside = board.with_extension("README.txt");
    let folder = board.with_file_name("README.txt");
    let readme = [beside, folder].into_iter().find(|p| p.exists());
    let Some(readme) = readme else {
        return Some(format!("{}: no README.txt", board.display()));
    };
    let text = std::fs::read_to_string(&readme).unwrap_or_default();
    let first = text.lines().next().unwrap_or_default().trim();
    if first == DECLARATION {
        None
    } else {
        Some(format!(
            "{}: does not start with the declaration line from CONTRIBUTING.md",
            readme.display()
        ))
    }
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
