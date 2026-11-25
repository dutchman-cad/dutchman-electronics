//! The reader against our own CC0 test boards, made in KiCad 9.0.9. Every number checked here is
//! written in the board's README.txt (KiCad's Y points down and the board's corner is at page
//! position (100, 100); the model has Y up, so model y = -(100 + Y) and model x = 100 + X).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::indexing_slicing
)]

use board_model::{
    CopperItem, Detection, Graphic, InputFormat, LayerKind, ModelSource, PadShape, Point,
    ReadOutcome, Segment, Severity, Side,
};
use kicad_pcb::KiCad;

fn board(name: &str) -> ReadOutcome {
    let path = format!(
        "{}/tests/boards/{name}.kicad_pcb",
        env!("CARGO_MANIFEST_DIR")
    );
    let bytes = std::fs::read(&path).unwrap();
    KiCad.read(&bytes).unwrap()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.001
}

/// Equal angles in degrees, whole turns apart or not (KiCad rounds the middle point of an arc to
/// a nanometre, so the angles are right to about a ten-thousandth of a degree).
fn same_angle(a: f64, b: f64) -> bool {
    let d = (a - b).rem_euclid(360.0);
    d < 0.001 || d > 359.999
}

fn at(p: Point, x: f64, y: f64) -> bool {
    near(p.x, 100.0 + x) && near(p.y, -(100.0 + y))
}

fn bounds(c: &board_model::Contour) -> (f64, f64, f64, f64) {
    let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for s in &c.segments {
        if let Segment::Line { from, to } = s {
            for p in [from, to] {
                b = (b.0.min(p.x), b.1.min(p.y), b.2.max(p.x), b.3.max(p.y));
            }
        }
    }
    b
}

fn notes_contain(o: &ReadOutcome, word: &str) -> bool {
    o.notes.iter().any(|n| n.message.contains(word))
}

#[test]
fn empty_outline_is_a_50_by_30_rectangle_1_6_thick() {
    let o = board("empty-outline");
    let b = &o.board;
    assert!(near(b.thickness.unwrap(), 1.6));
    assert_eq!(b.outline.outer.segments.len(), 4);
    assert!(b.outline.cutouts.is_empty());
    let (x0, y0, x1, y1) = bounds(&b.outline.outer);
    assert!(near(x0, 100.0) && near(x1, 150.0) && near(y0, -130.0) && near(y1, -100.0));
    assert!(b.copper.is_empty() && b.components.is_empty() && b.texts.is_empty());
    assert!(o.notes.iter().all(|n| n.severity == Severity::Info));
}

#[test]
fn layers_come_in_stack_order_with_kind_and_side() {
    let b = board("empty-outline").board;
    let names: Vec<&str> = b.layers.iter().map(|l| l.name.as_str()).collect();
    let pos = |n: &str| names.iter().position(|x| *x == n).unwrap();
    assert!(pos("F.SilkS") < pos("F.Paste"));
    assert!(pos("F.Paste") < pos("F.Mask"));
    assert!(pos("F.Mask") < pos("F.Cu"));
    assert!(pos("F.Cu") < pos("B.Cu"));
    assert!(pos("B.Cu") < pos("B.Mask"));
    assert!(pos("B.Mask") < pos("B.Paste"));
    assert!(pos("B.Paste") < pos("B.SilkS"));
    let layer = |n: &str| &b.layers[pos(n)];
    assert_eq!(layer("F.Cu").kind, LayerKind::Copper);
    assert_eq!(layer("F.Cu").side, Some(Side::Top));
    assert_eq!(layer("B.Cu").side, Some(Side::Bottom));
    assert_eq!(layer("B.SilkS").kind, LayerKind::Silkscreen);
    assert_eq!(layer("B.SilkS").side, Some(Side::Bottom));
    assert_eq!(layer("F.Mask").kind, LayerKind::SolderMask);
    assert_eq!(layer("F.Paste").kind, LayerKind::Paste);
    assert_eq!(layer("Edge.Cuts").kind, LayerKind::Outline);
    assert_eq!(layer("Edge.Cuts").side, None);
    assert_eq!(layer("Dwgs.User").kind, LayerKind::Other);
}

#[test]
fn four_layer_board_lists_its_inner_copper_in_order() {
    let b = board("four-layer").board;
    let copper: Vec<(&str, Option<Side>)> = b
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Copper)
        .map(|l| (l.name.as_str(), l.side))
        .collect();
    assert_eq!(
        copper,
        vec![
            ("F.Cu", Some(Side::Top)),
            ("In1.Cu", None),
            ("In2.Cu", None),
            ("B.Cu", Some(Side::Bottom)),
        ]
    );
    assert!(near(b.thickness.unwrap(), 1.2));
}

#[test]
fn outline_shapes_join_lines_and_an_arc_and_find_every_cutout() {
    let b = board("outline-shapes").board;
    let outer = &b.outline.outer;
    assert_eq!(outer.segments.len(), 5);
    let arcs: Vec<&Segment> = outer
        .segments
        .iter()
        .filter(|s| matches!(s, Segment::Arc { .. }))
        .collect();
    assert_eq!(arcs.len(), 1);
    match arcs[0] {
        Segment::Arc {
            center,
            radius,
            start_deg,
            end_deg,
            clockwise,
        } => {
            assert!(at(*center, 46.0, 4.0));
            assert!(near(*radius, 4.0));
            // The corner is drawn from (46, 0) to (50, 4): in Y up that is 90 degrees down to 0.
            assert!(*clockwise);
            assert!(
                same_angle(*start_deg, 90.0) && same_angle(*end_deg, 0.0),
                "{start_deg} {end_deg}"
            );
        }
        _ => unreachable!(),
    }
    // The contour is closed: every piece starts where the one before ends.
    assert_eq!(b.outline.cutouts.len(), 3);
    let mut sizes: Vec<usize> = b.outline.cutouts.iter().map(|c| c.segments.len()).collect();
    sizes.sort_unstable();
    assert_eq!(sizes, vec![1, 3, 4]);
    let circle = b
        .outline
        .cutouts
        .iter()
        .find(|c| c.segments.len() == 1)
        .unwrap();
    match &circle.segments[0] {
        Segment::Arc {
            center,
            radius,
            start_deg,
            end_deg,
            ..
        } => {
            assert!(at(*center, 10.0, 15.0));
            assert!(near(*radius, 1.5));
            assert_eq!(start_deg, end_deg);
        }
        _ => panic!("a circle is one arc"),
    }
    let rect = b
        .outline
        .cutouts
        .iter()
        .find(|c| c.segments.len() == 4)
        .unwrap();
    let (x0, y0, x1, y1) = bounds(rect);
    assert!(near(x0, 120.0) && near(x1, 128.0) && near(y0, -114.0) && near(y1, -110.0));
    let tri = b
        .outline
        .cutouts
        .iter()
        .find(|c| c.segments.len() == 3)
        .unwrap();
    let (x0, y0, x1, y1) = bounds(tri);
    assert!(near(x0, 135.0) && near(x1, 142.0) && near(y0, -126.0) && near(y1, -120.0));
}

#[test]
fn tracks_arcs_and_vias_with_their_nets() {
    let o = board("one-track-arc-via");
    let c = &o.board.copper;
    assert_eq!(c.len(), 4);
    let name = |i: usize| o.board.layers[i].name.as_str();
    let mut seen = (false, false, false, false);
    for item in c {
        match item {
            CopperItem::Track {
                layer,
                from,
                to,
                width,
                net,
            } if name(*layer) == "F.Cu" => {
                assert!(at(*from, 10.0, 10.0) && at(*to, 20.0, 10.0));
                assert!(near(*width, 0.5));
                assert_eq!(net.as_deref(), Some("SIG"));
                seen.0 = true;
            }
            CopperItem::Track {
                layer,
                from,
                to,
                width,
                net,
            } if name(*layer) == "B.Cu" => {
                assert!(at(*from, 10.0, 20.0) && at(*to, 30.0, 20.0));
                assert!(near(*width, 0.4));
                assert_eq!(net.as_deref(), Some("GND"));
                seen.1 = true;
            }
            CopperItem::TrackArc {
                layer,
                center,
                radius,
                start_deg,
                end_deg,
                width,
                net,
            } => {
                assert_eq!(name(*layer), "F.Cu");
                assert!(at(*center, 30.0, 15.0) && near(*radius, 5.0));
                // From (30, 10) to (35, 15) in KiCad is, in Y up, from 90 degrees round to 0 the
                // clockwise way; as counter-clockwise it is 0 to 90.
                assert!(
                    same_angle(*start_deg, 0.0) && same_angle(*end_deg, 90.0),
                    "{start_deg} {end_deg}"
                );
                assert!(near(*width, 0.3));
                assert_eq!(net.as_deref(), Some("SIG"));
                seen.2 = true;
            }
            CopperItem::Via {
                at: p,
                diameter,
                drill,
            } => {
                assert!(at(*p, 40.0, 20.0));
                assert!(near(*diameter, 0.8) && near(*drill, 0.4));
                seen.3 = true;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(seen, (true, true, true, true));
}

fn pads(o: &ReadOutcome) -> Vec<&CopperItem> {
    o.board
        .copper
        .iter()
        .filter(|c| matches!(c, CopperItem::Pad { .. }))
        .collect()
}

#[test]
fn a_top_resistor_with_a_model_reference() {
    let o = board("one-resistor");
    assert_eq!(o.board.components.len(), 1);
    let p = &o.board.components[0];
    assert_eq!(p.designator, "R1");
    assert_eq!(p.footprint, "R_0603");
    assert!(at(p.at, 25.0, 15.0));
    assert!(near(p.rotation_deg, 0.0));
    assert_eq!(p.side, Side::Top);
    let m = p.model.as_ref().unwrap();
    assert_eq!(m.name, "box-1608.step");
    assert!(matches!(&m.source, ModelSource::External { file } if file == "box-1608.step"));
    assert!(notes_contain(&o, "3D model file"));
    // The offset as written; the rotation turned the other way (KiCad's z of 30 is -30 here).
    assert!(m
        .offset
        .iter()
        .zip([0.5, 0.25, 0.1])
        .all(|(a, b)| near(*a, b)));
    assert!(m
        .rotation_deg
        .iter()
        .zip([0.0, 0.0, -30.0])
        .all(|(a, b)| near(*a, b)));
    let pads = pads(&o);
    assert_eq!(pads.len(), 2);
    let top = o
        .board
        .layers
        .iter()
        .position(|l| l.name == "F.Cu")
        .unwrap();
    let mut xs = Vec::new();
    for p in pads {
        let CopperItem::Pad {
            layer,
            at: pos,
            shape,
            size_x,
            size_y,
            rotation_deg,
            drill,
            owner,
            ..
        } = p
        else {
            unreachable!()
        };
        assert_eq!(*layer, top);
        assert_eq!(*shape, PadShape::Rectangle);
        assert!(near(*size_x, 0.9) && near(*size_y, 0.95) && near(*rotation_deg, 0.0));
        assert!(drill.is_none());
        assert_eq!(owner.as_deref(), Some("R1"));
        assert!(near(pos.y, -115.0));
        xs.push(pos.x);
    }
    xs.sort_by(f64::total_cmp);
    assert!(near(xs[0], 124.2) && near(xs[1], 125.8));
    // The reference text is on the top silkscreen.
    assert_eq!(o.board.texts.len(), 1);
    assert_eq!(o.board.texts[0].content, "R1");
    assert_eq!(o.board.layers[o.board.texts[0].layer].name, "F.SilkS");
}

#[test]
fn a_bottom_resistor_turned_90_degrees() {
    let o = board("one-bottom-resistor");
    let p = &o.board.components[0];
    assert_eq!(p.side, Side::Bottom);
    assert!(near(p.rotation_deg, 90.0));
    assert!(at(p.at, 25.0, 15.0));
    let bottom = o
        .board
        .layers
        .iter()
        .position(|l| l.name == "B.Cu")
        .unwrap();
    let mut ys = Vec::new();
    for item in pads(&o) {
        let CopperItem::Pad {
            layer,
            at: pos,
            rotation_deg,
            ..
        } = item
        else {
            unreachable!()
        };
        assert_eq!(*layer, bottom);
        assert!(near(*rotation_deg, 90.0));
        assert!(near(pos.x, 125.0));
        ys.push(pos.y);
    }
    ys.sort_by(f64::total_cmp);
    // README: pads at (25, 15.8) and (25, 14.2) in KiCad's Y down.
    assert!(near(ys[0], -115.8) && near(ys[1], -114.2));
    // The bottom silkscreen text reads mirrored.
    assert!(o.board.texts.iter().all(|t| t.mirrored));
}

#[test]
fn through_hole_pads_have_their_holes_and_plating() {
    let o = board("one-thru-hole-pads");
    let all = o
        .board
        .layers
        .iter()
        .position(|l| l.name == "*.Cu")
        .unwrap();
    assert_eq!(o.board.layers[all].kind, LayerKind::Copper);
    assert_eq!(o.board.layers[all].side, None);
    let mut found = Vec::new();
    for item in pads(&o) {
        let CopperItem::Pad {
            layer,
            at: pos,
            shape,
            size_x,
            size_y,
            drill,
            plated,
            ..
        } = item
        else {
            unreachable!()
        };
        assert_eq!(*layer, all);
        found.push((
            pos.x.round() as i64,
            *shape,
            *size_x,
            *size_y,
            *drill,
            *plated,
        ));
    }
    found.sort_by_key(|f| f.0);
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].0, 120);
    assert_eq!(found[0].1, PadShape::Round);
    assert!(near(found[0].2, 1.8) && near(found[0].4.unwrap(), 1.0) && found[0].5);
    assert_eq!(found[1].1, PadShape::Oval);
    assert!(near(found[1].2, 2.0) && near(found[1].3, 1.4) && near(found[1].4.unwrap(), 0.8));
    assert!(found[1].5);
    assert!(near(found[2].2, 2.0) && near(found[2].4.unwrap(), 2.0) && !found[2].5);
}

#[test]
fn silkscreen_lines_arcs_circles_polygons_and_text() {
    let o = board("silkscreen");
    let b = &o.board;
    let silk = b.layers.iter().position(|l| l.name == "F.SilkS").unwrap();
    let mut lines = Vec::new();
    let mut arcs = Vec::new();
    for g in &b.graphics {
        match g {
            Graphic::Line {
                layer,
                from,
                to,
                width,
                ..
            } => {
                assert_eq!(*layer, silk);
                lines.push((*from, *to, *width));
            }
            Graphic::Arc {
                layer,
                center,
                radius,
                start_deg,
                end_deg,
                width,
                ..
            } => {
                assert_eq!(*layer, silk);
                arcs.push((*center, *radius, *start_deg, *end_deg, *width));
            }
        }
    }
    // One line plus the polygon's three sides.
    assert_eq!(lines.len(), 4);
    assert!(lines
        .iter()
        .any(|(a, b, w)| at(*a, 5.0, 5.0) && at(*b, 15.0, 5.0) && near(*w, 0.15)));
    // The arc and the circle.
    assert_eq!(arcs.len(), 2);
    let quarter = arcs.iter().find(|a| a.2 != a.3).unwrap();
    assert!(at(quarter.0, 20.0, 8.0) && (quarter.1 - 3.0).abs() < 0.001 && near(quarter.4, 0.2));
    // (20,5) to (23,8) in Y down is 90 degrees to 0 in Y up, clockwise: 0 to 90 counter-clockwise.
    assert!(
        same_angle(quarter.2, 0.0) && same_angle(quarter.3, 90.0),
        "{quarter:?}"
    );
    let circle = arcs.iter().find(|a| a.2 == a.3).unwrap();
    assert!(at(circle.0, 35.0, 8.0) && near(circle.1, 2.0) && near(circle.4, 0.15));
    // The text: "HELLO" centred on (30, 22). The model anchors a text at its baseline's left
    // end, so the reader moves it by half an estimated width (5 letters, 0.6 of the height
    // each) and half the height. The turn is 0.
    assert_eq!(b.texts.len(), 1);
    let t = &b.texts[0];
    assert_eq!(t.content, "HELLO");
    assert!(near(t.height, 1.5) && near(t.rotation_deg, 0.0) && !t.mirrored);
    assert!(matches!(t.font, board_model::TextFont::Stroke { width } if near(width, 0.2)));
    assert!(
        near(t.at.x, 130.0 - 2.25) && near(t.at.y, -122.0 - 0.75),
        "{:?}",
        t.at
    );
    assert!(notes_contain(&o, "estimate"));
}

#[test]
fn a_filled_zone_comes_as_copper_with_its_gap_around_the_via() {
    let o = board("one-zone");
    let top = o
        .board
        .layers
        .iter()
        .position(|l| l.name == "F.Cu")
        .unwrap();
    let regions: Vec<_> = o
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Region { layer, contour, .. } => Some((*layer, contour)),
            _ => None,
        })
        .collect();
    assert_eq!(regions.len(), 1);
    let (layer, contour) = regions[0];
    assert_eq!(layer, top);
    // Within the zone's outline (5, 5) to (45, 25), and with points on the gap round the via at
    // (25, 15): 0.9 away from its centre.
    let mut near_gap = 0;
    for s in &contour.segments {
        let Segment::Line { from, .. } = s else {
            panic!("fills are polygons")
        };
        assert!(from.x >= 105.0 - 0.001 && from.x <= 145.0 + 0.001);
        assert!(from.y <= -105.0 + 0.001 && from.y >= -125.0 - 0.001);
        let d = (from.x - 125.0).hypot(from.y + 115.0);
        if (d - 0.9).abs() < 0.01 {
            near_gap += 1;
        }
    }
    assert!(near_gap > 8, "{near_gap}");
    assert!(contour.segments.len() > 30);
    // The via of the other net is there too.
    assert!(o
        .board
        .copper
        .iter()
        .any(|c| matches!(c, CopperItem::Via { at: p, .. } if at(*p, 25.0, 15.0))));
}

#[test]
fn every_board_is_detected_by_content_and_by_name() {
    let dir = format!("{}/tests/boards", env!("CARGO_MANIFEST_DIR"));
    let mut n = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("kicad_pcb") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            KiCad.detect(&bytes[..bytes.len().min(512)], None),
            Detection::Yes
        );
        assert_eq!(KiCad.detect(b"", Some("KiCad_PCB")), Detection::MaybeByName);
        n += 1;
    }
    assert!(n >= 9);
}
