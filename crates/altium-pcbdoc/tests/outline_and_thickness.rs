//! Roadmap step 2: the board outline, its cutouts and the thickness, checked against the numbers
//! in tests/boards/README.txt (to 0.001 mm, as AGENTS.md asks).

use altium_pcbdoc::Altium;
use board_model::{Contour, InputFormat, ReadOutcome, Segment, Severity};

const TOLERANCE: f64 = 0.001;

fn read(name: &str) -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards")
        .join(name);
    Ok(Altium.read(&std::fs::read(path)?)?)
}

type Xy = (f64, f64);

/// The (from, to) ends of each segment, or `None` if any segment is not a straight line.
fn lines(contour: &Contour) -> Option<Vec<(Xy, Xy)>> {
    contour
        .segments
        .iter()
        .map(|s| match s {
            Segment::Line { from, to } => Some(((from.x, from.y), (to.x, to.y))),
            Segment::Arc { .. } => None,
        })
        .collect()
}

/// True when the contour is closed, straight-edged and has exactly these corners, in either
/// direction and starting anywhere.
fn same_polygon(contour: &Contour, expected: &[(f64, f64)]) -> bool {
    let Some(lines) = lines(contour) else {
        return false;
    };
    let next = lines.iter().skip(1).chain(lines.first());
    let closed = lines.iter().zip(next).all(|(a, b)| a.1 == b.0);
    let got: Vec<(f64, f64)> = lines.iter().map(|l| l.0).collect();
    let near = |a: (f64, f64), b: (f64, f64)| {
        (a.0 - b.0).abs() <= TOLERANCE && (a.1 - b.1).abs() <= TOLERANCE
    };
    let matches = |order: &[(f64, f64)]| {
        got.len() == order.len()
            && (0..order.len()).any(|shift| {
                let rotated = order.iter().cycle().skip(shift);
                got.iter().zip(rotated).all(|(p, q)| near(*p, *q))
            })
    };
    let reversed: Vec<(f64, f64)> = expected.iter().rev().copied().collect();
    closed && !got.is_empty() && (matches(expected) || matches(&reversed))
}

const BOARD_50_BY_30: [(f64, f64); 4] = [(0.0, 0.0), (50.0, 0.0), (50.0, 30.0), (0.0, 30.0)];

#[test]
fn empty_outline_is_50_by_30_and_1_6_thick() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("empty-outline.PcbDoc")?;
    assert!(
        same_polygon(&out.board.outline.outer, &BOARD_50_BY_30),
        "{:?}",
        out.board.outline
    );
    assert!(out.board.outline.cutouts.is_empty());
    let t = out.board.thickness.ok_or("no thickness")?;
    assert!((t - 1.6).abs() <= TOLERANCE, "thickness {t}");
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn outline_with_hole_has_the_10_by_10_cutout() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("outline-with-hole.PcbDoc")?;
    assert!(same_polygon(&out.board.outline.outer, &BOARD_50_BY_30));
    assert_eq!(out.board.outline.cutouts.len(), 1);
    assert!(same_polygon(
        &out.board.outline.cutouts[0],
        &[(30.0, 10.0), (40.0, 10.0), (40.0, 20.0), (30.0, 20.0)]
    ));
    // The 3 mm hole is a pad, read in roadmap step 3; until then it is reported, not lost.
    assert!(out
        .notes
        .iter()
        .any(|n| n.severity == Severity::Dropped && n.message.contains("/Pads6/Data")));
    Ok(())
}

#[test]
fn every_board_has_the_same_outline_and_thickness() -> Result<(), Box<dyn std::error::Error>> {
    for name in [
        "empty-outline.PcbDoc",
        "outline-with-hole.PcbDoc",
        "one-track.PcbDoc",
        "one-resistor.PcbDoc",
    ] {
        let out = read(name)?;
        assert!(
            same_polygon(&out.board.outline.outer, &BOARD_50_BY_30),
            "{name}"
        );
        let t = out.board.thickness.ok_or("no thickness")?;
        assert!((t - 1.6).abs() <= TOLERANCE, "{name}: thickness {t}");
    }
    Ok(())
}

#[test]
fn what_is_not_read_yet_is_reported() -> Result<(), Box<dyn std::error::Error>> {
    let track = read("one-track.PcbDoc")?;
    assert!(track
        .notes
        .iter()
        .any(|n| n.severity == Severity::Dropped && n.message.contains("/Tracks6/Data")));
    let resistor = read("one-resistor.PcbDoc")?;
    for stream in ["/Components6/Data", "/Pads6/Data", "/ComponentBodies6/Data"] {
        assert!(
            resistor.notes.iter().any(|n| n.message.contains(stream)),
            "{stream} missing from {:?}",
            resistor.notes
        );
    }
    Ok(())
}
