//! Roadmap step 3, first part: tracks and pads, checked against the numbers in
//! tests/boards/README.txt (to 0.001 mm, as AGENTS.md asks).

use altium_pcbdoc::Altium;
use board_model::{CopperItem, InputFormat, LayerKind, PadShape, ReadOutcome, Side};

const TOLERANCE: f64 = 0.001;

fn read(name: &str) -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards")
        .join(name);
    Ok(Altium.read(&std::fs::read(path)?)?)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOLERANCE
}

/// The name of the layer at `index` in the board's layer list.
fn layer_name(out: &ReadOutcome, index: usize) -> &str {
    out.board
        .layers
        .get(index)
        .map(|l| l.name.as_str())
        .unwrap_or("(no such layer)")
}

#[test]
fn one_track_has_one_top_layer_track() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-track.PcbDoc")?;
    assert_eq!(out.board.copper.len(), 1, "{:?}", out.board.copper);
    let CopperItem::Track {
        layer,
        from,
        to,
        width,
        net,
    } = &out.board.copper[0]
    else {
        panic!("expected a track, got {:?}", out.board.copper[0]);
    };
    assert_eq!(layer_name(&out, *layer), "Top Layer");
    let top = &out.board.layers[*layer];
    assert_eq!((top.kind, top.side), (LayerKind::Copper, Some(Side::Top)));
    assert!(near(from.x, 10.0) && near(from.y, 15.0), "{from:?}");
    assert!(near(to.x, 20.0) && near(to.y, 15.0), "{to:?}");
    assert!(near(*width, 0.5), "{width}");
    assert_eq!(net, &None);
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn the_hole_is_an_unplated_3_mm_pad_on_all_layers() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("outline-with-hole.PcbDoc")?;
    assert_eq!(out.board.copper.len(), 1, "{:?}", out.board.copper);
    let CopperItem::Pad {
        layer,
        at,
        shape,
        size_x,
        size_y,
        rotation_deg,
        drill,
        ..
    } = &out.board.copper[0]
    else {
        panic!("expected a pad, got {:?}", out.board.copper[0]);
    };
    assert_eq!(layer_name(&out, *layer), "Multi-Layer");
    assert!(near(at.x, 10.0) && near(at.y, 15.0), "{at:?}");
    assert_eq!(*shape, PadShape::Round);
    assert!(near(*size_x, 3.0) && near(*size_y, 3.0));
    assert_eq!(*rotation_deg, 0.0);
    assert!(drill.is_some_and(|d| near(d, 3.0)), "{drill:?}");
    Ok(())
}

#[test]
fn the_resistor_has_two_0_8_by_1_0_smd_pads() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-resistor.PcbDoc")?;
    let mut centers = Vec::new();
    for item in &out.board.copper {
        let CopperItem::Pad {
            layer,
            at,
            shape,
            size_x,
            size_y,
            drill,
            ..
        } = item
        else {
            panic!("expected only pads, got {item:?}");
        };
        assert_eq!(layer_name(&out, *layer), "Top Layer");
        assert_eq!(*shape, PadShape::Rectangle);
        assert!(
            near(*size_x, 0.8) && near(*size_y, 1.0),
            "{size_x} x {size_y}"
        );
        assert_eq!(*drill, None);
        centers.push((at.x, at.y));
    }
    centers.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(centers.len(), 2, "{centers:?}");
    assert!(
        near(centers[0].0, 24.2) && near(centers[0].1, 15.0),
        "{centers:?}"
    );
    assert!(
        near(centers[1].0, 25.8) && near(centers[1].1, 15.0),
        "{centers:?}"
    );
    Ok(())
}

#[test]
fn a_board_without_copper_has_none() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("empty-outline.PcbDoc")?;
    assert!(out.board.copper.is_empty());
    Ok(())
}

/// The one pad on a board.
fn only_pad(out: &ReadOutcome) -> Option<&CopperItem> {
    match out.board.copper.as_slice() {
        [one] => Some(one),
        _ => None,
    }
}

#[test]
fn one_via_is_1_mm_with_a_0_5_mm_hole() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-via.PcbDoc")?;
    assert_eq!(out.board.copper.len(), 1, "{:?}", out.board.copper);
    let CopperItem::Via {
        at,
        diameter,
        drill,
        ..
    } = &out.board.copper[0]
    else {
        panic!("expected a via, got {:?}", out.board.copper[0]);
    };
    assert!(near(at.x, 25.0) && near(at.y, 15.0), "{at:?}");
    assert!(near(*diameter, 1.0), "{diameter}");
    assert!(near(*drill, 0.5), "{drill}");
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn one_plated_pad_is_plated_and_the_hole_is_not() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-plated-pad.PcbDoc")?;
    let Some(CopperItem::Pad {
        layer,
        at,
        shape,
        size_x,
        size_y,
        rotation_deg,
        drill,
        plated,
        ..
    }) = only_pad(&out)
    else {
        panic!("expected a pad");
    };
    assert_eq!(layer_name(&out, *layer), "Multi-Layer");
    assert!(near(at.x, 25.0) && near(at.y, 15.0), "{at:?}");
    assert_eq!(*shape, PadShape::Round);
    assert!(near(*size_x, 2.0) && near(*size_y, 2.0));
    assert_eq!(*rotation_deg, 0.0);
    assert!(drill.is_some_and(|d| near(d, 1.0)), "{drill:?}");
    assert!(*plated);
    assert!(out.notes.is_empty(), "{:?}", out.notes);

    let hole = read("outline-with-hole.PcbDoc")?;
    let Some(CopperItem::Pad { plated, .. }) = only_pad(&hole) else {
        panic!("expected a pad");
    };
    assert!(!*plated);
    Ok(())
}

#[test]
fn one_rotated_pad_is_2_by_1_at_30_degrees() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-rotated-pad.PcbDoc")?;
    let Some(CopperItem::Pad {
        layer,
        at,
        shape,
        size_x,
        size_y,
        rotation_deg,
        drill,
        plated,
        ..
    }) = only_pad(&out)
    else {
        panic!("expected a pad");
    };
    assert_eq!(layer_name(&out, *layer), "Top Layer");
    assert!(near(at.x, 25.0) && near(at.y, 15.0), "{at:?}");
    assert_eq!(*shape, PadShape::Rectangle);
    // Sizes are before rotation, as typed.
    assert!(
        near(*size_x, 2.0) && near(*size_y, 1.0),
        "{size_x} x {size_y}"
    );
    assert!(near(*rotation_deg, 30.0), "{rotation_deg}");
    assert_eq!((*drill, *plated), (None, false));
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn one_oval_pad_is_an_oval_2_by_1() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-oval-pad.PcbDoc")?;
    let Some(CopperItem::Pad {
        layer,
        at,
        shape,
        size_x,
        size_y,
        rotation_deg,
        drill,
        ..
    }) = only_pad(&out)
    else {
        panic!("expected a pad");
    };
    assert_eq!(layer_name(&out, *layer), "Top Layer");
    assert!(near(at.x, 25.0) && near(at.y, 15.0), "{at:?}");
    assert_eq!(*shape, PadShape::Oval);
    assert!(
        near(*size_x, 2.0) && near(*size_y, 1.0),
        "{size_x} x {size_y}"
    );
    assert_eq!(*rotation_deg, 0.0);
    assert_eq!(*drill, None);
    Ok(())
}

#[test]
fn one_bottom_track_is_on_the_bottom_layer() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-bottom-track.PcbDoc")?;
    assert_eq!(out.board.copper.len(), 1, "{:?}", out.board.copper);
    let CopperItem::Track {
        layer,
        from,
        to,
        width,
        ..
    } = &out.board.copper[0]
    else {
        panic!("expected a track, got {:?}", out.board.copper[0]);
    };
    assert_eq!(layer_name(&out, *layer), "Bottom Layer");
    let bottom = &out.board.layers[*layer];
    assert_eq!(
        (bottom.kind, bottom.side),
        (LayerKind::Copper, Some(Side::Bottom))
    );
    assert!(near(from.x, 10.0) && near(from.y, 15.0), "{from:?}");
    assert!(near(to.x, 20.0) && near(to.y, 15.0), "{to:?}");
    assert!(near(*width, 0.5), "{width}");
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn one_arc_is_a_quarter_circle_of_radius_5() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-arc.PcbDoc")?;
    assert_eq!(out.board.copper.len(), 1, "{:?}", out.board.copper);
    let CopperItem::TrackArc {
        layer,
        center,
        radius,
        start_deg,
        end_deg,
        width,
        net,
    } = &out.board.copper[0]
    else {
        panic!("expected an arc, got {:?}", out.board.copper[0]);
    };
    assert_eq!(layer_name(&out, *layer), "Top Layer");
    assert!(near(center.x, 25.0) && near(center.y, 15.0), "{center:?}");
    assert!(near(*radius, 5.0), "{radius}");
    assert!(near(*start_deg, 0.0) && near(*end_deg, 90.0));
    assert!(near(*width, 0.5), "{width}");
    assert_eq!(net, &None);
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

/// The only copper region of a board: its layer name and its corners, sorted.
fn only_region(out: &ReadOutcome) -> Option<(String, Vec<(f64, f64)>)> {
    let [CopperItem::Region { layer, contour, .. }] = out.board.copper.as_slice() else {
        return None;
    };
    let mut corners: Vec<(f64, f64)> = contour
        .segments
        .iter()
        .map(|s| match s {
            board_model::Segment::Line { from, .. } => Some((from.x, from.y)),
            board_model::Segment::Arc { .. } => None,
        })
        .collect::<Option<_>>()?;
    corners.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    Some((layer_name(out, *layer).to_string(), corners))
}

fn same_corners(got: &[(f64, f64)], want: &[(f64, f64)]) -> bool {
    let mut want = want.to_vec();
    want.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    got.len() == want.len()
        && got
            .iter()
            .zip(&want)
            .all(|(g, w)| near(g.0, w.0) && near(g.1, w.1))
}

const SQUARE_20_30: [(f64, f64); 4] = [(20.0, 10.0), (30.0, 10.0), (30.0, 20.0), (20.0, 20.0)];

#[test]
fn one_fill_is_a_10_by_10_top_layer_region() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-fill.PcbDoc")?;
    let (layer, corners) = only_region(&out).ok_or("expected exactly one region")?;
    assert_eq!(layer, "Top Layer");
    assert!(same_corners(&corners, &SQUARE_20_30), "{corners:?}");
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn one_region_is_a_10_by_10_top_layer_region() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-region.PcbDoc")?;
    let (layer, corners) = only_region(&out).ok_or("expected exactly one region")?;
    assert_eq!(layer, "Top Layer");
    assert!(same_corners(&corners, &SQUARE_20_30), "{corners:?}");
    // Only the shape-based copy of the region is not read, and that is reported.
    assert!(
        out.notes
            .iter()
            .all(|n| n.message.contains("/ShapeBasedRegions6/Data")),
        "{:?}",
        out.notes
    );
    Ok(())
}

#[test]
fn one_pour_reads_its_poured_copper() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("one-pour.PcbDoc")?;
    let (layer, corners) = only_region(&out).ok_or("expected exactly one region")?;
    assert_eq!(layer, "Top Layer");
    // Nothing else is on the board, so Altium poured the whole outline it was given.
    assert!(
        same_corners(
            &corners,
            &[(5.0, 5.0), (45.0, 5.0), (45.0, 25.0), (5.0, 25.0)]
        ),
        "{corners:?}"
    );
    assert!(
        out.notes
            .iter()
            .any(|n| n.message.contains("copper of a polygon pour")),
        "{:?}",
        out.notes
    );
    Ok(())
}

/// /ShapeBasedRegions6/Data only repeats /Regions6/Data on these boards: no note about it.
#[test]
fn the_shape_based_copy_of_the_regions_raises_no_note() -> Result<(), Box<dyn std::error::Error>> {
    for name in [
        "one-region.PcbDoc",
        "one-pour.PcbDoc",
        "outline-with-hole.PcbDoc",
    ] {
        let out = read(name)?;
        assert!(
            !out.notes.iter().any(|n| n.message.contains("ShapeBased")),
            "{name}: {:?}",
            out.notes
        );
    }
    Ok(())
}

#[test]
fn four_layer_has_inner_layers_with_a_track_each() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("four-layer.PcbDoc")?;
    let copper: Vec<(&str, Option<Side>)> = out
        .board
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Copper && l.name != "Multi-Layer")
        .map(|l| (l.name.as_str(), l.side))
        .collect();
    assert_eq!(
        copper,
        [
            ("Top Layer", Some(Side::Top)),
            ("Layer 1", None),
            ("Layer 2", None),
            ("Bottom Layer", Some(Side::Bottom)),
        ]
    );
    // 1.600 of empty-outline, two more copper layers of 0.035 and two dielectrics of 2.8 mil.
    let t = out.board.thickness.ok_or("no thickness")?;
    assert!(
        near(t, 1.6 + 2.0 * 0.035 + 2.0 * 2.8 * 0.0254),
        "thickness {t}"
    );
    let mut tracks = Vec::new();
    let mut vias = Vec::new();
    for item in &out.board.copper {
        match item {
            CopperItem::Track {
                layer,
                from,
                to,
                width,
                ..
            } => tracks.push((layer_name(&out, *layer), *from, *to, *width)),
            CopperItem::Via {
                at,
                diameter,
                drill,
                ..
            } => vias.push((*at, *diameter, *drill)),
            other => return Err(format!("unexpected copper item {other:?}").into()),
        }
    }
    let [(l1, f1, t1, w1), (l2, f2, t2, w2)] = tracks.as_slice() else {
        return Err(format!("expected two tracks: {tracks:?}").into());
    };
    assert_eq!((*l1, *l2), ("Layer 1", "Layer 2"));
    for (from, to, y) in [(f1, t1, 10.0), (f2, t2, 20.0)] {
        assert!(
            near(from.x, 10.0) && near(from.y, y) && near(to.x, 20.0) && near(to.y, y),
            "{from:?} {to:?}"
        );
    }
    assert!(near(*w1, 0.25) && near(*w2, 0.25));
    let [(at, diameter, drill)] = vias.as_slice() else {
        return Err(format!("expected one via: {vias:?}").into());
    };
    assert!(near(at.x, 30.0) && near(at.y, 15.0), "{at:?}");
    assert!(near(*diameter, 0.6) && near(*drill, 0.3));
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    Ok(())
}

#[test]
fn small_board_pours_hand_over_their_holes() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("small-board.PcbDoc")?;
    let mut regions: Vec<&str> = out
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Region { layer, .. } => Some(layer_name(&out, *layer)),
            _ => None,
        })
        .collect();
    regions.sort_unstable();
    assert_eq!(regions, ["Bottom Layer", "Top Layer"]);
    // Each GND pour has 7 holes (clearances and thermal-relief gaps), 14 in all, and nothing is
    // left to warn about; the shape-based copy repeats them, so it gets no note of its own.
    let hole_counts: Vec<usize> = out
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Region { holes, .. } => Some(holes.len()),
            _ => None,
        })
        .collect();
    let mut hole_counts = hole_counts;
    hole_counts.sort_unstable();
    assert_eq!(hole_counts, [7, 7]);
    let all_lines = out.board.copper.iter().all(|c| match c {
        CopperItem::Region { holes, .. } => holes.iter().all(|h| {
            h.segments.len() >= 3
                && h.segments
                    .iter()
                    .all(|s| matches!(s, board_model::Segment::Line { .. }))
        }),
        _ => true,
    });
    assert!(all_lines);
    let warnings: Vec<&str> = out
        .notes
        .iter()
        .filter(|n| n.severity == board_model::Severity::Warning)
        .map(|n| n.message.as_str())
        .collect();
    assert!(warnings.is_empty(), "{warnings:?}");
    Ok(())
}

/// Where each copper layer of four-layer lies, from its V9 stack (tests/boards/README.txt): Top
/// Solder 0.010, Top Layer 0.035, a dielectric of 2.8 mil, Layer 1 0.035, FR-4 1.510, Layer 2
/// 0.035, a dielectric of 2.8 mil, Bottom Layer 0.035. Multi-Layer stands for all copper and has
/// no place.
#[test]
fn four_layers_copper_has_its_place_in_the_stack() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("four-layer.PcbDoc")?;
    let mil = 2.8 * 0.0254;
    let want = [
        ("Top Layer", 0.010),
        ("Layer 1", 0.010 + 0.035 + mil),
        ("Layer 2", 0.010 + 0.035 + mil + 0.035 + 1.510),
        (
            "Bottom Layer",
            0.010 + 0.035 + mil + 0.035 + 1.510 + 0.035 + mil,
        ),
    ];
    for (name, depth) in want {
        let layer = out
            .board
            .layers
            .iter()
            .find(|l| l.name == name)
            .ok_or(name)?;
        let place = layer.position.ok_or(name)?;
        assert!(
            near(place.depth, depth) && near(place.thickness, 0.035),
            "{name}: {place:?}"
        );
    }
    let multi = out
        .board
        .layers
        .iter()
        .find(|l| l.name == "Multi-Layer")
        .ok_or("no Multi-Layer")?;
    assert_eq!(multi.position, None);
    Ok(())
}

/// A rounded-rectangle pad: x, y, size x, size y, rotation, corner percent and hole.
type RoundedPad = (f64, f64, f64, f64, f64, f64, Option<f64>);

#[test]
fn pad_shapes_has_rounded_rectangles_and_octagons() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("pad-shapes.PcbDoc")?;
    // Rounded rectangles: (x, y, size x, size y, rotation, corner percent, hole).
    let rounded: [RoundedPad; 4] = [
        (10.0, 15.0, 2.0, 1.0, 0.0, 50.0, None),
        (20.0, 15.0, 2.0, 1.0, 0.0, 25.0, None),
        (30.0, 15.0, 2.0, 1.0, 30.0, 25.0, None),
        (20.0, 24.0, 1.6, 1.6, 0.0, 30.0, Some(0.8)),
    ];
    for (x, y, w, h, rot, percent, hole) in rounded {
        let found = out.board.copper.iter().find_map(|c| match c {
            CopperItem::Pad {
                at,
                shape,
                size_x,
                size_y,
                rotation_deg,
                drill,
                plated,
                ..
            } if near(at.x, x) && near(at.y, y) => {
                Some((*shape, *size_x, *size_y, *rotation_deg, *drill, *plated))
            }
            _ => None,
        });
        let (shape, size_x, size_y, rotation, drill, plated) =
            found.ok_or(format!("no pad at ({x}, {y})"))?;
        // The corner radius is the percentage of half the shorter side.
        let PadShape::RoundedRectangle { radius_ratio } = shape else {
            return Err(format!("pad at ({x}, {y}) is {shape:?}").into());
        };
        let radius = radius_ratio * w.min(h);
        assert!(near(radius, percent / 100.0 * w.min(h) / 2.0), "{radius}");
        assert!(near(size_x, w) && near(size_y, h) && near(rotation, rot));
        assert_eq!(drill.is_some(), hole.is_some());
        if let (Some(d), Some(want)) = (drill, hole) {
            assert!(near(d, want) && plated);
        }
    }
    // Octagons: each corner cut off by a quarter of the shorter side.
    let mut octagons: Vec<Vec<(f64, f64)>> = out
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::PolygonPad {
                shapes,
                drill: None,
                ..
            } => shapes.first().map(|s| {
                s.segments
                    .iter()
                    .filter_map(|seg| match *seg {
                        board_model::Segment::Line { from, .. } => Some((from.x, from.y)),
                        board_model::Segment::Arc { .. } => None,
                    })
                    .collect()
            }),
            _ => None,
        })
        .collect();
    octagons.sort_by(|a, b| {
        a.len().cmp(&b.len()).then(
            a.first()
                .partial_cmp(&b.first())
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    let want = |cx: f64, cy: f64, w: f64, h: f64| -> Vec<(f64, f64)> {
        let (a, b, c) = (w / 2.0, h / 2.0, w.min(h) / 4.0);
        [
            (a, -b + c),
            (a, b - c),
            (a - c, b),
            (-a + c, b),
            (-a, b - c),
            (-a, -b + c),
            (-a + c, -b),
            (a - c, -b),
        ]
        .iter()
        .map(|(x, y)| (cx + x, cy + y))
        .collect()
    };
    for corners in [want(40.0, 15.0, 2.0, 1.0), want(40.0, 24.0, 1.5, 1.5)] {
        assert!(
            octagons.iter().any(|o| o.len() == 8
                && o.iter()
                    .zip(&corners)
                    .all(|(p, q)| near(p.0, q.0) && near(p.1, q.1))),
            "{corners:?} not in {octagons:?}"
        );
    }
    let warnings: Vec<&str> = out
        .notes
        .iter()
        .filter(|n| n.severity == board_model::Severity::Warning)
        .map(|n| n.message.as_str())
        .collect();
    assert!(warnings.is_empty(), "{warnings:?}");
    Ok(())
}
