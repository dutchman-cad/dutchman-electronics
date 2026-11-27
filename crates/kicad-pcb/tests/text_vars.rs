//! KiCad's text variables against k10-text-vars: every row of its README.txt table (the stored
//! string and what KiCad 10 shows, read back with KiCad's own scripting).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use board_model::{InputFormat, ReadOutcome, Severity};
use kicad_pcb::{project_variables, read_with, KiCad, Variables};

fn file(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/boards/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// The board read as `dutchman-electronics read` reads it: with its file name and the project
/// beside it.
fn with_project() -> ReadOutcome {
    let variables = Variables {
        file_name: Some("k10-text-vars.kicad_pcb".into()),
        project: project_variables(&file("k10-text-vars.kicad_pro")).unwrap(),
    };
    read_with(&file("k10-text-vars.kicad_pcb"), &variables).unwrap()
}

/// The text on `layer` at KiCad's (x, y) on the page (model x = x, model y = -y).
fn text_at<'a>(o: &'a ReadOutcome, layer: &str, x: f64, y: f64) -> &'a str {
    let found: Vec<&str> = o
        .board
        .texts
        .iter()
        .filter(|t| {
            o.board.layers[t.layer].name == layer
                && (t.at.x - x).abs() < 0.001
                && (t.at.y + y).abs() < 0.001
        })
        .map(|t| t.content.as_str())
        .collect();
    assert_eq!(found.len(), 1, "{layer} ({x}, {y}): {found:?}");
    found[0]
}

/// README rows 1 to 20: the board's own texts, (row, layer, x, y on the page, KiCad shows).
const BOARD_ROWS: [(u32, &str, f64, f64, &str); 20] = [
    (1, "F.SilkS", 105.0, 105.0, "B2"),
    (2, "F.SilkS", 105.0, 108.0, "Rev B2"),
    (3, "F.SilkS", 105.0, 111.0, "Text Variables B2"),
    (4, "F.SilkS", 105.0, 114.0, "DN-0042"),
    (5, "F.SilkS", 105.0, 117.0, "DN-0042-B2"),
    (6, "F.SilkS", 105.0, 120.0, "F.Silkscreen"),
    (7, "F.SilkS", 105.0, 123.0, "${NOSUCHVAR}"),
    (8, "F.SilkS", 105.0, 126.0, "Rev B2"),
    (9, "F.SilkS", 105.0, 129.0, "Dutchman Test"),
    (10, "F.SilkS", 105.0, 132.0, "2026-11-27"),
    (
        11,
        "F.SilkS",
        105.0,
        135.0,
        "First comment / Second comment",
    ),
    (12, "F.SilkS", 105.0, 138.0, "CONN-2"),
    (13, "F.SilkS", 105.0, 141.0, "ABC-123"),
    (14, "F.SilkS", 105.0, 144.0, "board property"),
    (15, "F.SilkS", 105.0, 147.0, "k10-text-vars.kicad_pcb"),
    (16, "F.SilkS", 105.0, 150.0, "k10-text-vars"),
    (17, "F.SilkS", 105.0, 153.0, "${revision}"),
    (18, "F.SilkS", 105.0, 156.0, "$B2"),
    (19, "F.Cu", 105.0, 159.0, "F.Cu B2"),
    (20, "B.SilkS", 145.0, 175.0, "B.Silkscreen B2"),
];

/// README rows 21 and 23 to 36 (22 and 30 are the MPN fields): a footprint's Reference field
/// at its origin and its six texts 3 mm apart from 4 mm below it, (row, layer, x, y, shows).
const FOOTPRINT_ROWS: [(u32, &str, f64, f64, &str); 14] = [
    (21, "F.SilkS", 170.0, 120.0, "J1"),
    (23, "F.SilkS", 170.0, 124.0, "J1"),
    (24, "F.SilkS", 170.0, 127.0, "CONN-2"),
    (25, "F.SilkS", 170.0, 130.0, "ABC-123"),
    (26, "F.SilkS", 170.0, 133.0, "J1-ABC-123"),
    (27, "F.SilkS", 170.0, 136.0, "F.Silkscreen"),
    (28, "F.SilkS", 170.0, 139.0, "B2"),
    (29, "B.SilkS", 170.0, 150.0, "J2"),
    (31, "B.SilkS", 170.0, 154.0, "J2"),
    (32, "B.SilkS", 170.0, 157.0, "CONN-3"),
    (33, "B.SilkS", 170.0, 160.0, "XYZ-9"),
    (34, "B.SilkS", 170.0, 163.0, "J2-XYZ-9"),
    (35, "B.SilkS", 170.0, 166.0, "B.Silkscreen"),
    (36, "B.SilkS", 170.0, 169.0, "B2"),
];

#[test]
fn every_text_shows_what_kicad_shows() {
    let o = with_project();
    for (row, layer, x, y, shows) in BOARD_ROWS.iter().chain(&FOOTPRINT_ROWS) {
        assert_eq!(text_at(&o, layer, *x, *y), *shows, "README row {row}");
    }
}

#[test]
fn names_that_nothing_defines_get_one_warning_each() {
    let o = with_project();
    let warnings: Vec<&str> = o
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Warning && n.message.contains("text variable"))
        .map(|n| n.message.as_str())
        .collect();
    assert_eq!(
        warnings,
        [
            "1 text(s) use the text variable ${NOSUCHVAR}, which neither the board nor its \
             project defines; drawn as written",
            "1 text(s) use the text variable ${revision}, which neither the board nor its \
             project defines; drawn as written",
        ]
    );
    // Rows 1 to 6, 8 to 16 and 18 to 20, the 12 footprint texts that hold a variable and the two
    // BUILD fields (rows 37 and 41).
    assert!(o.notes.iter().any(|n| n.severity == Severity::Info
        && n.message
            == "32 text(s) use KiCad text variables and show their values, as KiCad does"));
}

#[test]
fn without_its_project_and_file_name_the_board_alone_fills_in_what_it_holds() {
    // As KiCad shows the board loaded without its project: the project's variables stay as
    // typed; the title block, the board property, the layers and the footprints' fields do not
    // need it. Without a file name, FILENAME and PROJECTNAME stay as typed too.
    let o = KiCad.read(&file("k10-text-vars.kicad_pcb")).unwrap();
    for (row, layer, x, y, shows) in BOARD_ROWS.iter().chain(&FOOTPRINT_ROWS) {
        let expected = match row {
            4 => "${PCB_NUMBER}",
            5 => "${BOARD_ID}",
            15 => "${FILENAME}",
            16 => "${PROJECTNAME}",
            _ => shows,
        };
        assert_eq!(text_at(&o, layer, *x, *y), expected, "README row {row}");
    }
    // PCB_NUMBER is also in the two BUILD fields (rows 37 and 41).
    for (name, n) in [
        ("BOARD_ID", 1),
        ("FILENAME", 1),
        ("NOSUCHVAR", 1),
        ("PCB_NUMBER", 3),
        ("PROJECTNAME", 1),
        ("revision", 1),
    ] {
        assert!(
            o.notes.iter().any(|note| note.severity == Severity::Warning
                && note.message.starts_with(&format!(
                    "{n} text(s) use the text variable ${{{name}}}, which neither"
                ))),
            "{name}"
        );
    }
}

#[test]
fn a_given_variable_wins_over_nothing_and_feeds_the_nested_one() {
    // As with --param PCB_NUMBER=X-1, which replaces the project's value.
    let mut project = project_variables(&file("k10-text-vars.kicad_pro")).unwrap();
    for (name, value) in &mut project {
        if name == "PCB_NUMBER" {
            *value = "X-1".into();
        }
    }
    let variables = Variables {
        file_name: Some("other.kicad_pcb".into()),
        project,
    };
    let o = read_with(&file("k10-text-vars.kicad_pcb"), &variables).unwrap();
    assert_eq!(text_at(&o, "F.SilkS", 105.0, 114.0), "X-1");
    assert_eq!(text_at(&o, "F.SilkS", 105.0, 117.0), "X-1-B2");
    assert_eq!(text_at(&o, "F.SilkS", 105.0, 147.0), "other.kicad_pcb");
    assert_eq!(text_at(&o, "F.SilkS", 105.0, 150.0), "other");
}

/// The one text on `layer` at KiCad's (x, y) on the page.
fn text_placed<'a>(o: &'a ReadOutcome, layer: &str, x: f64, y: f64) -> &'a board_model::Text {
    let found: Vec<_> = o
        .board
        .texts
        .iter()
        .filter(|t| {
            o.board.layers[t.layer].name == layer
                && (t.at.x - x).abs() < 0.001
                && (t.at.y + y).abs() < 0.001
        })
        .collect();
    assert_eq!(found.len(), 1, "{layer} ({x}, {y})");
    found[0]
}

#[test]
fn shown_fields_are_texts_with_their_own_placement() {
    use board_model::{HorizontalAnchor as H, VerticalAnchor as V};
    // README rows 22, 30 and 37 to 44: where KiCad shows each field (page coordinates), its
    // size, angle, justification and mirroring, read back with KiCad's scripting.
    let o = with_project();
    let rows = [
        // (row, shows, layer, x, y, height, angle, horizontal, vertical, mirrored)
        (
            22,
            "ABC-123",
            "F.SilkS",
            164.0,
            120.0,
            1.0,
            90.0,
            H::Right,
            V::Center,
            false,
        ),
        (
            30,
            "XYZ-9",
            "B.SilkS",
            176.0,
            150.0,
            1.0,
            90.0,
            H::Left,
            V::Center,
            true,
        ),
        (
            37,
            "DN-0042/J1",
            "F.SilkS",
            176.0,
            120.0,
            1.2,
            0.0,
            H::Left,
            V::Top,
            false,
        ),
        (
            41,
            "DN-0042/J2",
            "B.SilkS",
            164.0,
            150.0,
            1.2,
            0.0,
            H::Left,
            V::Top,
            true,
        ),
    ];
    for (row, shows, layer, x, y, height, angle, h, v, mirrored) in rows {
        let t = text_placed(&o, layer, x, y);
        assert_eq!(t.content, shows, "row {row}");
        assert!((t.height - height).abs() < 0.001, "row {row}");
        assert!((t.rotation_deg - angle).abs() < 0.001, "row {row}");
        assert_eq!(
            (t.anchor.horizontal, t.anchor.vertical),
            (h, v),
            "row {row}"
        );
        assert_eq!(t.mirrored, mirrored, "row {row}");
    }
    // The Reference fields are there as before (rows 21 and 29).
    assert_eq!(text_at(&o, "F.SilkS", 170.0, 120.0), "J1");
    assert_eq!(text_at(&o, "B.SilkS", 170.0, 150.0), "J2");
    // Rows 39 and 43: the hidden SECRET fields are left out, without a note.
    assert!(!o.board.texts.iter().any(|t| t.content == "hidden"));
    assert!(
        !o.notes.iter().any(|n| n.message.contains("hidden")),
        "{:?}",
        o.notes
    );
    // Rows 40 and 44: the FAB_NOTE fields are on the Fab layers, which are not drawn; they are
    // counted with the Value fields there.
    assert!(!o.board.texts.iter().any(|t| t.content == "fab note"));
    for layer in ["F.Fab", "B.Fab"] {
        let note =
            format!("2 text(s) on layer {layer} (only silkscreen and copper texts are read)");
        assert!(
            o.notes.iter().any(|n| n.message == note),
            "{note}: {:?}",
            o.notes
        );
    }
}
