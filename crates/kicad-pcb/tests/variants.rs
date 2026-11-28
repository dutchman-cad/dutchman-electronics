//! Do-not-populate parts and a board variant against our own KiCad 10 board `k10-dnp`. Every
//! number is in its README.txt, read back with KiCad's scripting (page coordinates; model
//! x = 100 + X, model y = -(100 + Y)).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use board_model::{InputFormat, ReadOutcome, Side, Variant, VariantValue};
use kicad_pcb::KiCad;

fn board() -> ReadOutcome {
    let path = format!(
        "{}/tests/boards/k10-dnp.kicad_pcb",
        env!("CARGO_MANIFEST_DIR")
    );
    KiCad.read(&std::fs::read(path).unwrap()).unwrap()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 0.001
}

/// All five parts are placed as the README gives them, value 10k, test:R_0603 on the top, not turned, with
/// their model; only R2, `(attr smd dnp)`, is not fitted. R3 to R5 carry the other flags
/// (exclude_from_pos_files, exclude_from_bom, board_only) and stay fitted.
#[test]
fn only_the_do_not_populate_part_is_not_fitted() {
    let o = board();
    let mut parts: Vec<_> = o.board.components.iter().collect();
    parts.sort_by(|a, b| a.designator.cmp(&b.designator));
    let want = [
        ("R1", 10.0, 10.0, true),
        ("R2", 20.0, 10.0, false),
        ("R3", 30.0, 10.0, true),
        ("R4", 40.0, 10.0, true),
        ("R5", 10.0, 20.0, true),
    ];
    assert_eq!(parts.len(), want.len());
    for (p, (name, x, y, fitted)) in parts.iter().zip(want) {
        assert_eq!(p.designator, name);
        assert!(
            near(p.at.x, 100.0 + x) && near(p.at.y, -(100.0 + y)),
            "{name}: {:?}",
            p.at
        );
        assert_eq!(p.fitted, fitted, "{name}");
        assert_eq!(p.footprint, "R_0603", "the name without its library");
        assert_eq!(p.side, Side::Top);
        assert!(near(p.rotation_deg, 0.0));
        assert_eq!(p.models.len(), 1, "{name}");
        assert_eq!(p.comment, "10k", "{name}: the comment is the Value field");
    }
}

/// LITE leaves R1 out and gives it the value 1k, and fits R2, which the board marks DNP. R3 to
/// R5 have no entry. The variants list is read, so the note that it was not is gone, and no note
/// mentions a variant.
#[test]
fn the_lite_variant_leaves_r1_out_with_its_value_and_fits_r2() {
    let o = board();
    assert_eq!(
        o.board.variants,
        [Variant {
            name: "LITE".into(),
            not_fitted: vec!["R1".into()],
            fitted: vec!["R2".into()],
            values: vec![VariantValue {
                designator: "R1".into(),
                comment: "1k".into(),
            }],
        }]
    );
    assert!(
        !o.notes
            .iter()
            .any(|n| n.message.contains("variant") || n.message.contains("R2")),
        "{:?}",
        o.notes
    );
}
