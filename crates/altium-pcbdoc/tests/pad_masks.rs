//! Pads with mask settings of their own (tests/boards/README.txt, "Pad mask overrides"): the
//! reader puts them on the pads. P2 to P6 and P8 have a manual solder mask expansion (P8 from the
//! hole's edge), P4 and P6 are tented and P7 has a manual paste expansion: seven pads. No other
//! test board has a pad like that, except all-features, which carries every feature the reader
//! handles (its pads are checked in all_features.rs).

use altium_pcbdoc::Altium;
use board_model::{CopperItem, InputFormat, Severity};
use std::path::Path;

const NOTES: [&str; 2] = [
    "measure their solder mask expansion",
    "paste mask expansion of their own",
];

#[test]
fn pads_carry_their_own_mask_settings() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards/pad-mask-overrides.PcbDoc");
    let out = Altium.read(&std::fs::read(path)?)?;
    let mut tented = 0;
    let mut expanded = 0;
    for item in &out.board.copper {
        if let CopperItem::Pad {
            tented: t,
            mask_expansion: e,
            ..
        } = item
        {
            tented += usize::from(t.contains(&Some(true)));
            expanded += usize::from(e.iter().any(Option::is_some));
        }
    }
    assert_eq!((tented, expanded), (2, 6), "{:?}", out.board.copper);
    let hole_edge: Vec<_> = out
        .notes
        .iter()
        .filter(|n| n.message.contains("from the hole's edge"))
        .collect();
    let [hole_edge] = hole_edge.as_slice() else {
        return Err(format!("{:?}", out.notes).into());
    };
    assert_eq!(hole_edge.severity, Severity::Info);
    assert_eq!(
        hole_edge.message,
        "1 pad(s) measure their solder mask expansion from the hole's edge, which leaves the \
         opening smaller than the pad; read as the matching negative expansion from the pad's \
         edge"
    );
    assert!(out.notes.iter().any(|n| n.severity == Severity::Info
        && n.message
            == "1 pad(s) have a paste mask expansion of their own; the paste mask is not drawn"));
    Ok(())
}

#[test]
fn no_other_board_has_a_pad_with_its_own_mask() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards");
    let mut folders = vec![dir];
    let mut read = 0;
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "PcbDoc")
                || path.file_name().is_some_and(|n| {
                    n == "pad-mask-overrides.PcbDoc"
                        || n == "all-features.PcbDoc"
                        || n == "paste.PcbDoc"
                })
            {
                continue;
            }
            let out = Altium.read(&std::fs::read(&path)?)?;
            assert!(
                out.notes
                    .iter()
                    .all(|n| NOTES.iter().all(|k| !n.message.contains(k))),
                "{}: {:?}",
                path.display(),
                out.notes
            );
            read += 1;
        }
    }
    assert!(read > 50, "only {read} boards read");
    Ok(())
}

/// A pad's x, tented and mask expansion, as the board model holds them.
type PadMask = (f64, [Option<bool>; 2], [Option<f64>; 2]);

/// Each of P1 to P8 (x 13 to 41) with its tenting and mask expansion (top, bottom), to 0.001 mm.
/// P8: 0.1 from the edge of its 1.000 hole on a 2.000 pad is (1.0 + 0.2 - 2.0) / 2 = -0.4 from the
/// pad's edge.
#[test]
fn each_pad_has_the_tenting_and_expansion_it_was_given() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards/pad-mask-overrides.PcbDoc");
    let out = Altium.read(&std::fs::read(path)?)?;
    let mut pads: Vec<PadMask> = out
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Pad {
                at,
                tented,
                mask_expansion,
                ..
            }
            | CopperItem::PolygonPad {
                at,
                tented,
                mask_expansion,
                ..
            } => Some((at.x, *tented, *mask_expansion)),
            _ => None,
        })
        .collect();
    pads.sort_by(|a, b| a.0.total_cmp(&b.0));
    let both = |e: f64| [Some(e), Some(e)];
    let want = [
        (13.0, [None, None], [None, None]),
        (17.0, [None, None], both(0.0)),
        (21.0, [None, None], both(0.2)),
        (25.0, [Some(true), None], both(0.1)),
        (29.0, [None, None], [Some(0.2), Some(0.05)]),
        (33.0, [None, Some(true)], both(0.1)),
        (37.0, [None, None], [None, None]),
        (41.0, [None, None], both(-0.4)),
    ];
    assert_eq!(pads.len(), want.len(), "{pads:?}");
    for ((x, tented, expansion), (wx, wt, we)) in pads.iter().zip(want) {
        assert!((x - wx).abs() <= 0.001, "{pads:?}");
        assert_eq!(*tented, wt, "pad at x {wx}");
        let same = expansion.iter().zip(we).all(|(a, b)| match (a, b) {
            (Some(a), Some(b)) => (a - b).abs() <= 0.001,
            (None, None) => true,
            _ => false,
        });
        assert!(same, "pad at x {wx}: {expansion:?}, want {we:?}");
    }
    Ok(())
}
