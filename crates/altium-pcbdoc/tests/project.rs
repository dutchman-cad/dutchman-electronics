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

/// The test board `quoted-strings` (issue 146), read with the parameters of param-project as
/// its README.txt section "Quoted special strings" says: each text, by its y, shows what Altium
/// showed.
#[test]
fn quoted_references_show_what_altium_shows() -> Result<(), Box<dyn std::error::Error>> {
    let parameters = the_project()?.parameters;
    let (out, notes) = read("quoted-strings.PcbDoc", &parameters)?;
    let want = [
        (27.5, "Top Overlay", "Rev. B"),
        (25.3, "Top Overlay", "B end"),
        (23.1, "Top Overlay", "DN-0042 Rev B"),
        (20.9, "Top Overlay", "DN-0042B"),
        (18.7, "Top Overlay", "DN-0042"),
        (16.5, "Top Overlay", "Top Overlay"),
        (14.3, "Top Overlay", "L: Top Overlay x"),
        (12.1, "Top Overlay", "'.NoSuchParameter'"),
        (9.9, "Top Overlay", "Rev 'B"),
        (7.7, "Top Overlay", "Rev ''"),
        (5.5, "Top Overlay", "'word' Rev"),
        (3.3, "Top Overlay", "Rev B end"),
        (1.1, "Bottom Overlay", "B"),
    ];
    let mut got: Vec<(f64, String, String)> = out
        .board
        .texts
        .iter()
        .map(|t| {
            let layer = out.board.layers.get(t.layer).map(|l| l.name.clone());
            (t.at.y, layer.unwrap_or_default(), t.content.clone())
        })
        .collect();
    got.sort_by(|a, b| b.0.total_cmp(&a.0));
    assert_eq!(got.len(), want.len(), "{got:?}");
    for ((y, layer, content), (wy, wl, wc)) in got.iter().zip(want) {
        assert!((y - wy).abs() <= 0.001, "{got:?}");
        assert!(out
            .board
            .texts
            .iter()
            .all(|t| (t.at.x - 2.0).abs() <= 0.001));
        assert_eq!((layer.as_str(), content.as_str()), (wl, wc), "at y {y}");
    }
    assert_eq!(
        warnings(&notes),
        [
            "1 text(s) read \".NoSuchParameter\", which names no parameter given and no special \
          string read here; drawn as written"
        ]
    );
    Ok(())
}

/// The test board `quoted-component-strings` (issue 146; README.txt "Quoted component strings"),
/// read with no parameters: R1's special strings in quotes on its own texts, on both sides, and
/// the same strings on free texts, which stay as typed. Each text found by its position.
#[test]
fn quoted_component_strings_show_what_altium_shows() -> Result<(), Box<dyn std::error::Error>> {
    let (out, notes) = read("quoted-component-strings.PcbDoc", &[])?;
    // (x, y, owned by R1, layer, mirrored, shown)
    let want = [
        (28.0, 27.0, true, "Top Overlay", false, "R1"),
        (28.0, 24.8, true, "Top Overlay", false, "R: R1"),
        (28.0, 22.6, true, "Top Overlay", false, "10k ohm"),
        (28.0, 20.4, true, "Top Overlay", false, "R1 10k"),
        (28.0, 18.2, true, "Top Overlay", false, "R1"),
        (30.0, 11.0, true, "Top Overlay", false, "L: Top Overlay"),
        (48.0, 7.0, true, "Bottom Overlay", true, "B: R1"),
        (2.0, 27.0, false, "Top Overlay", false, "'.Designator'"),
        (2.0, 24.8, false, "Top Overlay", false, "'.Comment'"),
        (2.0, 22.6, false, "Top Overlay", false, "R: '.Designator' x"),
        (48.0, 3.0, false, "Bottom Overlay", true, "Bottom Overlay x"),
    ];
    for (x, y, owned, layer, mirrored, shown) in want {
        let found: Vec<_> = out
            .board
            .texts
            .iter()
            .filter(|t| (t.at.x - x).abs() <= 0.001 && (t.at.y - y).abs() <= 0.001)
            .collect();
        let [t] = found.as_slice() else {
            return Err(format!("{} text(s) at ({x}, {y})", found.len()).into());
        };
        let name = out.board.layers.get(t.layer).map(|l| l.name.as_str());
        assert_eq!(
            (t.content.as_str(), t.component == Some(0), name, t.mirrored),
            (shown, owned, Some(layer), mirrored),
            "at ({x}, {y})"
        );
    }
    // The rest is one-resistor's own: R1's designator text, which shows "R1".
    let others: Vec<_> = out
        .board
        .texts
        .iter()
        .filter(|t| {
            !want
                .iter()
                .any(|w| (t.at.x - w.0).abs() <= 0.001 && (t.at.y - w.1).abs() <= 0.001)
        })
        .map(|t| t.content.as_str())
        .collect();
    assert_eq!(others, ["R1"]);
    assert_eq!(warnings(&notes), Vec::<&str>::new());
    let info: Vec<&str> = notes
        .iter()
        .filter(|n| n.severity == Severity::Info)
        .map(|n| n.message.as_str())
        .collect();
    assert_eq!(
        info,
        [
            "8 text(s) refer to a project parameter or to .Designator, .Comment or .Layer_Name and show its value, as Altium does",
            "3 text(s) read .Designator or .Comment but belong to no component; they are drawn as written, as Altium does",
        ]
    );
    Ok(())
}
