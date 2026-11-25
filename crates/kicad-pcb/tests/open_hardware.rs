//! The reader against openly licensed boards made by others (tests/open-hardware, one folder each
//! with its README.txt). Every number here is the one KiCad 9.0 itself gives for the saved file
//! (see the folder's README.txt), not one the reader gave.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::indexing_slicing
)]

use board_model::{Contour, InputFormat, LayerKind, Point, ReadOutcome, Segment, Side};
use kicad_pcb::KiCad;

fn board(folder: &str, file: &str) -> ReadOutcome {
    let path = format!(
        "{}/tests/open-hardware/{folder}/{file}.kicad_pcb",
        env!("CARGO_MANIFEST_DIR")
    );
    KiCad.read(&std::fs::read(path).unwrap()).unwrap()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.001
}

/// Where a segment starts and ends.
fn ends(s: &Segment) -> (Point, Point) {
    match *s {
        Segment::Line { from, to } => (from, to),
        Segment::Arc {
            center,
            radius,
            start_deg,
            end_deg,
            ..
        } => {
            let at = |deg: f64| Point {
                x: center.x + radius * deg.to_radians().cos(),
                y: center.y + radius * deg.to_radians().sin(),
            };
            (at(start_deg), at(end_deg))
        }
    }
}

/// The box round the straight ends and arc ends of a contour (the outline's centre line, as
/// KiCad's own board size ignores the line width).
fn bounds(c: &Contour) -> (f64, f64, f64, f64) {
    let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for s in &c.segments {
        let (from, to) = ends(s);
        for p in [from, to] {
            b = (b.0.min(p.x), b.1.min(p.y), b.2.max(p.x), b.3.max(p.y));
        }
    }
    b
}

/// The contour closes: each segment ends where the next begins.
fn closed(c: &Contour) -> bool {
    let ends: Vec<_> = c.segments.iter().map(ends).collect();
    let n = ends.len();
    n > 2
        && (0..n).all(|i| {
            let (_, to) = ends[i];
            let (from, _) = ends[(i + 1) % n];
            (to.x - from.x).abs() < 0.001 && (to.y - from.y).abs() < 0.001
        })
}

/// The copper layers that are really there (the reader also lists the `*.Cu` wildcard layer
/// that through-hole pads name; it has no side).
fn copper_layers(o: &ReadOutcome) -> usize {
    o.board
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Copper && l.side.is_some())
        .count()
}

fn sides(o: &ReadOutcome) -> (usize, usize) {
    let on = |s| o.board.components.iter().filter(|c| c.side == s).count();
    (on(Side::Top), on(Side::Bottom))
}

/// A real board names 3D models from KiCad's own library and draws on layers the reader does not
/// read; each of those is a note, none is an error, and the models are said to be outside.
fn notes_name_outside_models(o: &ReadOutcome) -> bool {
    o.notes.iter().any(|n| {
        n.message
            .contains("3D model file(s) named by the board, not stored in it")
    })
}

#[test]
fn gb_cartpp_xc_is_80_by_55_with_rounded_corners_1_0_thick() {
    let o = board("gb-cartpp-xc", "GB-CARTPP-XC");
    let b = &o.board;
    assert!(near(b.thickness.unwrap(), 1.0));
    assert_eq!(copper_layers(&o), 2);
    // Four straight sides and four rounded corners, one closed outline, no cutouts.
    assert_eq!(b.outline.outer.segments.len(), 8);
    assert!(closed(&b.outline.outer));
    assert!(b.outline.cutouts.is_empty());
    let (x0, y0, x1, y1) = bounds(&b.outline.outer);
    assert!(
        near(x1 - x0, 80.0) && near(y1 - y0, 55.0),
        "{x0} {y0} {x1} {y1}"
    );
    assert!(near(x0, 50.0) && near(x1, 130.0) && near(y0, -105.0) && near(y1, -50.0));
    assert!(notes_name_outside_models(&o), "{:?}", o.notes);
}

#[test]
fn gb_cartpp_xc_has_29_parts_all_on_top() {
    let o = board("gb-cartpp-xc", "GB-CARTPP-XC");
    assert_eq!(o.board.components.len(), 29);
    assert_eq!(sides(&o), (29, 0));
    let c = |d: &str| o.board.components.iter().find(|c| c.designator == d);
    assert!(c("U2").unwrap().footprint.contains("TQFP-44"));
    assert!(c("J3").unwrap().footprint.contains("GameBoy_Cartridge"));
    assert!(c("J1").unwrap().footprint.contains("USB_C_Receptacle"));
    assert!(c("R4").is_some() && c("TP4").is_some());
}

#[test]
fn franzininho_diy_is_51_by_30_and_1_6_thick() {
    let o = board("franzininho-diy", "Franzininho-DIY");
    let b = &o.board;
    assert!(near(b.thickness.unwrap(), 1.6));
    assert_eq!(copper_layers(&o), 2);
    assert_eq!(b.outline.outer.segments.len(), 4);
    assert!(closed(&b.outline.outer));
    assert!(b.outline.cutouts.is_empty());
    let (x0, y0, x1, y1) = bounds(&b.outline.outer);
    assert!(
        near(x1 - x0, 51.0675) && near(y1 - y0, 30.03),
        "{x0} {y0} {x1} {y1}"
    );
    assert!(notes_name_outside_models(&o), "{:?}", o.notes);
}

#[test]
fn franzininho_diy_has_30_footprints_28_on_top_and_2_on_the_bottom() {
    let o = board("franzininho-diy", "Franzininho-DIY");
    assert_eq!(o.board.components.len(), 30);
    assert_eq!(sides(&o), (28, 2));
    let c = |d: &str| o.board.components.iter().find(|c| c.designator == d);
    assert!(c("X1").unwrap().footprint.contains("USB-A-H"));
    assert!(c("IC1").unwrap().footprint.contains("DIP-8"));
    assert!(c("RESET").is_some());
}
