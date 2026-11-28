//! The test project `variants-project`: its two assembly variants, read from the project file
//! and put on its board. Checked against tests/boards/variants-project/README.txt.

use altium_pcbdoc::{project, Altium};
use board_model::{InputFormat, Variant, VariantValue};

fn folder() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards/variants-project")
}

/// Lite leaves R2 out; Alt changes R1's comment to 22k. Nothing in either is left unread.
#[test]
fn the_project_has_lite_and_alt() -> Result<(), Box<dyn std::error::Error>> {
    let p = project::read(&std::fs::read(folder().join("variants-project.PrjPcb"))?)?;
    assert_eq!(p.boards, ["variants-board.PcbDoc"]);
    assert_eq!(
        p.variants,
        [
            Variant {
                name: "Lite".into(),
                not_fitted: vec!["R2".into()],
                values: Vec::new(),
            },
            Variant {
                name: "Alt".into(),
                not_fitted: Vec::new(),
                values: vec![VariantValue {
                    designator: "R1".into(),
                    comment: "22k".into(),
                }],
            },
        ]
    );
    assert!(p.notes.is_empty(), "{:?}", p.notes);
    Ok(())
}

/// The board read alone has no variants and both parts fitted (R1 10k at (20, 15), R2 4k7 at
/// (30, 15)); read with its project it has the project's two, with no note.
#[test]
fn the_board_gets_its_variants_only_with_its_project() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = Altium.read(&std::fs::read(folder().join("variants-board.PcbDoc"))?)?;
    assert!(out.board.variants.is_empty());
    let mut parts: Vec<(&str, &str, f64, f64, bool)> = out
        .board
        .components
        .iter()
        .map(|c| {
            (
                c.designator.as_str(),
                c.comment.as_str(),
                c.at.x,
                c.at.y,
                c.fitted,
            )
        })
        .collect();
    parts.sort_by(|a, b| a.0.cmp(b.0));
    let [r1, r2] = parts.as_slice() else {
        return Err(format!("{parts:?}").into());
    };
    let near = |a: f64, b: f64| (a - b).abs() <= 0.001;
    assert!(
        r1.0 == "R1" && r1.1 == "10k" && near(r1.2, 20.0) && near(r1.3, 15.0) && r1.4,
        "{parts:?}"
    );
    assert!(
        r2.0 == "R2" && r2.1 == "4k7" && near(r2.2, 30.0) && near(r2.3, 15.0) && r2.4,
        "{parts:?}"
    );
    let p = project::read(&std::fs::read(folder().join("variants-project.PrjPcb"))?)?;
    let notes = project::add_variants(&mut out.board, &p);
    assert!(notes.is_empty(), "{notes:?}");
    let names: Vec<&str> = out.board.variants.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["Lite", "Alt"]);
    Ok(())
}
