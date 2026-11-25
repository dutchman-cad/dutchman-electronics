use board_model::{Board, CopperItem, Graphic, Layer, LayerKind, PadShape, Point, Side};
use serde_json::Value;

use super::*;

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn layers() -> Vec<Layer> {
    let l = |name: &str, kind, side| Layer {
        name: name.into(),
        kind,
        side,
    };
    vec![
        l("Top Layer", LayerKind::Copper, Some(Side::Top)),
        l("Bottom Layer", LayerKind::Copper, Some(Side::Bottom)),
        l("Multi-Layer", LayerKind::Copper, None),
        l("Top Overlay", LayerKind::Silkscreen, Some(Side::Top)),
        l("Mid Layer 1", LayerKind::Copper, None),
        l("Mechanical 1", LayerKind::Other, None),
    ]
}

fn board(copper: Vec<CopperItem>, graphics: Vec<Graphic>) -> Board {
    Board {
        layers: layers(),
        copper,
        graphics,
        ..Board::default()
    }
}

/// The area enclosed by one package ring (shoelace), positive when counter-clockwise.
fn ring_area(ring: &Value) -> f64 {
    let pts: Vec<(f64, f64)> = ring
        .as_array()
        .unwrap()
        .iter()
        .map(|q| (q[0].as_f64().unwrap(), q[1].as_f64().unwrap()))
        .collect();
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f64>()
        / 2.0
}

/// The net area of an overlay: outer rings minus holes.
fn area(o: &Value) -> f64 {
    o["areas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            let outer = ring_area(&a["outer"]);
            assert!(outer > 0.0, "outer rings run counter-clockwise");
            let holes: f64 = a["holes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|h| {
                    let h = ring_area(h);
                    assert!(h < 0.0, "holes run clockwise");
                    h
                })
                .sum();
            outer + holes
        })
        .sum()
}

fn named<'a>(list: &'a [Value], name: &str) -> &'a Value {
    list.iter()
        .find(|o| o["name"] == name)
        .unwrap_or_else(|| panic!("no overlay {name} in {list:?}"))
}

fn track(layer: usize, from: Point, to: Point, width: f64) -> CopperItem {
    CopperItem::Track {
        layer,
        from,
        to,
        width,
        net: None,
    }
}

fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> board_model::Contour {
    let c = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    board_model::Contour {
        segments: c
            .iter()
            .zip(c.iter().cycle().skip(1))
            .map(|(a, b)| board_model::Segment::Line {
                from: p(a.0, a.1),
                to: p(b.0, b.1),
            })
            .collect(),
    }
}

#[test]
fn the_mask_is_the_outline_less_cutouts_pads_copper_and_drills() {
    // A 20 x 10 board (200 mm2) with a 2 x 2 cutout (4 mm2) and, on the top, a bare 1 x 1 SMD
    // pad (1 mm2) and a via of 1 mm across with a 0.5 mm drill. The mask stays off the via's
    // copper ring and its drill, so a 1 mm circle (drawn as a 32-sided polygon of 0.78036 mm2)
    // is open on both faces. Top mask: 200 - 4 - 1 - 0.78036 = 194.21964. Bottom: no pad, so
    // 200 - 4 - 0.78036 = 195.21964.
    let mut b = board(
        vec![
            CopperItem::Pad {
                layer: 0,
                at: p(5.0, 5.0),
                shape: PadShape::Rectangle,
                size_x: 1.0,
                size_y: 1.0,
                rotation_deg: 0.0,
                drill: None,
                plated: false,
                owner: None,
            },
            CopperItem::Via {
                at: p(15.0, 5.0),
                diameter: 1.0,
                drill: 0.5,
            },
        ],
        vec![],
    );
    b.outline.outer = square(0.0, 0.0, 20.0, 10.0);
    b.outline.cutouts = vec![square(8.0, 4.0, 10.0, 6.0)];
    let (list, notes) = overlays(&b, 1.6, "Board");
    assert!(notes.is_empty(), "{notes:?}");
    let top = named(&list, "Solder mask top");
    let bottom = named(&list, "Solder mask bottom");
    assert_eq!(top["kind"], "mask");
    // Mask z = T + 0: 1.6.
    assert_eq!(top["z"], 1.6);
    assert_eq!(top["opacity"], 0.85);
    assert_eq!(top["color"], "#1f5f3a");
    // Bottom mask z = 0 - 0: 0.
    assert_eq!(bottom["z"], 0.0);
    assert!((area(top) - 194.21964).abs() < 1e-3, "{}", area(top));
    assert!((area(bottom) - 195.21964).abs() < 1e-3, "{}", area(bottom));
    // Three holes in the top mask: the cutout, the pad and the drill.
    assert_eq!(top["areas"][0]["holes"].as_array().unwrap().len(), 3);
}

#[test]
fn pads_on_the_bottom_only_open_the_bottom_mask() {
    // A 20 x 10 board (200 mm2) and one bare 1 x 1 SMD pad (1 mm2) on Bottom Layer: the top
    // mask is whole (200), the bottom mask has the pad's opening (199).
    let mut b = board(
        vec![CopperItem::Pad {
            layer: 1,
            at: p(5.0, 5.0),
            shape: PadShape::Rectangle,
            size_x: 1.0,
            size_y: 1.0,
            rotation_deg: 0.0,
            drill: None,
            plated: false,
            owner: None,
        }],
        vec![],
    );
    b.outline.outer = square(0.0, 0.0, 20.0, 10.0);
    let (list, notes) = overlays(&b, 1.6, "Board");
    assert!(notes.is_empty(), "{notes:?}");
    let top = named(&list, "Solder mask top");
    let bottom = named(&list, "Solder mask bottom");
    assert!((area(top) - 200.0).abs() < 1e-6, "{}", area(top));
    assert!(top["areas"][0]["holes"].as_array().unwrap().is_empty());
    assert!((area(bottom) - 199.0).abs() < 1e-6, "{}", area(bottom));
    assert_eq!(bottom["areas"][0]["holes"].as_array().unwrap().len(), 1);
}

#[test]
fn cutouts_past_the_cap_are_counted_not_united() {
    let mut b = board(vec![], vec![]);
    b.outline.outer = square(0.0, 0.0, 10.0, 10.0);
    b.outline.cutouts = vec![square(1.0, 1.0, 2.0, 2.0); MAX_ITEMS + 3];
    let (outline, cuts, bad, over) = mask_outline(&b);
    assert!(outline.is_some());
    assert_eq!((cuts.polygons.len(), bad, over), (MAX_ITEMS, 0, 3));
}

#[test]
fn an_outline_that_cannot_be_drawn_is_a_dropped_note_and_no_mask() {
    let mut b = board(vec![], vec![]);
    // A line of no area.
    b.outline.outer = square(0.0, 0.0, 10.0, 0.0);
    let (list, notes) = overlays(&b, 1.6, "Board");
    assert!(list.iter().all(|o| o["kind"] != "mask"));
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["severity"], "Dropped");
    assert!(notes[0]["message"]
        .as_str()
        .unwrap()
        .contains("solder mask"));
    // A cutout that cannot be drawn is counted, and the mask still comes.
    let mut b = board(vec![], vec![]);
    b.outline.outer = square(0.0, 0.0, 10.0, 10.0);
    b.outline.cutouts = vec![square(1.0, 1.0, 2.0, 1.0)];
    let (list, notes) = overlays(&b, 1.6, "Board");
    assert!((area(named(&list, "Solder mask top")) - 100.0).abs() < 1e-6);
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["severity"], "Dropped");
}

#[test]
fn a_track_is_a_stroke_with_round_ends_on_its_face() {
    // one-track's numbers: (10, 15) to (20, 15), 0.5 wide, on Top Layer.
    let (list, notes) = overlays(
        &board(vec![track(0, p(10.0, 15.0), p(20.0, 15.0), 0.5)], vec![]),
        1.6,
        "Board",
    );
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(list.len(), 1);
    let o = &list[0];
    assert_eq!(o["name"], "Copper top");
    assert_eq!(o["part"], "Board");
    assert_eq!(o["kind"], "copper");
    assert_eq!(o["finish"], "masked");
    assert_eq!(o["side"], "top");
    // Masked copper z = T + 0 = 1.6; its thickness is 0.035.
    assert_eq!(o["z"], 1.6);
    assert_eq!(o["thickness"], 0.035);
    // 10 x 0.5 plus two half circles of radius 0.25; the polyline loses at most 0.02 mm a side.
    let exact = 5.0 + PI * 0.25 * 0.25;
    assert!(area(o) < exact && area(o) > exact - 0.01, "{}", area(o));
}

#[test]
fn a_bottom_track_is_drawn_below_the_board() {
    let (list, _) = overlays(
        &board(vec![track(1, p(10.0, 15.0), p(20.0, 15.0), 0.5)], vec![]),
        1.6,
        "Board",
    );
    let o = named(&list, "Copper bottom");
    // Bottom masked copper z = 0 - 0 = 0, extruded down.
    assert_eq!(o["z"], 0.0);
    assert_eq!(o["side"], "bottom");
}

#[test]
fn a_via_is_a_tented_disc_on_both_faces() {
    // one-via's numbers: diameter 1, hole 0.5. Vias are drawn tented: the copper is the whole
    // disc, with no hole, and the mask stays off it.
    let via = CopperItem::Via {
        at: p(25.0, 15.0),
        diameter: 1.0,
        drill: 0.5,
    };
    let mut b = board(vec![via], vec![]);
    b.outline.outer = square(0.0, 0.0, 50.0, 30.0);
    let (list, _) = overlays(&b, 1.6, "Board");
    let disc = PI * 0.25;
    for side in ["top", "bottom"] {
        let o = named(&list, &format!("Copper {side}"));
        assert_eq!(o["areas"].as_array().unwrap().len(), 1);
        assert_eq!(o["areas"][0]["holes"].as_array().unwrap().len(), 0);
        assert!(covers(o, 25.0, 15.0), "{side}: no copper over the drill");
        assert!((area(o) - disc).abs() < 0.01, "{side}: {}", area(o));
        let mask = named(&list, &format!("Solder mask {side}"));
        assert!(!covers(mask, 25.0, 15.0), "{side}: mask over the via");
    }
}

#[test]
fn a_plated_pad_is_bare_copper_on_both_faces_with_its_hole() {
    // one-plated-pad's numbers: round 2 x 2, hole 1, on Multi-Layer.
    let pad = CopperItem::Pad {
        layer: 2,
        at: p(25.0, 15.0),
        shape: PadShape::Round,
        size_x: 2.0,
        size_y: 2.0,
        rotation_deg: 0.0,
        drill: Some(1.0),
        plated: true,
        owner: None,
    };
    let (list, _) = overlays(&board(vec![pad], vec![]), 1.6, "Board");
    let ring = PI * (1.0 - 0.25);
    for name in ["Copper top pads", "Copper bottom pads"] {
        let o = named(&list, name);
        assert_eq!(o["finish"], "bare");
        // A pad's drill stays open, unlike a via's.
        assert!(!covers(o, 25.0, 15.0), "{name}: copper over the hole");
        assert!((area(o) - ring).abs() < 0.02, "{name}: {}", area(o));
    }
}

#[test]
fn masked_copper_stops_where_a_pad_begins() {
    // A track running into the middle of a 2 x 2 square pad: its last 1 mm is under the pad.
    let pad = CopperItem::Pad {
        layer: 0,
        at: p(10.0, 0.0),
        shape: PadShape::Rectangle,
        size_x: 2.0,
        size_y: 2.0,
        rotation_deg: 0.0,
        drill: None,
        plated: false,
        owner: Some("R1".into()),
    };
    let (list, _) = overlays(
        &board(vec![track(0, p(0.0, 0.0), p(10.0, 0.0), 0.5), pad], vec![]),
        1.6,
        "Board",
    );
    let pads = named(&list, "Copper top pads");
    assert!((area(pads) - 4.0).abs() < 1e-6, "{}", area(pads));
    // The track from x = 0 (with its round end) to the pad's edge at x = 9.
    let masked = area(named(&list, "Copper top"));
    let exact = 9.0 * 0.5 + PI * 0.25 * 0.25 / 2.0;
    assert!(masked < exact && masked > exact - 0.01, "{masked}");
}

#[test]
fn a_rotated_rectangle_pad_keeps_its_area() {
    let pad = CopperItem::Pad {
        layer: 0,
        at: p(25.0, 15.0),
        shape: PadShape::Rectangle,
        size_x: 2.0,
        size_y: 1.0,
        rotation_deg: 30.0,
        drill: None,
        plated: false,
        owner: None,
    };
    let (list, _) = overlays(&board(vec![pad], vec![]), 1.6, "Board");
    let o = named(&list, "Copper top pads");
    assert!((area(o) - 2.0).abs() < 1e-6);
    // one-rotated-pad: a corner at (25, 15) + rot30(1, 0.5).
    let (s, c) = 30f64.to_radians().sin_cos();
    let corner = [25.0 + c - 0.5 * s, 15.0 + s + 0.5 * c];
    let found = o["areas"][0]["outer"].as_array().unwrap().iter().any(|q| {
        (q[0].as_f64().unwrap() - corner[0]).abs() < 1e-6
            && (q[1].as_f64().unwrap() - corner[1]).abs() < 1e-6
    });
    assert!(found);
}

#[test]
fn touching_items_become_one_area() {
    let (list, _) = overlays(
        &board(
            vec![
                track(0, p(0.0, 0.0), p(10.0, 0.0), 0.5),
                track(0, p(10.0, 0.0), p(10.0, 10.0), 0.5),
            ],
            vec![],
        ),
        1.6,
        "Board",
    );
    assert_eq!(list[0]["areas"].as_array().unwrap().len(), 1);
}

#[test]
fn silkscreen_lines_and_arcs_are_drawn_and_other_layers_are_noted() {
    let graphics = vec![
        // one-overlay-line's numbers.
        Graphic::Line {
            layer: 3,
            from: p(10.0, 10.0),
            to: p(20.0, 10.0),
            width: 0.2,
            owner: None,
        },
        Graphic::Arc {
            layer: 3,
            center: p(30.0, 15.0),
            radius: 2.0,
            start_deg: 0.0,
            end_deg: 360.0,
            width: 0.2,
            owner: None,
        },
        Graphic::Line {
            layer: 5,
            from: p(0.0, 0.0),
            to: p(1.0, 0.0),
            width: 0.1,
            owner: None,
        },
    ];
    let (list, notes) = overlays(&board(vec![], graphics), 1.6, "Board");
    let o = named(&list, "Silkscreen top");
    assert_eq!(o["kind"], "silkscreen");
    assert!(o.get("finish").is_none());
    // Silkscreen z = T + 0.035 = 1.635; its thickness is 0.015.
    assert_eq!(o["z"], 1.635);
    assert_eq!(o["thickness"], 0.015);
    // The line, and a ring of width 0.2 around radius 2 (with a hole).
    let exact = 10.0 * 0.2 + PI * 0.01 + 2.0 * PI * 2.0 * 0.2;
    assert!((area(o) - exact).abs() < 0.05, "{}", area(o));
    assert_eq!(o["areas"].as_array().unwrap().len(), 2);
    assert_eq!(notes.len(), 1);
    assert!(notes[0]["message"]
        .as_str()
        .unwrap()
        .starts_with("1 line(s) and arc(s) on layers other than the silkscreen"));
}

#[test]
fn inner_layer_copper_and_bad_sizes_are_noted() {
    let (list, notes) = overlays(
        &board(
            vec![
                track(4, p(0.0, 0.0), p(1.0, 0.0), 0.2),
                track(0, p(0.0, 0.0), p(1.0, 0.0), f64::NAN),
                track(0, p(0.0, 0.0), p(1.0, 0.0), -1.0),
            ],
            vec![],
        ),
        1.6,
        "Board",
    );
    assert!(list.is_empty());
    let messages: Vec<&str> = notes
        .iter()
        .map(|n| n["message"].as_str().unwrap())
        .collect();
    assert_eq!(
        messages,
        [
            "1 copper item(s) on inner layers are left out, as they lie inside the board",
            "2 copper or silkscreen item(s) have unusable sizes or layers and were left out",
        ]
    );
}

#[test]
fn a_region_with_an_arc_is_drawn_within_the_chord() {
    use board_model::{Contour, Segment};
    // A half disc of radius 5: the arc from 0 to 180 degrees, closed by its diameter.
    let c = Contour {
        segments: vec![
            Segment::Arc {
                center: p(0.0, 0.0),
                radius: 5.0,
                start_deg: 0.0,
                end_deg: 180.0,
                clockwise: false,
            },
            Segment::Line {
                from: p(-5.0, 0.0),
                to: p(5.0, 0.0),
            },
        ],
    };
    let (list, _) = overlays(
        &board(
            vec![CopperItem::Region {
                layer: 0,
                contour: c,
                holes: vec![],
            }],
            vec![],
        ),
        1.6,
        "Board",
    );
    let a = area(&list[0]);
    let exact = PI * 25.0 / 2.0;
    assert!(a < exact && a > exact - 0.02 * 5.0 * PI, "{a}");
}

fn square_cw(x0: f64, y0: f64, side: f64) -> board_model::Contour {
    use board_model::{Contour, Segment};
    let c = [
        (x0, y0),
        (x0, y0 + side),
        (x0 + side, y0 + side),
        (x0 + side, y0),
    ];
    Contour {
        segments: (0..4)
            .map(|i| Segment::Line {
                from: p(c[i].0, c[i].1),
                to: p(c[(i + 1) % 4].0, c[(i + 1) % 4].1),
            })
            .collect(),
    }
}

#[test]
fn a_clockwise_pour_first_does_not_wipe_out_the_rest() {
    // The union picks its fill rule from the first ring; every ring must run the same way.
    let (list, _) = overlays(
        &board(
            vec![
                CopperItem::Region {
                    layer: 0,
                    contour: square_cw(0.0, 0.0, 2.0),
                    holes: vec![],
                },
                track(0, p(10.0, 0.0), p(20.0, 0.0), 0.5),
            ],
            vec![],
        ),
        1.6,
        "Board",
    );
    let a = area(named(&list, "Copper top"));
    let exact = 4.0 + 5.0 + PI * 0.0625;
    assert!((a - exact).abs() < 0.01, "{a}");
}

#[test]
fn a_clockwise_pour_after_a_track_is_kept() {
    let (list, _) = overlays(
        &board(
            vec![
                track(0, p(10.0, 0.0), p(20.0, 0.0), 0.5),
                CopperItem::Region {
                    layer: 0,
                    contour: square_cw(0.0, 0.0, 2.0),
                    holes: vec![],
                },
            ],
            vec![],
        ),
        1.6,
        "Board",
    );
    let a = area(named(&list, "Copper top"));
    assert!((a - (4.0 + 5.0 + PI * 0.0625)).abs() < 0.01, "{a}");
}

/// True when the point lies inside one of the overlay's areas (outer minus holes).
fn covers(o: &Value, x: f64, y: f64) -> bool {
    let inside = |ring: &Value| {
        let pts: Vec<(f64, f64)> = ring
            .as_array()
            .unwrap()
            .iter()
            .map(|q| (q[0].as_f64().unwrap(), q[1].as_f64().unwrap()))
            .collect();
        let mut odd = false;
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            if (a.1 > y) != (b.1 > y) && x < a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1) {
                odd = !odd;
            }
        }
        odd
    };
    o["areas"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| inside(&a["outer"]) && !a["holes"].as_array().unwrap().iter().any(inside))
}

#[test]
fn an_arc_runs_counter_clockwise_from_its_start() {
    // A quarter arc of radius 5 from 0 to 90 degrees covers (3.54, 3.54), not (-3.54, -3.54).
    let arc = CopperItem::TrackArc {
        layer: 0,
        center: p(0.0, 0.0),
        radius: 5.0,
        start_deg: 0.0,
        end_deg: 90.0,
        width: 0.5,
        net: None,
    };
    let (list, _) = overlays(&board(vec![arc], vec![]), 1.6, "Board");
    let o = named(&list, "Copper top");
    let r = 5.0 / 2f64.sqrt();
    assert!(covers(o, r, r));
    assert!(!covers(o, -r, -r));
    assert!(!covers(o, 0.0, 0.0));
    // Round ends: the band plus two half circles.
    let exact = PI / 2.0 * 5.0 * 0.5 + PI * 0.0625;
    assert!((area(o) - exact).abs() < 0.05, "{}", area(o));
}

#[test]
fn a_whole_circle_arc_is_a_ring_with_its_hole_open() {
    let arc = CopperItem::TrackArc {
        layer: 0,
        center: p(0.0, 0.0),
        radius: 5.0,
        start_deg: 30.0,
        end_deg: 30.0,
        width: 0.5,
        net: None,
    };
    let (list, _) = overlays(&board(vec![arc], vec![]), 1.6, "Board");
    let o = named(&list, "Copper top");
    assert!(covers(o, 5.0, 0.0));
    assert!(!covers(o, 0.0, 0.0));
    // Both edges lie within the 0.02 mm chord of their circles, so the band can lose about
    // 0.02 x its 33 mm length.
    assert!((area(o) - 2.0 * PI * 5.0 * 0.5).abs() < 0.1, "{}", area(o));
}

#[test]
fn a_tall_slot_turned_a_quarter_lies_along_x() {
    // An oval 1 wide and 3 tall, turned 90 degrees, reaches x = +-1.5 and y = +-0.5.
    let pad = CopperItem::Pad {
        layer: 0,
        at: p(0.0, 0.0),
        shape: PadShape::Oval,
        size_x: 1.0,
        size_y: 3.0,
        rotation_deg: 90.0,
        drill: None,
        plated: false,
        owner: None,
    };
    let (list, _) = overlays(&board(vec![pad], vec![]), 1.6, "Board");
    let o = named(&list, "Copper top pads");
    assert!(covers(o, 1.4, 0.0) && covers(o, -1.4, 0.0));
    assert!(!covers(o, 0.0, 1.0));
    assert!((area(o) - (2.0 + PI * 0.25)).abs() < 0.01);
}

#[test]
fn silkscreen_is_not_printed_over_holes_or_pads() {
    let line = Graphic::Line {
        layer: 3,
        from: p(0.0, 0.0),
        to: p(10.0, 0.0),
        width: 0.2,
        owner: None,
    };
    let via = CopperItem::Via {
        at: p(5.0, 0.0),
        diameter: 1.0,
        drill: 0.5,
    };
    let (list, _) = overlays(&board(vec![via], vec![line]), 1.6, "Board");
    let o = named(&list, "Silkscreen top");
    assert!(!covers(o, 5.0, 0.0), "the drill is open");
    assert!(covers(o, 1.0, 0.0));
}

#[test]
fn items_far_off_the_board_or_on_other_layers_are_left_out() {
    let (list, notes) = overlays(
        &board(
            vec![
                track(0, p(1e9, 0.0), p(1e9 + 1.0, 0.0), 0.5),
                track(5, p(0.0, 0.0), p(1.0, 0.0), 0.5),
                track(99, p(0.0, 0.0), p(1.0, 0.0), 0.5),
            ],
            vec![],
        ),
        1.6,
        "Board",
    );
    assert!(list.is_empty());
    assert_eq!(
        notes[0]["message"],
        "3 copper or silkscreen item(s) have unusable sizes or layers and were left out"
    );
}

#[test]
fn a_clockwise_arc_in_a_region_is_a_concave_corner() {
    use board_model::{Contour, Segment};
    // A 40 x 20 pour whose top right corner is a concave quarter circle of radius 5 around
    // (40, 20), running clockwise from (40, 15) to (35, 20): 800 - 25π/4.
    let line = |a: (f64, f64), b: (f64, f64)| Segment::Line {
        from: p(a.0, a.1),
        to: p(b.0, b.1),
    };
    let c = Contour {
        segments: vec![
            line((0.0, 0.0), (40.0, 0.0)),
            line((40.0, 0.0), (40.0, 15.0)),
            Segment::Arc {
                center: p(40.0, 20.0),
                radius: 5.0,
                start_deg: 270.0,
                end_deg: 180.0,
                clockwise: true,
            },
            line((35.0, 20.0), (0.0, 20.0)),
            line((0.0, 20.0), (0.0, 0.0)),
        ],
    };
    let (list, _) = overlays(
        &board(
            vec![CopperItem::Region {
                layer: 0,
                contour: c,
                holes: vec![],
            }],
            vec![],
        ),
        1.6,
        "Board",
    );
    let a = area(&list[0]);
    // The polyline's points sit on the circle, so it cuts the concave corner short by at most
    // the chord limit along the arc's 2.5π length: a little more area, never less.
    let exact = 800.0 - 25.0 * PI / 4.0;
    assert!(a > exact && a < exact + 0.02 * 2.5 * PI, "{a} vs {exact}");
    assert!(!covers(&list[0], 39.0, 19.0));
    assert!(covers(&list[0], 34.0, 19.0));
}

fn text_on(layer: usize, content: &str, height: f64, at: Point) -> board_model::Text {
    board_model::Text {
        layer,
        at,
        content: content.into(),
        height,
        rotation_deg: 0.0,
        mirrored: false,
        font: board_model::TextFont::Stroke { width: 0.2 },
    }
}

fn messages(notes: &[Value]) -> Vec<String> {
    notes
        .iter()
        .map(|n| n["message"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_text_on_a_top_copper_layer_lands_in_copper_top() {
    let mut b = board(vec![], vec![]);
    b.texts.push(text_on(0, "H", 2.0, p(10.0, 10.0)));
    let (list, notes) = overlays(&b, 1.6, "Board");
    let o = named(&list, "Copper top");
    assert_eq!(o["finish"], "masked");
    assert!(area(o) > 0.0);
    assert!(list.iter().all(|o| o["name"] != "Silkscreen top"));
    assert!(
        messages(&notes)
            .iter()
            .all(|m| !m.contains("unusable") && !m.contains("limit")),
        "{notes:?}"
    );
}

#[test]
fn a_huge_text_or_one_far_away_adds_nothing_and_is_one_unusable_item_each() {
    let mut b = board(vec![], vec![]);
    b.texts.push(text_on(3, "HHHH", 1.0e6, p(10.0, 10.0)));
    b.texts.push(text_on(3, "HHHH", 2.0, p(1.0e9, 10.0)));
    let (list, notes) = overlays(&b, 1.6, "Board");
    assert!(list.is_empty(), "{list:?}");
    let unusable: Vec<String> = messages(&notes)
        .into_iter()
        .filter(|m| m.contains("unusable"))
        .collect();
    assert_eq!(
        unusable,
        ["2 copper or silkscreen item(s) have unusable sizes or layers and were left out"]
    );
    assert!(
        messages(&notes).iter().all(|m| !m.contains("Hershey")),
        "{notes:?}"
    );
}

#[test]
fn texts_past_the_segment_budget_are_counted_not_drawn() {
    // "HHHH" is four capitals of three one-segment strokes: 12 segments a text. A budget of 20
    // lets two texts in (12, then 24 spent); the other eight are past the limit.
    let mut b = board(vec![], vec![]);
    for i in 0..10 {
        let at = p(10.0, 10.0 + f64::from(i) * 5.0);
        b.texts.push(text_on(3, "HHHH", 2.0, at));
    }
    let (list, notes) = overlays_within(&b, 1.6, "Board", 20, MAX_HOLE_VERTICES, MAX_SOLID_FACES);
    assert!(area(named(&list, "Silkscreen top")) > 0.0);
    let m = messages(&notes);
    assert!(
        m.contains(&"8 text(s) past the drawing limit were not drawn".to_string()),
        "{m:?}"
    );
    assert!(
        m.iter()
            .any(|m| m.starts_with("2 text(s) are drawn in the Hershey")),
        "{m:?}"
    );
}

#[test]
fn a_text_whose_strokes_all_fail_is_one_unusable_item_not_drawn() {
    // Near the coordinate limit, so the text passes the checks but its strokes fall outside.
    let mut b = board(vec![], vec![]);
    b.texts.push(text_on(3, "HHHH", 2.0, p(24_999.9, 10.0)));
    let (_, notes) = overlays(&b, 1.6, "Board");
    let m = messages(&notes);
    assert!(
        m.iter().all(|m| !m.contains("Hershey")),
        "a text without a stroke is not counted as drawn: {m:?}"
    );
    assert!(
        m.iter().any(|m| m.starts_with("1 copper or silkscreen")),
        "{m:?}"
    );
}

#[test]
fn truetype_font_notes_are_capped_at_twenty_names() {
    let mut b = board(vec![], vec![]);
    for i in 0..23 {
        let mut t = text_on(3, "H", 2.0, p(10.0, 10.0 + f64::from(i) * 5.0));
        t.font = board_model::TextFont::TrueType {
            name: format!("Font {i:02}"),
        };
        b.texts.push(t);
    }
    let (_, notes) = overlays(&b, 1.6, "Board");
    let m = messages(&notes);
    assert_eq!(
        m.iter().filter(|m| m.contains("TrueType font \"")).count(),
        20
    );
    assert!(
        m.contains(&"and 3 more TrueType font(s) used by drawn texts".to_string()),
        "{m:?}"
    );
}

fn pour(holes: Vec<board_model::Contour>) -> Board {
    board(
        vec![CopperItem::Region {
            layer: 0,
            contour: square(0.0, 0.0, 10.0, 10.0),
            holes,
        }],
        vec![],
    )
}

#[test]
fn a_region_is_its_outline_less_its_holes() {
    // A clockwise hole, as the reader may hand it over: the winding makes no difference.
    let (list, notes) = overlays(&pour(vec![square_cw(4.0, 4.0, 2.0)]), 1.6, "Board");
    let copper = named(&list, "Copper top");
    assert!((area(copper) - 96.0).abs() < 1e-9, "{}", area(copper));
    assert!(!covers(copper, 5.0, 5.0));
    assert!(covers(copper, 1.0, 1.0) && covers(copper, 7.0, 5.0));
    assert!(notes.is_empty(), "{notes:?}");
    // The same with a hole wound counter-clockwise, and with one that strays outside the outline.
    let (list, _) = overlays(
        &pour(vec![
            square(4.0, 4.0, 6.0, 6.0),
            square(9.0, 9.0, 12.0, 12.0),
        ]),
        1.6,
        "Board",
    );
    let copper = named(&list, "Copper top");
    assert!((area(copper) - 95.0).abs() < 1e-9, "{}", area(copper));
}

#[test]
fn a_hole_that_cannot_be_drawn_is_counted_and_the_pour_stays() {
    let flat = board_model::Contour {
        segments: vec![board_model::Segment::Line {
            from: p(1.0, 1.0),
            to: p(1.0, 1.0),
        }],
    };
    let (list, notes) = overlays(&pour(vec![flat, square(4.0, 4.0, 6.0, 6.0)]), 1.6, "Board");
    let copper = named(&list, "Copper top");
    assert!((area(copper) - 96.0).abs() < 1e-9);
    assert!(
        messages(&notes)
            .iter()
            .any(|m| m.starts_with("1 hole(s) of copper regions could not be drawn")),
        "{notes:?}"
    );
    assert!(
        !messages(&notes).iter().any(|m| m.contains("unusable")),
        "{notes:?}"
    );
}

#[test]
fn holes_that_remove_the_whole_region_are_not_unusable() {
    let (list, notes) = overlays(&pour(vec![square(-1.0, -1.0, 11.0, 11.0)]), 1.6, "Board");
    assert!(list.iter().all(|o| o["name"] != "Copper top"));
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn a_hole_touching_the_edge_notches_the_outline() {
    // A 2 x 2 hole on the left edge of the 10 x 10 pour.
    let (list, notes) = overlays(&pour(vec![square(0.0, 4.0, 2.0, 6.0)]), 1.6, "Board");
    let copper = named(&list, "Copper top");
    assert!((area(copper) - 96.0).abs() < 1e-9, "{}", area(copper));
    assert!(!covers(copper, 1.0, 5.0) && covers(copper, 3.0, 5.0));
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn overlapping_holes_are_cut_once() {
    // Two 2 x 2 holes overlapping in a 1 x 1 square: the union is 7.
    let holes = vec![square(3.0, 3.0, 5.0, 5.0), square(4.0, 4.0, 6.0, 6.0)];
    let (list, _) = overlays(&pour(holes), 1.6, "Board");
    let copper = named(&list, "Copper top");
    assert!((area(copper) - 93.0).abs() < 1e-9, "{}", area(copper));
}

#[test]
fn regions_past_the_hole_budget_are_drawn_solid_and_noted() {
    let b = pour(vec![square(4.0, 4.0, 6.0, 6.0)]);
    let (list, notes) = overlays_within(&b, 1.6, "Board", 20, 3, MAX_SOLID_FACES);
    let copper = named(&list, "Copper top");
    assert!((area(copper) - 100.0).abs() < 1e-9, "{}", area(copper));
    assert!(
        messages(&notes)
            .iter()
            .any(|m| m.starts_with("1 copper region(s) are drawn solid")),
        "{notes:?}"
    );
    // With room for the four vertices the hole is cut.
    let (list, _) = overlays_within(&b, 1.6, "Board", 20, 4, MAX_SOLID_FACES);
    assert!((area(named(&list, "Copper top")) - 96.0).abs() < 1e-9);
}

#[test]
fn holes_past_the_cap_are_noted_not_cut() {
    let holes: Vec<_> = (0..MAX_REGION_HOLES + 3)
        .map(|i| {
            let x = 0.5 + (i % 100) as f64 * 0.09;
            let y = 0.5 + (i / 100) as f64 * 0.09;
            square(x, y, x + 0.05, y + 0.05)
        })
        .collect();
    let (_, notes) = overlays(&pour(holes), 1.6, "Board");
    assert!(
        messages(&notes)
            .iter()
            .any(|m| m.starts_with("3 hole(s) past the first 10000")),
        "{notes:?}"
    );
}

/// A 20 x 10 board with, on the copper layer `copper` and the silkscreen layer `silk`: a track
/// along y = 8 from x = 2 to 8 (0.5 wide), a 2 x 2 pad at (15, 5) and a silkscreen line along
/// y = 2.
fn stacked_board(copper: usize, silk: usize) -> Board {
    let pad = CopperItem::Pad {
        layer: copper,
        at: p(15.0, 5.0),
        shape: PadShape::Rectangle,
        size_x: 2.0,
        size_y: 2.0,
        rotation_deg: 0.0,
        drill: None,
        plated: false,
        owner: None,
    };
    let line = Graphic::Line {
        layer: silk,
        from: p(2.0, 2.0),
        to: p(4.0, 2.0),
        width: 0.2,
        owner: None,
    };
    let mut b = board(
        vec![track(copper, p(2.0, 8.0), p(8.0, 8.0), 0.5), pad],
        vec![line],
    );
    b.layers.push(Layer {
        name: "Bottom Overlay".into(),
        kind: LayerKind::Silkscreen,
        side: Some(Side::Bottom),
    });
    b.outline.outer = square(0.0, 0.0, 20.0, 10.0);
    b
}

/// The face count the estimate gives a package overlay: 2 for each area plus its ring vertices.
fn faces_of(o: &Value) -> usize {
    o["areas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            2 + a["outer"].as_array().unwrap().len()
                + a["holes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|h| h.as_array().unwrap().len())
                    .sum::<usize>()
        })
        .sum()
}

#[test]
fn each_overlay_has_its_z_thickness_and_colour() {
    // T = 1.6: copper and mask start at the face, silkscreen on top of the copper (T + 0.035).
    let (list, notes) = overlays(&stacked_board(0, 3), 1.6, "Board");
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(list.len(), 5);
    for (name, z, thickness, color) in [
        ("Copper top", 1.6, 0.035, "#2b6e43"),
        ("Copper top pads", 1.6, 0.035, "#d4af37"),
        ("Solder mask top", 1.6, 0.02, "#1f5f3a"),
        ("Silkscreen top", 1.635, 0.015, "#f2f2f2"),
    ] {
        let o = named(&list, name);
        assert_eq!(o["z"], z, "{name}");
        assert_eq!(o["thickness"], thickness, "{name}");
        assert_eq!(o["color"], color, "{name}");
    }
    assert_eq!(named(&list, "Solder mask top")["opacity"], 0.85);
}

#[test]
fn the_mask_no_longer_covers_a_track() {
    let (list, _) = overlays(&stacked_board(0, 3), 1.6, "Board");
    let mask = named(&list, "Solder mask top");
    let copper = named(&list, "Copper top");
    // On the track (y = 8): copper, no mask. Beside it (y = 8.5, the track ends at 8.25): mask.
    assert!(covers(copper, 5.0, 8.0));
    assert!(!covers(mask, 5.0, 8.0));
    assert!(covers(mask, 5.0, 8.5));
    assert!(!covers(copper, 5.0, 8.5));
    // The bare pad stays open too.
    assert!(!covers(mask, 15.0, 5.0));
}

#[test]
fn the_bottom_side_mirrors_the_top() {
    let (top, _) = overlays(&stacked_board(0, 3), 1.6, "Board");
    let (bottom, notes) = overlays(&stacked_board(1, 6), 1.6, "Board");
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(bottom.len(), top.len());
    for (name, z) in [
        ("Copper bottom", 0.0),
        ("Copper bottom pads", 0.0),
        ("Solder mask bottom", 0.0),
        ("Silkscreen bottom", -0.035),
    ] {
        let o = named(&bottom, name);
        let t = named(&top, &name.replace("bottom", "top"));
        assert_eq!(o["z"], z, "{name}");
        assert_eq!(o["side"], "bottom");
        assert_eq!(o["thickness"], t["thickness"], "{name}");
        assert_eq!(o["color"], t["color"], "{name}");
        assert_eq!(o["areas"], t["areas"], "{name}");
    }
    let mask = named(&bottom, "Solder mask bottom");
    assert!(!covers(mask, 5.0, 8.0));
    assert!(covers(mask, 5.0, 8.5));
}

#[test]
fn the_largest_overlays_are_drawn_flat_past_the_face_budget() {
    let b = stacked_board(0, 3);
    let (full, notes) = overlays(&b, 1.6, "Board");
    assert!(notes.is_empty(), "{notes:?}");
    let total: usize = full.iter().map(faces_of).sum();
    let biggest = full.iter().map(faces_of).max().unwrap();
    assert_eq!(faces_of(named(&full, "Solder mask top")), biggest);
    // A budget just under the total sends only the mask (the largest) flat.
    let (list, notes) = overlays_within(&b, 1.6, "Board", 20, MAX_HOLE_VERTICES, total - 1);
    let flat = named(&list, "Solder mask top");
    assert!(flat.get("thickness").is_none());
    // Flat, the mask sits FLAT_MASK_LIFT above the face: T + 0.005.
    assert_eq!(flat["z"], 1.605);
    assert_eq!(list.len(), full.len());
    for o in list.iter().filter(|o| o["name"] != "Solder mask top") {
        assert!(o.get("thickness").is_some(), "{o}");
    }
    let degraded: Vec<&Value> = notes
        .iter()
        .filter(|n| n["severity"] == "Degraded")
        .collect();
    assert_eq!(degraded.len(), 1, "{notes:?}");
    let message = degraded[0]["message"].as_str().unwrap();
    assert!(message.contains("Solder mask top"), "{message}");
    // No room at all: every overlay is flat and each is named.
    let (list, notes) = overlays_within(&b, 1.6, "Board", 20, MAX_HOLE_VERTICES, 0);
    assert!(list.iter().all(|o| o.get("thickness").is_none()));
    // Flat sheets are lifted off the face, away from the board, so they do not z-fight it.
    for (name, z) in [
        ("Copper top", 1.607),
        ("Copper top pads", 1.601),
        ("Solder mask top", 1.605),
        ("Silkscreen top", 1.61),
        ("Solder mask bottom", -0.005),
    ] {
        assert_eq!(named(&list, name)["z"], z, "{name}");
    }
    let all = messages(&notes).join("\n");
    for o in &list {
        assert!(all.contains(o["name"].as_str().unwrap()), "{all}");
    }
}
