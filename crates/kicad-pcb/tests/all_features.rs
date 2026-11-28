//! The KiCad all-features board (`k10-all-features`): one KiCad 10 board with every feature the
//! KiCad reader handles, the twin of the Altium all-features board. When the reader learns
//! something new, add it to the board (fixtures/kicad/build_k10_boards.py), its README.txt and a
//! check here. Numbers from k10-all-features.README.txt, read back with KiCad's own scripting
//! (page coordinates; model x = 100 + X, model y = -(100 + Y)).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use board_model::{
    Contour, CopperItem, Graphic, HorizontalAnchor, LayerKind, ModelSource, PadShape, Point,
    ReadOutcome, Segment, Severity, Side, TextFont,
};
use kicad_pcb::{project_variables, read_with, Variables};

const TOLERANCE: f64 = 0.001;

fn file(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/boards/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// The board as `dutchman-electronics read` reads it: with its file name and its project.
fn read() -> ReadOutcome {
    let variables = Variables {
        file_name: Some("k10-all-features.kicad_pcb".into()),
        project: project_variables(&file("k10-all-features.kicad_pro")).unwrap(),
    };
    read_with(&file("k10-all-features.kicad_pcb"), &variables).unwrap()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOLERANCE
}

/// True when `p` is KiCad's (x, y) on the page, relative to the board's corner.
fn at(p: Point, x: f64, y: f64) -> bool {
    near(p.x, 100.0 + x) && near(p.y, -(100.0 + y))
}

fn layer(o: &ReadOutcome, name: &str) -> usize {
    o.board
        .layers
        .iter()
        .position(|l| l.name == name)
        .unwrap_or_else(|| panic!("no layer {name}"))
}

/// The area of a contour (Green's theorem, arcs sampled finely).
fn area(c: &Contour) -> f64 {
    let mut ring: Vec<Point> = Vec::new();
    for s in &c.segments {
        match *s {
            Segment::Line { from, .. } => ring.push(from),
            Segment::Arc {
                center,
                radius,
                start_deg,
                ..
            } => {
                let sweep = s.sweep_deg().unwrap();
                for k in 0..720 {
                    let a = (start_deg + sweep * f64::from(k) / 720.0).to_radians();
                    ring.push(Point {
                        x: center.x + radius * a.cos(),
                        y: center.y + radius * a.sin(),
                    });
                }
            }
        }
    }
    let n = ring.len();
    ((0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0)
        .abs()
}

/// Everything not read yet is named in a note; this is the list of what this board still loses.
/// A note that is not Info and not on this list is a regression; an item that drops off the list
/// is progress (remove it here and in the README).
#[test]
fn only_the_known_gaps_are_warned_about() {
    let o = read();
    let known = [
        "3 text(s) on layer F.Fab (only silkscreen and copper texts are read)",
        "1 text(s) on layer B.Fab (only silkscreen and copper texts are read)",
    ];
    let unexpected: Vec<&str> = o
        .notes
        .iter()
        .filter(|n| n.severity != Severity::Info)
        .map(|n| n.message.as_str())
        .filter(|m| !known.contains(m))
        .collect();
    assert!(unexpected.is_empty(), "{unexpected:?}");
    // The Info notes on what is not read: the stack-up's finish and materials, and the model
    // files the board names but does not hold.
    for info in [
        "1 board(s) whose stack-up colors, materials, copper weights and finish were not read",
        "2 3D model file(s) named by the board, not stored in it: first R_0603.step",
    ] {
        assert!(o.notes.iter().any(|n| n.message == info), "{info}");
    }
}

#[test]
fn the_outline_has_four_rounded_corners_and_two_cutouts() {
    let o = read();
    let outline = &o.board.outline;
    let arcs: Vec<(Point, f64)> = outline
        .outer
        .segments
        .iter()
        .filter_map(|s| match *s {
            Segment::Arc { center, radius, .. } => Some((center, radius)),
            Segment::Line { .. } => None,
        })
        .collect();
    assert_eq!(outline.outer.segments.len(), 8);
    assert_eq!(arcs.len(), 4);
    for (x, y) in [(97.0, 3.0), (97.0, 67.0), (3.0, 67.0), (3.0, 3.0)] {
        assert!(
            arcs.iter()
                .any(|(c, r)| at(*c, x, y) && (r - 3.0).abs() < 1e-5),
            "corner about ({x}, {y})"
        );
    }
    // 100 x 70 less the four corners: 7000 - (4 - pi) * 9.
    let expected = 7000.0 - (4.0 - std::f64::consts::PI) * 9.0;
    assert!(
        (area(&outline.outer) - expected).abs() < 0.01,
        "{}",
        area(&outline.outer)
    );
    assert_eq!(outline.cutouts.len(), 2);
    let circle = outline
        .cutouts
        .iter()
        .find_map(|c| match c.segments.as_slice() {
            [Segment::Arc { center, radius, .. }] => Some((*center, *radius)),
            _ => None,
        })
        .unwrap();
    assert!(at(circle.0, 90.0, 60.0) && near(circle.1, 2.0));
    let rect = outline
        .cutouts
        .iter()
        .find(|c| c.segments.len() == 4)
        .unwrap();
    // (80, 6) to (86, 12): 6 x 6.
    assert!((area(rect) - 36.0).abs() < TOLERANCE);
    for s in &rect.segments {
        let Segment::Line { from, .. } = *s else {
            panic!("a rectangle has straight sides")
        };
        assert!(
            [(80.0, 6.0), (86.0, 6.0), (86.0, 12.0), (80.0, 12.0)]
                .iter()
                .any(|(x, y)| at(from, *x, *y)),
            "{from:?}"
        );
    }
}

#[test]
fn four_copper_layers_placed_by_the_stack_up() {
    let o = read();
    assert!(near(o.board.thickness.unwrap(), 1.6));
    assert_eq!(o.board.solder_mask_expansion, Some(0.0));
    // (layer, depth of its top from the board's top, thickness): F.Mask 0.01 lies above F.Cu.
    for (name, depth, thickness) in [
        ("F.Cu", 0.01, 0.035),
        ("In1.Cu", 0.245, 0.0175),
        ("In2.Cu", 1.3375, 0.0175),
        ("B.Cu", 1.555, 0.035),
    ] {
        let l = &o.board.layers[layer(&o, name)];
        assert_eq!(l.kind, LayerKind::Copper, "{name}");
        let p = l.position.unwrap();
        assert!(
            near(p.depth, depth) && near(p.thickness, thickness),
            "{name}: {p:?}"
        );
    }
}

#[test]
fn u1_has_a_pad_of_every_shape() {
    let o = read();
    let top = layer(&o, "F.Cu");
    let all = layer(&o, "*.Cu");
    let pads: Vec<_> = o
        .board
        .copper
        .iter()
        .filter(|c| {
            matches!(c, CopperItem::Pad { owner: Some(n), .. } | CopperItem::PolygonPad { owner: Some(n), .. } if n == "U1")
        })
        .collect();
    assert_eq!(pads.len(), 10);
    let pad_at = |x: f64| {
        pads.iter()
            .find(|c| match c {
                CopperItem::Pad { at: p, .. } | CopperItem::PolygonPad { at: p, .. } => {
                    at(*p, x, 8.0)
                }
                _ => false,
            })
            .unwrap_or_else(|| panic!("no U1 pad at x = {x}"))
    };
    let plain = |x: f64| match pad_at(x) {
        CopperItem::Pad {
            layer,
            shape,
            size_x,
            size_y,
            drill,
            plated,
            ..
        } => (*layer, *shape, *size_x, *size_y, *drill, *plated),
        other => panic!("x = {x}: {other:?}"),
    };
    let (l, s, w, h, d, _) = plain(12.0);
    assert!(l == top && s == PadShape::Round && near(w, 1.5) && near(h, 1.5) && d.is_none());
    let (l, s, w, h, ..) = plain(16.0);
    assert!(l == top && s == PadShape::Rectangle && near(w, 2.0) && near(h, 1.5));
    let (_, s, w, h, ..) = plain(20.0);
    assert!(s == PadShape::Oval && near(w, 2.5) && near(h, 1.5));
    let (_, s, w, h, ..) = plain(24.0);
    assert!(
        matches!(s, PadShape::RoundedRectangle { radius_ratio } if near(radius_ratio, 0.25))
            && near(w, 2.0)
            && near(h, 1.5)
    );
    // Pads 5 to 8 come as outlines; their areas against KiCad's.
    let polygon_area = |x: f64| match pad_at(x) {
        CopperItem::PolygonPad { layer, shapes, .. } => {
            assert_eq!(*layer, top, "x = {x}");
            shapes.iter().map(area).collect::<Vec<f64>>()
        }
        other => panic!("x = {x}: {other:?}"),
    };
    // 5: chamfered rectangle, two corners cut by 0.375 and the other two rounded off by 0.375
    // (KiCad keeps the rounding ratio 0.25 on the corners it does not chamfer).
    let chamfered = polygon_area(28.0);
    let r: f64 = 0.375;
    let expected = 3.0 - 2.0 * r * r / 2.0 - 2.0 * (r * r - std::f64::consts::PI * r * r / 4.0);
    assert!(
        (chamfered.iter().sum::<f64>() - expected).abs() < 0.001,
        "{chamfered:?} {expected}"
    );
    // 6: trapezoid, KiCad's polygon has area 3.0.
    assert!((polygon_area(32.0).iter().sum::<f64>() - 3.0).abs() < TOLERANCE);
    // 7: the 0.8 square anchor and the L, (0, 0) to (1.5, 1.5) less (0.6, 0.6) to (1.5, 1.5).
    let custom = polygon_area(36.0);
    assert_eq!(custom.len(), 2);
    assert!(custom.iter().any(|a| near(*a, 0.64)) && custom.iter().any(|a| near(*a, 1.44)));
    // 8: the 1.0 circle anchor and the D of radius 1.2, an arc, not facets.
    let d = polygon_area(40.0);
    assert_eq!(d.len(), 2);
    let half_disc = std::f64::consts::PI * 1.44 / 2.0;
    assert!(d.iter().any(|a| (a - half_disc).abs() < 0.001), "{d:?}");
    if let CopperItem::PolygonPad { shapes, .. } = pad_at(40.0) {
        assert!(shapes.iter().any(|c| c
            .segments
            .iter()
            .any(|s| matches!(s, Segment::Arc { radius, .. } if near(*radius, 1.2)))));
    }
    let (l, s, w, _, d, plated) = plain(44.0);
    assert!(l == all && s == PadShape::Round && near(w, 1.6) && d == Some(0.8) && plated);
    let (l, _, w, _, d, plated) = plain(48.0);
    assert!(l == all && near(w, 1.0) && d == Some(1.0) && !plated);
}

#[test]
fn the_zone_fill_keeps_clear_of_the_via_and_relieves_the_pad() {
    let o = read();
    let top = layer(&o, "F.Cu");
    let regions: Vec<_> = o
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Region {
                layer,
                contour,
                holes,
            } => Some((*layer, contour, holes)),
            _ => None,
        })
        .collect();
    assert_eq!(regions.len(), 1);
    let (l, contour, holes) = regions[0];
    assert_eq!(l, top);
    assert_eq!(holes.len(), 2);
    let copper = area(contour) - holes.iter().map(area).sum::<f64>();
    assert!((copper - 793.948).abs() < 0.01, "{copper}");
    let via_gap = holes
        .iter()
        .map(area)
        .find(|a| (a - std::f64::consts::PI * 0.81).abs() < 0.05);
    let pad_gap = holes.iter().map(area).find(|a| (a - 3.470).abs() < 0.05);
    assert!(via_gap.is_some() && pad_gap.is_some());
    let j1 = o
        .board
        .copper
        .iter()
        .find(|c| matches!(c, CopperItem::Pad { owner: Some(n), .. } if n == "J1"))
        .unwrap();
    assert!(
        matches!(j1, CopperItem::Pad { at: p, drill: Some(d), plated: true, .. } if at(*p, 15.0, 28.0) && near(*d, 0.8))
    );
}

#[test]
fn tracks_and_an_arc_on_both_sides() {
    let o = read();
    let (top, bottom) = (layer(&o, "F.Cu"), layer(&o, "B.Cu"));
    let tracks: Vec<_> = o
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Track {
                layer,
                from,
                to,
                width,
                net,
            } => Some((*layer, *from, *to, *width, net.as_deref())),
            _ => None,
        })
        .collect();
    assert_eq!(tracks.len(), 2);
    assert!(tracks.iter().any(|(l, f, t, w, n)| *l == top
        && at(*f, 5.0, 45.0)
        && at(*t, 20.0, 45.0)
        && near(*w, 0.25)
        && *n == Some("SIG")));
    assert!(tracks.iter().any(|(l, f, t, w, _)| *l == bottom
        && at(*f, 30.0, 45.0)
        && at(*t, 40.0, 45.0)
        && near(*w, 0.3)));
    let arcs: Vec<_> = o
        .board
        .copper
        .iter()
        .filter(|c| matches!(c, CopperItem::TrackArc { .. }))
        .collect();
    assert_eq!(arcs.len(), 1);
    let CopperItem::TrackArc {
        layer: l,
        center,
        radius,
        start_deg,
        end_deg,
        width,
        ..
    } = arcs[0]
    else {
        panic!("filtered to track arcs")
    };
    // The half above (25, 45) in KiCad's terms is the half with larger model y: 0 to 180.
    assert!(*l == top && at(*center, 25.0, 45.0) && near(*radius, 5.0) && near(*width, 0.25));
    assert!(
        near(*start_deg, 0.0) && near(*end_deg, 180.0),
        "{start_deg} {end_deg}"
    );
}

#[test]
fn vias_of_every_kind_with_their_tenting() {
    let o = read();
    let (top, in1, in2) = (layer(&o, "F.Cu"), layer(&o, "In1.Cu"), layer(&o, "In2.Cu"));
    let via_at = |x: f64, y: f64| {
        o.board
            .copper
            .iter()
            .find_map(|c| match c {
                CopperItem::Via {
                    at: p,
                    diameter,
                    drill,
                    tented,
                    layers,
                    ..
                } if at(*p, x, y) => Some((*diameter, *drill, *tented, *layers)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no via at ({x}, {y})"))
    };
    let vias = o
        .board
        .copper
        .iter()
        .filter(|c| matches!(c, CopperItem::Via { .. }))
        .count();
    assert_eq!(vias, 8);
    for (x, y) in [(30.0, 28.0), (30.0, 45.0), (5.0, 50.0), (10.0, 50.0)] {
        let (d, h, t, l) = via_at(x, y);
        assert!(near(d, 0.8) && near(h, 0.4) && l.is_none(), "({x}, {y})");
        assert_eq!(t, [Some(true), Some(true)], "({x}, {y})");
    }
    assert_eq!(via_at(15.0, 50.0).2, [Some(false), Some(false)]);
    assert_eq!(via_at(20.0, 50.0).3, Some((top, in1)));
    assert_eq!(via_at(25.0, 50.0).3, Some((in1, in2)));
    let (d, h, _, l) = via_at(30.0, 50.0);
    assert!(near(d, 0.5) && near(h, 0.2) && l == Some((top, in1)));
}

#[test]
fn silkscreen_shapes_on_the_top() {
    let o = read();
    let silk = layer(&o, "F.SilkS");
    let lines: Vec<_> = o
        .board
        .graphics
        .iter()
        .filter_map(|g| match g {
            Graphic::Line {
                layer,
                from,
                to,
                width,
                ..
            } if *layer == silk => Some((*from, *to, *width)),
            _ => None,
        })
        .collect();
    // The line, the rectangle's four sides and the polygon's four.
    assert_eq!(lines.len(), 9);
    assert!(lines
        .iter()
        .any(|(f, t, w)| at(*f, 50.0, 20.0) && at(*t, 60.0, 20.0) && near(*w, 0.15)));
    for (x, y) in [(80.0, 19.0), (85.0, 24.0), (88.0, 19.0), (93.0, 24.0)] {
        assert!(
            lines.iter().any(|(f, _, _)| at(*f, x, y)),
            "a corner at ({x}, {y})"
        );
    }
    let arcs: Vec<_> = o
        .board
        .graphics
        .iter()
        .filter_map(|g| match g {
            Graphic::Arc {
                layer,
                center,
                radius,
                start_deg,
                end_deg,
                width,
                ..
            } if *layer == silk => Some((*center, *radius, *start_deg, *end_deg, *width)),
            _ => None,
        })
        .collect();
    assert_eq!(arcs.len(), 2);
    // The quarter arc about (65, 23), radius 3 (KiCad rounds its middle point to a nanometer).
    assert!(arcs.iter().any(|(c, r, s, e, w)| (c.x - 165.0).abs() < 1e-5
        && (c.y + 123.0).abs() < 1e-5
        && (r - 3.0).abs() < 1e-5
        && ((e - s).rem_euclid(360.0) - 90.0).abs() < 0.001
        && near(*w, 0.2)));
    // The circle about (75, 22), radius 2.
    assert!(arcs
        .iter()
        .any(|(c, r, ..)| at(*c, 75.0, 22.0) && near(*r, 2.0)));
}

#[test]
fn texts_in_both_fonts_with_variables_and_on_the_bottom() {
    let o = read();
    let text = |content: &str| {
        let found: Vec<_> = o
            .board
            .texts
            .iter()
            .filter(|t| t.content == content)
            .collect();
        assert_eq!(found.len(), 1, "{content}");
        found[0]
    };
    let silk = layer(&o, "F.SilkS");
    let stroke = text("STROKE");
    assert!(stroke.layer == silk && at(stroke.at, 55.0, 30.0) && near(stroke.height, 1.5));
    assert!(matches!(stroke.font, TextFont::Stroke { width, .. } if near(width, 0.2)));
    assert_eq!(stroke.anchor.horizontal, HorizontalAnchor::Center);
    let tt = text("TrueType 789");
    assert!(at(tt.at, 65.0, 30.0) && near(tt.height, 2.0));
    assert_eq!(tt.anchor.horizontal, HorizontalAnchor::Left);
    assert!(
        matches!(&tt.font, TextFont::TrueType { name, bold: false, italic: false } if name == "DejaVu Sans Mono")
    );
    assert_eq!(o.fonts.len(), 1);
    assert_eq!(o.fonts[0].family, "DejaVu Sans Mono");
    // The text variables: the title block's revision and the project's PCB_NUMBER.
    assert!(at(text("Rev C1").at, 55.0, 35.0));
    assert!(at(text("DN-0100").at, 75.0, 35.0));
    assert!(o.notes.iter().any(|n| n.severity == Severity::Info
        && n.message == "2 text(s) use KiCad text variables and show their values, as KiCad does"));
    let bottom = text("BOTTOM");
    assert!(bottom.layer == layer(&o, "B.SilkS") && at(bottom.at, 60.0, 40.0) && bottom.mirrored);
    assert_eq!(o.board.name, "All Features");
}

#[test]
fn r1_and_r2_on_both_sides_with_models_and_shown_fields() {
    let o = read();
    let part = |name: &str| {
        o.board
            .components
            .iter()
            .find(|c| c.designator == name)
            .unwrap_or_else(|| panic!("no {name}"))
    };
    let r1 = part("R1");
    let r2 = part("R2");
    assert!(at(r1.at, 15.0, 58.0) && near(r1.rotation_deg, 0.0) && r1.side == Side::Top);
    assert!(at(r2.at, 35.0, 58.0) && near(r2.rotation_deg, 90.0) && r2.side == Side::Bottom);
    assert_eq!(o.board.components.len(), 4, "U1, J1, R1, R2");
    for r in [r1, r2] {
        let m = r.models.first().unwrap();
        assert!(matches!(&m.source, ModelSource::External { file, .. } if file == "R_0603.step"));
        assert!(near(m.offset[0], 0.1) && near(m.offset[1], 0.2) && near(m.offset[2], 0.0));
        // KiCad's rotation (0, 0, 90) turns the model the other way about each axis (see the
        // reader's model notes): -90 about Z in the model.
        assert!(near(m.rotation_deg[2], -90.0), "{:?}", m.rotation_deg);
    }
    let pads = |name: &str| {
        o.board
            .copper
            .iter()
            .filter_map(|c| match c {
                CopperItem::Pad {
                    owner: Some(n),
                    layer,
                    at: p,
                    rotation_deg,
                    ..
                } if n == name => Some((*layer, *p, *rotation_deg)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let r1_pads = pads("R1");
    assert!(r1_pads.iter().all(|(l, ..)| *l == layer(&o, "F.Cu")));
    assert!(r1_pads.iter().any(|(_, p, _)| at(*p, 14.2, 58.0)));
    assert!(r1_pads.iter().any(|(_, p, _)| at(*p, 15.8, 58.0)));
    let r2_pads = pads("R2");
    assert!(r2_pads
        .iter()
        .all(|(l, _, r)| *l == layer(&o, "B.Cu") && near(*r, 90.0)));
    assert!(r2_pads.iter().any(|(_, p, _)| at(*p, 35.0, 58.8)));
    assert!(r2_pads.iter().any(|(_, p, _)| at(*p, 35.0, 57.2)));
    // The Reference and the shown MPN field of each, where KiCad shows them.
    let text = |content: &str| {
        o.board
            .texts
            .iter()
            .find(|t| t.content == content)
            .unwrap_or_else(|| panic!("no text {content}"))
    };
    for (content, x, y, height, rot, mirrored, side) in [
        ("R1", 15.0, 60.0, 1.27, 0.0, false, "F.SilkS"),
        ("RC0603-10K", 15.0, 56.0, 1.0, 0.0, false, "F.SilkS"),
        ("R2", 33.0, 58.0, 1.27, 90.0, true, "B.SilkS"),
        ("RC0603-1K", 37.0, 58.0, 1.0, 90.0, true, "B.SilkS"),
    ] {
        let t = text(content);
        assert!(
            t.layer == layer(&o, side)
                && at(t.at, x, y)
                && near(t.height, height)
                && near(t.rotation_deg, rot)
                && t.mirrored == mirrored,
            "{content}: {t:?}"
        );
    }
}
