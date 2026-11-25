//! The test project `param-project` (its project file, and texts that refer to its parameters)
//! and the test board `special-strings` (Altium's own special strings). Checked against
//! tests/boards/param-project/README.txt and tests/boards/README.txt.

use altium_pcbdoc::special_strings::resolve_texts;
use altium_pcbdoc::{project, Altium};
use board_model::{InputFormat, Note, ReadOutcome, Severity};

fn boards() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards")
}

fn the_project() -> Result<project::Project, Box<dyn std::error::Error>> {
    Ok(project::read(&std::fs::read(
        boards().join("param-project/param-project.PrjPcb"),
    )?)?)
}

/// Reads a board and resolves its texts with `parameters`, as the command does.
fn read(
    board: &str,
    parameters: &[(String, String)],
) -> Result<(ReadOutcome, Vec<Note>), Box<dyn std::error::Error>> {
    let mut out = Altium.read(&std::fs::read(boards().join(board))?)?;
    let notes = resolve_texts(&mut out.board, parameters);
    Ok((out, notes))
}

/// What Altium shows for each text of param-board (README.txt).
const PARAM_BOARD: [&str; 18] = [
    ".BoardRevision.",
    ".NoSuchParameter",
    ".PCBNumber rev",
    ".PCBNumber/.BoardRevision",
    ".PCBNumberX",
    ".x.PCBNumber",
    "..PCBNumber",
    "=PCBNumber",
    "B",
    "DN-0042",
    "DN-0042",
    "DN-0042",
    "Rev B",
    "Top Overlay",
    "XDN-0042",
    "a.bDN-0042",
    "a/DN-0042",
    "x.DN-0042",
];

fn contents(out: &ReadOutcome) -> Vec<String> {
    let mut v: Vec<String> = out.board.texts.iter().map(|t| t.content.clone()).collect();
    v.sort();
    v
}

fn warnings(notes: &[Note]) -> Vec<&str> {
    notes
        .iter()
        .filter(|n| n.severity == Severity::Warning)
        .map(|n| n.message.as_str())
        .collect()
}

#[test]
fn the_project_lists_its_two_boards_and_two_parameters() -> Result<(), Box<dyn std::error::Error>> {
    let p = the_project()?;
    assert_eq!(p.boards, ["param-board.PcbDoc", "param-board-2.PcbDoc"]);
    assert_eq!(
        p.parameters,
        [
            ("BoardRevision".to_string(), "B".to_string()),
            ("PCBNumber".to_string(), "DN-0042".to_string())
        ]
    );
    for link in &p.boards {
        assert_eq!(project::link_problem(link), None, "{link}");
    }
    Ok(())
}

#[test]
fn with_the_projects_parameters_the_texts_show_what_altium_shows(
) -> Result<(), Box<dyn std::error::Error>> {
    let parameters = the_project()?.parameters;
    let (out, notes) = read("param-project/param-board.PcbDoc", &parameters)?;
    let mut want = PARAM_BOARD.to_vec();
    want.sort_unstable();
    assert_eq!(contents(&out), want);
    // 9 parameters and .Layer_Name; the 7 texts left that start with "." each get a note.
    assert_eq!(
        notes.first().map(|n| n.message.as_str()),
        Some(
            "10 text(s) refer to a project parameter or to .Designator, .Comment or .Layer_Name \
             and show its value, as Altium does"
        )
    );
    assert_eq!(warnings(&notes).len(), 7, "{notes:?}");

    let (second, notes) = read("param-project/param-board-2.PcbDoc", &parameters)?;
    assert_eq!(contents(&second), ["Board 2 rev B", "DN-0042"]);
    assert!(warnings(&notes).is_empty(), "{notes:?}");
    Ok(())
}

#[test]
fn without_parameters_the_texts_stay_as_stored_with_a_note(
) -> Result<(), Box<dyn std::error::Error>> {
    let (out, notes) = read("param-project/param-board-2.PcbDoc", &[])?;
    assert_eq!(contents(&out), [".PCBNumber", "Board 2 rev .BoardRevision"]);
    // Only a text that starts with "." is named; "Board 2 rev .BoardRevision" could be plain
    // text with a dot in it.
    assert_eq!(
        warnings(&notes),
        [
            "1 text(s) read \".PCBNumber\", which names no parameter given and no special string \
          read here; drawn as written"
        ]
    );
    Ok(())
}

#[test]
fn a_parameter_given_by_hand_works_like_a_project_parameter(
) -> Result<(), Box<dyn std::error::Error>> {
    let (out, _) = read(
        "param-project/param-board-2.PcbDoc",
        &[("boardrevision".into(), "C".into())],
    )?;
    assert_eq!(contents(&out), [".PCBNumber", "Board 2 rev C"]);
    Ok(())
}

#[test]
fn special_strings_show_what_altium_shows() -> Result<(), Box<dyn std::error::Error>> {
    let (out, notes) = read("special-strings.PcbDoc", &[])?;
    // R1's comment comes from its (hidden) comment text.
    let r1 = out.board.components.first().ok_or("no R1")?;
    assert_eq!((r1.designator.as_str(), r1.comment.as_str()), ("R1", "10k"));
    // (shown, belongs to R1), from README.txt; R1's own designator text "R1" is one of them.
    let mut got: Vec<(String, bool)> = out
        .board
        .texts
        .iter()
        .map(|t| (t.content.clone(), t.component == Some(0)))
        .collect();
    got.sort();
    let mut want: Vec<(String, bool)> = [
        ("R1", true),
        ("R1", true),
        ("R1", true),
        ("10k", true),
        ("R: R1", true),
        ("Top Overlay", true),
        (".Designator", false),
        (".Comment", false),
        ("Top Layer", false),
        ("L: Top Overlay", false),
        ("Top Overlay", false),
        (".Print_Date", false),
        (".NoSuchString", false),
        ("Bottom Overlay", false),
    ]
    .iter()
    .map(|(s, owned)| (s.to_string(), *owned))
    .collect();
    want.sort();
    assert_eq!(got, want);
    assert_eq!(
        warnings(&notes),
        [
            "1 text(s) read \".NoSuchString\", which names no parameter given and no special \
             string read here; drawn as written",
            "1 text(s) read \".Print_Date\", which Altium fills in with a date or time when it \
             draws the board; drawn as written, since a conversion should not stamp a date onto \
             a board",
        ]
    );
    Ok(())
}
