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

/// text-kinds: twelve DejaVu Sans Mono texts on Top Overlay. The nine 10 x 4 frames (stored at
/// their lower-left corners, x 3, 18 and 33) are placed at their justification's point of the
/// frame, anchored there by the same part of the text's box; PLAIN, INV and OFS keep their
/// location and Altium's left-baseline anchor. INV (margin 0.5) and OFS (offset 0.3) are
/// inverted, which is not drawn yet: a note names each.
#[test]
fn frames_place_their_text_and_inverted_texts_get_a_note() -> Result<(), Box<dyn std::error::Error>>
{
    use board_model::{HorizontalAnchor as H, TextAnchor, VerticalAnchor as V};
    let out = read("text-kinds.PcbDoc")?;
    let anchor = |horizontal, vertical| TextAnchor {
        horizontal,
        vertical,
    };
    let want = [
        ("LT", 3.0, 21.0, anchor(H::Left, V::Top)),
        ("CT", 23.0, 20.988, anchor(H::Center, V::Top)),
        ("RT", 43.0, 21.0, anchor(H::Right, V::Top)),
        ("LM", 3.0, 12.5, anchor(H::Left, V::Center)),
        ("CM", 23.0, 12.488, anchor(H::Center, V::Center)),
        ("RM", 43.0, 12.5, anchor(H::Right, V::Center)),
        ("LB", 3.0, 4.0, anchor(H::Left, V::Bottom)),
        ("CB", 23.0, 3.988, anchor(H::Center, V::Bottom)),
        ("RB", 43.0, 4.0, anchor(H::Right, V::Bottom)),
        ("PLAIN", 33.0, 25.0, TextAnchor::default()),
        ("INV", 3.0, 25.0, TextAnchor::default()),
        ("OFS", 18.0, 25.0, TextAnchor::default()),
    ];
    assert_eq!(out.board.texts.len(), want.len());
    for (content, x, y, a) in want {
        let t = out
            .board
            .texts
            .iter()
            .find(|t| t.content == content)
            .ok_or(content)?;
        assert!(
            (t.at.x - x).abs() <= TOLERANCE && (t.at.y - y).abs() <= TOLERANCE,
            "{content}: {:?}",
            t.at
        );
        assert_eq!(t.anchor, a, "{content}");
    }
    let mut warnings: Vec<&str> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Warning)
        .map(|n| n.message.as_str())
        .collect();
    warnings.sort_unstable();
    assert_eq!(
        warnings,
        [
            "the inverted text \"INV\" at (3.000, 25.000), margin 0.500 is drawn as plain \
             letters: its letters cut out of a filled box are not drawn yet",
            "the inverted text \"OFS\" at (18.000, 25.000), offset 0.300 is drawn as plain \
             letters: its letters cut out of a filled box are not drawn yet",
        ]
    );
    Ok(())
}

/// pad-shapes-custom: pad 1 a 2.000 x 1.000 rounded rectangle (50%, radius 0.25) at (10, 15);
/// pad 2 at (20, 15) a 2.000 x 1.000 rectangle with its upper corners cut by 0.25; pad 3 at
/// (30.5, 14.5) an L from (30, 14) to (33, 17). Pads 2 and 3 store only a 1 mil square; their
/// copper is the region that names them (PADINDEX), so they are polygon pads of that outline,
/// and the region is not also read as free copper.
#[test]
fn chamfered_and_custom_pads_take_their_region_outline() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("pad-shapes-custom.PcbDoc")?;
    let corners = |c: &board_model::Contour| -> Vec<(f64, f64)> {
        c.segments
            .iter()
            .filter_map(|s| match s {
                board_model::Segment::Line { from, .. } => Some((from.x, from.y)),
                _ => None,
            })
            .collect()
    };
    let same = |got: &[(f64, f64)], want: &[(f64, f64)]| {
        got.len() == want.len()
            && want.iter().all(|w| {
                got.iter()
                    .any(|g| (g.0 - w.0).abs() <= TOLERANCE && (g.1 - w.1).abs() <= TOLERANCE)
            })
    };
    let (mut rounded, mut polygons) = (Vec::new(), Vec::new());
    for c in &out.board.copper {
        match c {
            CopperItem::Pad {
                at,
                shape,
                size_x,
                size_y,
                ..
            } => rounded.push((at.x, at.y, *shape, *size_x, *size_y)),
            CopperItem::PolygonPad { at, shapes, .. } => polygons.push((at.x, at.y, shapes)),
            other => return Err(format!("{other:?}").into()),
        }
    }
    let [(x, y, shape, sx, sy)] = rounded.as_slice() else {
        return Err(format!("{rounded:?}").into());
    };
    assert!((x - 10.0).abs() <= TOLERANCE && (y - 15.0).abs() <= TOLERANCE);
    assert_eq!(
        *shape,
        board_model::PadShape::RoundedRectangle { radius_ratio: 0.25 }
    );
    assert!((sx - 2.0).abs() <= TOLERANCE && (sy - 1.0).abs() <= TOLERANCE);
    polygons.sort_by(|a, b| a.0.total_cmp(&b.0));
    let [(x2, y2, s2), (x3, y3, s3)] = polygons.as_slice() else {
        return Err(format!("{polygons:?}").into());
    };
    assert!((x2 - 20.0).abs() <= TOLERANCE && (y2 - 15.0).abs() <= TOLERANCE);
    assert!((x3 - 30.5).abs() <= TOLERANCE && (y3 - 14.5).abs() <= TOLERANCE);
    let [chamfered] = s2.as_slice() else {
        return Err(format!("{s2:?}").into());
    };
    let [l] = s3.as_slice() else {
        return Err(format!("{s3:?}").into());
    };
    assert!(same(
        &corners(chamfered),
        &[
            (19.0, 14.5),
            (21.0, 14.5),
            (21.0, 15.25),
            (20.75, 15.5),
            (19.25, 15.5),
            (19.0, 15.25)
        ]
    ));
    assert!(same(
        &corners(l),
        &[
            (30.0, 14.0),
            (33.0, 14.0),
            (33.0, 15.0),
            (31.0, 15.0),
            (31.0, 17.0),
            (30.0, 17.0)
        ]
    ));
    Ok(())
}
