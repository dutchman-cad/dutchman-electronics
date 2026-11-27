//! Features no board had before (tests/boards/README.txt, "Feature gaps, first batch"): keepouts
//! and pad holes that are not round. Numbers to 0.001 mm, as AGENTS.md asks.

use altium_pcbdoc::Altium;
use board_model::{CopperItem, InputFormat, ReadOutcome, Severity};

const TOLERANCE: f64 = 0.001;

fn read(name: &str) -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards")
        .join(name);
    Ok(Altium.read(&std::fs::read(path)?)?)
}

/// keepouts: a track, arc, fill and region on the Keep-Out layer, and a track, fill and region on
/// Top Layer with the Keepout box ticked. None of them is copper: the only copper is the plain
/// Top Layer track (5, 15) to (15, 15), width 0.300. One Info note per kind and layer.
#[test]
fn keepouts_are_not_copper() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("keepouts.PcbDoc")?;
    let [CopperItem::Track {
        from, to, width, ..
    }] = out.board.copper.as_slice()
    else {
        return Err(format!("{:?}", out.board.copper).into());
    };
    let near = |a: f64, b: f64| (a - b).abs() <= TOLERANCE;
    assert!(near(from.x, 5.0) && near(from.y, 15.0), "{from:?}");
    assert!(near(to.x, 15.0) && near(to.y, 15.0), "{to:?}");
    assert!(near(*width, 0.3), "{width}");
    assert!(out.board.graphics.is_empty(), "{:?}", out.board.graphics);

    let mut info: Vec<&str> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Info && n.message.contains("keepout"))
        .map(|n| n.message.as_str())
        .collect();
    info.sort_unstable();
    let layer = "on Keep-Out Layer left out: a keepout is a rule for placing and routing, not \
                 part of the board";
    let flagged = "on Top Layer are keepouts (Altium's Keepout box), not copper; left out";
    assert_eq!(
        info,
        [
            format!("1 arc(s) {layer}"),
            format!("1 fill(s) {layer}"),
            format!("1 fill(s) {flagged}"),
            format!("1 region(s) {layer}"),
            format!("1 region(s) {flagged}"),
            format!("1 track(s) {layer}"),
            format!("1 track(s) {flagged}"),
        ]
    );
    assert!(
        out.notes
            .iter()
            .all(|n| n.severity != Severity::Dropped && n.severity != Severity::Warning),
        "{:?}",
        out.notes
    );
    Ok(())
}

/// pad-hole-shapes: five pads at y 15 with a round hole of 1.000 (x 8), a slot 0.800 by 2.200
/// (x 16), the same slot turned 45 degrees (x 25), a square hole of side 1.000 (x 34) and an
/// unplated slot 0.800 by 2.200 (x 42). The board model holds round drills only, so each is read
/// with a round hole of the slot's width or the square's side, and a Degraded note names each
/// pad that is not round.
#[test]
fn slot_and_square_holes_are_read_as_round_with_a_note() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("pad-hole-shapes.PcbDoc")?;
    let mut pads: Vec<(f64, f64, Option<f64>, bool)> = out
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Pad {
                at, drill, plated, ..
            } => Some((at.x, at.y, *drill, *plated)),
            _ => None,
        })
        .collect();
    pads.sort_by(|a, b| a.0.total_cmp(&b.0));
    let want = [
        (8.0, 1.0, true),
        (16.0, 0.8, true),
        (25.0, 0.8, true),
        (34.0, 1.0, true),
        (42.0, 0.8, false),
    ];
    assert_eq!(pads.len(), want.len(), "{pads:?}");
    for ((x, y, drill, plated), (wx, wd, wp)) in pads.iter().zip(want) {
        assert!(
            (x - wx).abs() <= TOLERANCE && (y - 15.0).abs() <= TOLERANCE,
            "{pads:?}"
        );
        assert!(
            drill.is_some_and(|d| (d - wd).abs() <= TOLERANCE),
            "pad at x {wx}"
        );
        assert_eq!(*plated, wp, "pad at x {wx}");
    }
    let mut warnings: Vec<&str> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Warning)
        .map(|n| n.message.as_str())
        .collect();
    warnings.sort_unstable();
    let tail = "; the board model holds round holes only, so it is read as a round hole of that \
                width";
    assert_eq!(
        warnings,
        [
            format!("the pad at (16.000, 15.000) has a slot hole 0.800 wide and 2.200 long{tail}"),
            format!(
                "the pad at (25.000, 15.000) has a slot hole 0.800 wide and 2.200 long, turned 45 \
                 degrees{tail}"
            ),
            format!("the pad at (34.000, 15.000) has a square hole of side 1.000{tail}"),
            format!("the pad at (42.000, 15.000) has a slot hole 0.800 wide and 2.200 long{tail}"),
        ]
    );
    Ok(())
}

/// pour-modes: four 10 x 10 GND pours on Top Layer, y 10 to 20. What Altium poured is read as
/// copper: the Solid pour (x 2 to 12) as one region, the 90 degree hatch (x 14 to 24) as 24
/// tracks, the 45 degree hatch (x 26 to 36) as 34 and the None pour (x 38 to 48) as its four
/// outline tracks only, all 0.300 wide; nothing fills the None pour.
#[test]
fn each_pour_is_read_as_altium_poured_it() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("pour-modes.PcbDoc")?;
    let mut tracks = [0usize; 4];
    let mut regions = [0usize; 4];
    let pour = |x: f64| ((x - 1.0) / 12.0).floor().clamp(0.0, 3.0) as usize;
    for c in &out.board.copper {
        match c {
            CopperItem::Track {
                from, to, width, ..
            } => {
                assert!((width - 0.3).abs() <= TOLERANCE, "{c:?}");
                let p = pour((from.x + to.x) / 2.0);
                for q in [from, to] {
                    assert!(q.y >= 10.0 - TOLERANCE && q.y <= 20.0 + TOLERANCE, "{c:?}");
                    let (lo, hi) = (2.0 + 12.0 * p as f64, 12.0 + 12.0 * p as f64);
                    assert!(q.x >= lo - TOLERANCE && q.x <= hi + TOLERANCE, "{c:?}");
                }
                if let Some(n) = tracks.get_mut(p) {
                    *n += 1;
                }
            }
            CopperItem::Region { contour, .. } => {
                let x = contour.segments.iter().find_map(|s| match s {
                    board_model::Segment::Line { from, .. } => Some(from.x),
                    _ => None,
                });
                if let Some(n) = x.and_then(|x| regions.get_mut(pour(x))) {
                    *n += 1;
                }
            }
            other => return Err(format!("{other:?}").into()),
        }
    }
    assert_eq!(tracks, [0, 24, 34, 4]);
    assert_eq!(regions, [1, 0, 0, 0]);
    Ok(())
}

/// mech-layer-kinds: what lies on the typed mechanical layers is not read yet, and each note
/// names the layer's kind and side.
#[test]
fn notes_for_mechanical_layers_name_their_kind() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("mech-layer-kinds.PcbDoc")?;
    let mut dropped: Vec<&str> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Dropped)
        .map(|n| n.message.as_str())
        .collect();
    dropped.sort_unstable();
    assert_eq!(
        dropped,
        [
            "1 track(s) on Bottom Courtyard (courtyard, bottom) not read yet",
            "2 text(s) on Top Designator (designator, top) not read yet",
            "4 track(s) on Top Assembly (assembly, top) not read yet",
            "4 track(s) on Top Component Outline (component outline, top) not read yet",
            "4 track(s) on Top Courtyard (courtyard, top) not read yet",
        ]
    );
    Ok(())
}

/// The box of a contour's line ends and arc ends, as (x0, x1, y0, y1).
fn contour_box(contour: &board_model::Contour) -> (f64, f64, f64, f64) {
    let mut b = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    let mut add = |x: f64, y: f64| {
        b = (b.0.min(x), b.1.max(x), b.2.min(y), b.3.max(y));
    };
    for s in &contour.segments {
        match s {
            board_model::Segment::Line { from, to } => {
                add(from.x, from.y);
                add(to.x, to.y);
            }
            board_model::Segment::Arc {
                center,
                radius,
                start_deg,
                end_deg,
                ..
            } => {
                for a in [start_deg, end_deg] {
                    let (sin, cos) = a.to_radians().sin_cos();
                    add(center.x + radius * cos, center.y + radius * sin);
                }
            }
        }
    }
    b
}

/// teardrops: Altium's seven teardrops are ordinary copper regions on Top Layer (byte 1 of their
/// records is 0x1C, not 0x0C; nothing else differs), so they are read as copper, each with the
/// box Altium reports, and raise no note.
#[test]
fn teardrops_are_copper_regions() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("teardrops.PcbDoc")?;
    let mut boxes: Vec<(f64, f64, f64, f64)> = out
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Region {
                layer,
                contour,
                holes,
            } => {
                let top = out.board.layers.get(*layer).map(|l| l.name.as_str());
                assert_eq!(top, Some("Top Layer"));
                assert!(holes.is_empty());
                Some(contour_box(contour))
            }
            _ => None,
        })
        .collect();
    boxes.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.total_cmp(&b.2)));
    let want = [
        (10.571, 11.270, 14.440, 15.560),
        (19.548, 19.786, 14.790, 15.210),
        (20.214, 20.452, 14.790, 15.210),
        (28.999, 29.253, 14.746, 15.254),
        (30.747, 31.001, 14.746, 15.254),
        (39.548, 39.786, 14.790, 15.210),
        (39.790, 40.210, 15.214, 15.452),
    ];
    assert_eq!(boxes.len(), want.len(), "{boxes:?}");
    for (b, w) in boxes.iter().zip(want) {
        let got = [b.0, b.1, b.2, b.3];
        let expected = [w.0, w.1, w.2, w.3];
        assert!(
            got.iter()
                .zip(expected)
                .all(|(g, e)| (g - e).abs() <= TOLERANCE),
            "{got:?}, want {expected:?}"
        );
    }
    let tracks = out
        .board
        .copper
        .iter()
        .filter(|c| matches!(c, CopperItem::Track { .. }))
        .count();
    assert_eq!(tracks, 4);
    assert!(
        out.notes
            .iter()
            .all(|n| !n.message.contains("region") && n.severity != Severity::Dropped),
        "{:?}",
        out.notes
    );
    Ok(())
}

/// A hole's box (x0, x1, y0, y1) and its radius when it is a whole circle.
type NearHole = ((f64, f64, f64, f64), Option<f64>);

/// The holes of the copper regions on one layer that lie near (x, 25): their boxes, and the
/// radius of each that is one whole circle (two half arcs).
fn holes_near(out: &ReadOutcome, layer: &str, x: f64) -> Vec<NearHole> {
    let mut found = Vec::new();
    for c in &out.board.copper {
        let CopperItem::Region {
            layer: l, holes, ..
        } = c
        else {
            continue;
        };
        if out.board.layers.get(*l).map(|l| l.name.as_str()) != Some(layer) {
            continue;
        }
        for h in holes {
            let b = contour_box(h);
            let (cx, cy) = ((b.0 + b.1) / 2.0, (b.2 + b.3) / 2.0);
            if (cx - x).abs() < 1.5 && (cy - 25.0).abs() < 1.5 {
                let radius = match h.segments.as_slice() {
                    [board_model::Segment::Arc { radius: a, .. }, board_model::Segment::Arc { radius: b, .. }]
                        if (a - b).abs() < 1e-9 =>
                    {
                        Some(*a)
                    }
                    _ => None,
                };
                found.push((b, radius));
            }
        }
    }
    found
}

/// plane-polygon-connect: free GND pads R, D and N (round 1.600, hole 0.800) at (6, 25),
/// (12, 25) and (18, 25) under a GND pour on Top Layer and on the GND plane Layer 1, with rules
/// NoConnect for N (priority 1), Direct for D (2) and the board's Relief (3).
/// - Top Layer (Altium's pour, read as poured): R four relief gaps, D none, N one round
///   clearance 2.126 across.
/// - Layer 1 (rebuilt from the rules): R a relief, its ring from 0.400 + 0.508 = 0.908 to 1.162
///   about the center, crossed by spokes; D nothing (solid to the drill); N a round clearance of
///   0.400 + 0.300 = 0.700, as Layer 2 (another net) has round all three.
#[test]
fn plane_and_pour_joins_follow_the_rules_for_single_pads() -> Result<(), Box<dyn std::error::Error>>
{
    let out = read("plane-polygon-connect.PcbDoc")?;
    let near = |a: f64, b: f64| (a - b).abs() <= TOLERANCE;

    // Top Layer pour.
    assert_eq!(holes_near(&out, "Top Layer", 6.0).len(), 4);
    assert!(holes_near(&out, "Top Layer", 12.0).is_empty());
    let n = holes_near(&out, "Top Layer", 18.0);
    let [(b, _)] = n.as_slice() else {
        return Err(format!("{n:?}").into());
    };
    assert!(near(b.1 - b.0, 2.126) && near(b.3 - b.2, 2.126), "{b:?}");

    // Layer 1 plane.
    let r = holes_near(&out, "Layer 1", 6.0);
    assert_eq!(r.len(), 4, "{r:?}");
    for (b, _) in &r {
        // Each piece of air lies between 0.908 and 1.162 from (6, 25) along an axis.
        let far = (b.0 - 6.0)
            .abs()
            .max((b.1 - 6.0).abs())
            .max((b.2 - 25.0).abs())
            .max((b.3 - 25.0).abs());
        assert!(far <= 1.162 + TOLERANCE, "{b:?}");
    }
    let outer: f64 = r
        .iter()
        .map(|(b, _)| (b.1 - 6.0).max(6.0 - b.0))
        .fold(0.0, f64::max);
    assert!(near(outer, 0.907), "{outer}");
    assert!(holes_near(&out, "Layer 1", 12.0).is_empty());
    let n = holes_near(&out, "Layer 1", 18.0);
    let [(b, Some(radius))] = n.as_slice() else {
        return Err(format!("{n:?}").into());
    };
    assert!(
        near(*radius, 0.7) && near((b.0 + b.1) / 2.0, 18.0),
        "{b:?} {radius}"
    );

    // Layer 2, another net: a round clearance of 0.700 round each.
    for x in [6.0, 12.0, 18.0] {
        let h = holes_near(&out, "Layer 2", x);
        assert!(
            matches!(h.as_slice(), [(_, Some(r))] if near(*r, 0.7)),
            "{x}: {h:?}"
        );
    }
    assert!(
        out.notes
            .iter()
            .all(|n| !n.message.contains("for some objects only")),
        "{:?}",
        out.notes
    );
    Ok(())
}
