//! Shapes drawn on copper (`gr_*` and `fp_*` on a copper layer) against our own KiCad 10 boards.
//! Every number is in the board's README.txt, read back with KiCad's scripting (page
//! coordinates; model x = 100 + X, model y = -(100 + Y)).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use board_model::{Contour, CopperItem, InputFormat, Point, ReadOutcome, Segment, Severity};
use kicad_pcb::KiCad;

fn board(name: &str) -> ReadOutcome {
    let path = format!(
        "{}/tests/boards/{name}.kicad_pcb",
        env!("CARGO_MANIFEST_DIR")
    );
    KiCad.read(&std::fs::read(path).unwrap()).unwrap()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 0.001
}

fn at(p: Point, x: f64, y: f64) -> bool {
    near(p.x, 100.0 + x) && near(p.y, -(100.0 + y))
}

fn layer(o: &ReadOutcome, name: &str) -> usize {
    o.board.layers.iter().position(|l| l.name == name).unwrap()
}

/// The corners of a polygon, as KiCad's (x, y).
fn corners(c: &Contour) -> Vec<(f64, f64)> {
    c.segments
        .iter()
        .map(|s| match *s {
            Segment::Line { from, .. } => (from.x - 100.0, -from.y - 100.0),
            Segment::Arc { .. } => panic!("a polygon has straight sides"),
        })
        .collect()
}

/// True when the polygon has exactly these corners, in any order and from any start.
fn has_corners(c: &Contour, wanted: &[(f64, f64)]) -> bool {
    let found = corners(c);
    found.len() == wanted.len()
        && wanted
            .iter()
            .all(|w| found.iter().any(|f| near(f.0, w.0) && near(f.1, w.1)))
}

/// The regions on a layer: (contour, holes).
fn regions<'a>(o: &'a ReadOutcome, name: &str) -> Vec<(&'a Contour, &'a Vec<Contour>)> {
    let l = layer(o, name);
    o.board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Region {
                layer,
                contour,
                holes,
            } if *layer == l => Some((contour, holes)),
            _ => None,
        })
        .collect()
}

#[test]
fn filled_copper_shapes_are_regions_and_their_strokes_tracks() {
    let o = board("k10-copper-shapes");
    assert!(
        o.notes.iter().all(|n| n.severity == Severity::Info),
        "{:?}",
        o.notes
    );
    let top = regions(&o, "F.Cu");
    let bottom = regions(&o, "B.Cu");
    // Board: the L, the rectangle and the circle on F.Cu, the triangle on B.Cu. U1: a circle and a
    // rectangle on F.Cu. U2 (flipped): a triangle on B.Cu.
    assert_eq!(top.len(), 5);
    assert_eq!(bottom.len(), 2);
    assert!(top.iter().chain(&bottom).all(|(_, holes)| holes.is_empty()));
    let polygon = |list: &[(&Contour, &Vec<Contour>)], wanted: &[(f64, f64)]| {
        assert!(
            list.iter().any(
                |(c, _)| c.segments.iter().all(|s| matches!(s, Segment::Line { .. }))
                    && has_corners(c, wanted)
            ),
            "{wanted:?}"
        );
    };
    polygon(
        &top,
        &[
            (5.0, 5.0),
            (15.0, 5.0),
            (15.0, 8.0),
            (8.0, 8.0),
            (8.0, 12.0),
            (5.0, 12.0),
        ],
    );
    polygon(
        &top,
        &[(20.0, 5.0), (28.0, 5.0), (28.0, 12.0), (20.0, 12.0)],
    );
    polygon(
        &top,
        &[(22.0, 20.5), (25.0, 20.5), (25.0, 23.5), (22.0, 23.5)],
    );
    polygon(&bottom, &[(42.0, 4.0), (47.0, 4.0), (44.5, 10.0)]);
    // U2's triangle, turned 150 by its footprint (30, then the flip), as KiCad gives its corners.
    polygon(
        &bottom,
        &[(39.0, 20.2679), (34.9019, 21.366), (40.0981, 24.366)],
    );
    // The circles are exact: one full arc each.
    for (x, y, r) in [(36.0, 8.5, 3.0), (20.0, 22.0, 1.5)] {
        assert!(
            top.iter().any(|(c, _)| matches!(
                c.segments.as_slice(),
                [Segment::Arc { center, radius, start_deg, end_deg, .. }]
                    if at(*center, x, y) && near(*radius, r) && start_deg == end_deg
            )),
            "circle about ({x}, {y})"
        );
    }
    // The strokes: the filled rectangle's 0.2 (4 sides), the triangle's 0.3 (3) and the open
    // rectangle's 0.25 (4), as tracks on the shapes' edges. The shapes without a stroke have none.
    let tracks: Vec<(usize, Point, Point, f64)> = o
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Track {
                layer,
                from,
                to,
                width,
                ..
            } => Some((*layer, *from, *to, *width)),
            _ => None,
        })
        .collect();
    assert_eq!(tracks.len(), 11);
    let count = |l: &str, w: f64| {
        tracks
            .iter()
            .filter(|(layer_, _, _, width)| *layer_ == layer(&o, l) && near(*width, w))
            .count()
    };
    assert_eq!(count("F.Cu", 0.2), 4);
    assert_eq!(count("B.Cu", 0.3), 3);
    assert_eq!(count("F.Cu", 0.25), 4);
    for (x, y) in [(5.0, 16.0), (12.0, 16.0), (12.0, 22.0), (5.0, 22.0)] {
        assert!(tracks
            .iter()
            .any(|(_, from, _, w)| near(*w, 0.25) && at(*from, x, y)));
    }
    assert!(
        !o.board
            .copper
            .iter()
            .any(|c| matches!(c, CopperItem::TrackArc { .. })),
        "the filled circles have no stroke"
    );
}

/// The area of a polygon (its corners only; fills are polygons).
fn polygon_area(c: &Contour) -> f64 {
    let pts: Vec<Point> = c
        .segments
        .iter()
        .map(|s| match *s {
            Segment::Line { from, .. } => from,
            Segment::Arc { .. } => panic!("fills are polygons"),
        })
        .collect();
    let n = pts.len();
    ((0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0)
        .abs()
}

#[test]
fn a_footprints_zones_are_copper_in_board_coordinates() {
    let o = board("k10-footprint-zone");
    // U1's zone on F.Cu, U2's (flipped) on B.Cu, each with the gap round the footprint's hole:
    // KiCad's fill covers 118.2093 mm2, and its corners nearest the hole lie 0.7509 to 0.7541
    // from the hole's center (the 0.5 hole and KiCad's hole clearance of 0.25).
    for (layer_name, cx, cy, ring) in [
        (
            "F.Cu",
            13.0,
            15.0,
            [(7.0, 10.0), (19.0, 10.0), (19.0, 20.0), (7.0, 20.0)],
        ),
        (
            "B.Cu",
            36.0,
            15.0,
            [
                (43.6962, 13.6699),
                (33.3038, 7.6699),
                (28.3038, 16.3301),
                (38.6962, 22.3301),
            ],
        ),
    ] {
        let found = regions(&o, layer_name);
        assert_eq!(found.len(), 1, "{layer_name}");
        let (contour, holes) = found[0];
        assert_eq!(holes.len(), 1, "{layer_name}");
        let copper = polygon_area(contour) - polygon_area(&holes[0]);
        assert!((copper - 118.2093).abs() < 0.001, "{layer_name}: {copper}");
        for s in &holes[0].segments {
            let Segment::Line { from, .. } = *s else {
                panic!("fills are polygons")
            };
            let d = (from.x - 100.0 - cx).hypot(-from.y - 100.0 - cy);
            assert!(
                (0.7509 - 0.001..=0.7541 + 0.001).contains(&d),
                "{layer_name}: {d}"
            );
        }
        // The outer contour follows the zone's outline (its corners rounded off by the minimum
        // width): every outline corner is within 0.13 of the fill's corners.
        let pts = corners(contour);
        for (x, y) in ring {
            assert!(
                pts.iter().any(|p| (p.0 - x).hypot(p.1 - y) < 0.13),
                "{layer_name}: ({x}, {y})"
            );
        }
    }
    // U2's keep-out rule area holds no copper and is named in a note; nothing else is lost.
    let other: Vec<&str> = o
        .notes
        .iter()
        .filter(|n| n.severity != Severity::Info)
        .map(|n| n.message.as_str())
        .collect();
    assert_eq!(other, ["1 keep-out zone(s) (they hold no copper)"]);
}
