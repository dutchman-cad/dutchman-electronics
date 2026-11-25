//! The test project `param-project`: its project file, and texts that refer to its parameters.
//! Checked against tests/boards/param-project/README.txt.

use altium_pcbdoc::{project, Altium};
use board_model::{InputFormat, ReadOptions, ReadOutcome, Severity};

fn folder() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards/param-project")
}

fn the_project() -> Result<project::Project, Box<dyn std::error::Error>> {
    Ok(project::read(&std::fs::read(
        folder().join("param-project.PrjPcb"),
    )?)?)
}

fn read(board: &str, options: &ReadOptions) -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    Ok(Altium.read_with(&std::fs::read(folder().join(board))?, options)?)
}

/// (stored text, what Altium shows), from README.txt; Altium's own `.Layer_Name` is left out.
const PARAM_BOARD: [(&str, &str); 17] = [
    (".PCBNumber", "DN-0042"),
    (".BoardRevision", "B"),
    ("Rev .BoardRevision", "Rev B"),
    (".pcbnumber", "DN-0042"),
    (".NoSuchParameter", ".NoSuchParameter"),
    ("X.PCBNumber", "XDN-0042"),
    (".PCBNumber/.BoardRevision", ".PCBNumber/.BoardRevision"),
    (".PCBNumberX", ".PCBNumberX"),
    ("=PCBNumber", "=PCBNumber"),
    (".BoardRevision.", ".BoardRevision."),
    ("..PCBNumber", "..PCBNumber"),
    (".PCBNumber", "DN-0042"),
    (".PCBNumber rev", ".PCBNumber rev"),
    ("a.b.PCBNumber", "a.bDN-0042"),
    ("a/.PCBNumber", "a/DN-0042"),
    ("x..PCBNumber", "x.DN-0042"),
    (".x.PCBNumber", ".x.PCBNumber"),
];

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

fn contents(out: &ReadOutcome) -> Vec<String> {
    sorted(out.board.texts.iter().map(|t| t.content.clone()).collect())
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
    Ok(())
}

#[test]
fn with_the_projects_parameters_the_texts_show_what_altium_shows(
) -> Result<(), Box<dyn std::error::Error>> {
    let options = ReadOptions {
        parameters: the_project()?.parameters,
    };
    let out = read("param-board.PcbDoc", &options)?;
    let mut want: Vec<String> = PARAM_BOARD.iter().map(|(_, s)| s.to_string()).collect();
    want.push(".Layer_Name".into());
    assert_eq!(contents(&out), sorted(want));
    let info: Vec<_> = out
        .notes
        .iter()
        .filter(|n| n.message.contains("refer to a project parameter and show"))
        .map(|n| (n.severity, n.message.as_str()))
        .collect();
    assert_eq!(
        info,
        [(
            Severity::Info,
            "9 text(s) refer to a project parameter and show its value, as Altium does"
        )]
    );
    // The eight texts that start with "." and stay as they are, in one note.
    let left: Vec<_> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Warning)
        .collect();
    assert_eq!(left.len(), 1, "{left:?}");
    assert!(left.iter().all(|n| n.message.contains("and 3 more")));

    let second = read("param-board-2.PcbDoc", &options)?;
    assert_eq!(
        contents(&second),
        sorted(vec!["Board 2 rev B".into(), "DN-0042".into()])
    );
    assert!(second.notes.iter().all(|n| n.severity == Severity::Info));
    Ok(())
}

#[test]
fn without_parameters_the_texts_stay_as_stored_with_a_warning(
) -> Result<(), Box<dyn std::error::Error>> {
    let out = Altium.read(&std::fs::read(folder().join("param-board-2.PcbDoc"))?)?;
    assert_eq!(
        contents(&out),
        sorted(vec![
            "Board 2 rev .BoardRevision".into(),
            ".PCBNumber".into()
        ])
    );
    let warnings: Vec<_> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Warning)
        .map(|n| n.message.as_str())
        .collect();
    // Only a text that starts with "." is named; "Board 2 rev .BoardRevision" could be plain
    // text with a dot in it.
    assert_eq!(
        warnings,
        [
            "text(s) \".PCBNumber\" may be Altium special strings that refer to a project \
          parameter or to the board; no value was given for them, so they are drawn as written"
        ]
    );
    Ok(())
}

#[test]
fn a_parameter_given_by_hand_works_like_a_project_parameter(
) -> Result<(), Box<dyn std::error::Error>> {
    let options = ReadOptions {
        parameters: vec![("boardrevision".into(), "C".into())],
    };
    let out = read("param-board-2.PcbDoc", &options)?;
    assert_eq!(
        contents(&out),
        sorted(vec!["Board 2 rev C".into(), ".PCBNumber".into()])
    );
    Ok(())
}
