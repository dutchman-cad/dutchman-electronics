use board_model::{
    Board, Contour, CopperItem, Graphic, Layer, LayerKind, PadShape, Point, Segment, Side,
};
use serde_json::Value;

use super::*;

/// [`super::overlays`] for a board with no embedded fonts.
fn overlays(board: &Board, thickness: f64, board_part: &str) -> (Vec<Value>, Vec<Value>) {
    super::overlays(board, &[], thickness, board_part, Copper::Outer)
}

/// [`super::overlays_within`] for a board with no embedded fonts.
fn overlays_within(
    board: &Board,
    thickness: f64,
    board_part: &str,
    text_budget: usize,
    hole_budget: usize,
    face_budget: usize,
) -> (Vec<Value>, Vec<Value>) {
    super::overlays_within(
        board,
        &[],
        thickness,
        board_part,
        Copper::Outer,
        text_budget,
        hole_budget,
        face_budget,
    )
}

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn layers() -> Vec<Layer> {
    let l = |name: &str, kind, side| Layer {
        name: name.into(),
        kind,
        side,
        position: None,
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
fn the_mask_is_the_outline_less_cutouts_openings_and_drills() {
    // A 20 x 10 board (200 mm2) with a 2 x 2 cutout (4 mm2) and, on the top, a bare 1 x 1 SMD
    // pad and a via of 1 mm across with a 0.5 mm drill. The mask lies over all copper, so the
    // via, which is tented, takes nothing from it. The pad's opening is grown by the default
    // mask expansion of 0.1 mm on each side: (1 + 2 * 0.1)^2 = 1.2^2 = 1.44 mm2.
    // Top mask: 200 - 4 - 1.44 = 194.56. Bottom: no pad, so 200 - 4 = 196.
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
                tented: [Some(true), Some(true)],
                mask_expansion: None,
                layers: None,
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
    // The board file gives no mask opacity, so the default.
    assert_eq!(top["opacity"], 0.75);
    assert_eq!(top["color"], "#1f5f3a");
    // Bottom mask z = 0 - 0: 0.
    assert_eq!(bottom["z"], 0.0);
    assert!((area(top) - 194.56).abs() < 1e-3, "{}", area(top));
    assert!((area(bottom) - 196.0).abs() < 1e-3, "{}", area(bottom));
    // Two holes in the top mask: the cutout and the pad's opening.
    assert_eq!(top["areas"][0]["holes"].as_array().unwrap().len(), 2);
}

#[test]
fn pads_on_the_bottom_only_open_the_bottom_mask() {
    // A 20 x 10 board (200 mm2) and one bare 1 x 1 SMD pad on Bottom Layer: the top mask is
    // whole (200), the bottom mask has the pad's opening, grown by the default mask expansion of
    // 0.1 mm a side: 1.2^2 = 1.44 mm2, so 200 - 1.44 = 198.56.
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
    assert!((area(bottom) - 198.56).abs() < 1e-5, "{}", area(bottom));
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
    assert!(o.get("finish").is_none());
    assert_eq!(o["side"], "top");
    // Copper z = T + 0 = 1.6; its thickness is 0.035.
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
    // Bottom copper z = 0 - 0 = 0, extruded down.
    assert_eq!(o["z"], 0.0);
    assert_eq!(o["side"], "bottom");
}

#[test]
fn a_via_is_a_tented_disc_on_both_faces() {
    // one-via's numbers: diameter 1, hole 0.5. Vias are drawn tented: the copper is the whole
    // disc, with no hole, and the mask lies over it.
    let via = CopperItem::Via {
        at: p(25.0, 15.0),
        diameter: 1.0,
        drill: 0.5,
        tented: [None, None],
        mask_expansion: None,
        layers: None,
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
        assert!(covers(mask, 25.0, 15.0), "{side}: no mask over the via");
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
    for name in ["Finish top", "Finish bottom"] {
        let o = named(&list, name);
        assert_eq!(o["finish"], "bare");
        // A pad's drill stays open, unlike a via's.
        assert!(!covers(o, 25.0, 15.0), "{name}: copper over the hole");
        assert!((area(o) - ring).abs() < 0.02, "{name}: {}", area(o));
    }
}

#[test]
fn copper_is_one_unit_and_the_finish_follows_the_opening() {
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
    // The finish is the copper inside the opening, the pad grown by 0.1 mm: the pad (4 mm2) and
    // the 0.1 mm of track just outside it, 0.1 * 0.5 = 0.05 mm2.
    let finish = named(&list, "Finish top");
    assert!((area(finish) - 4.05).abs() < 1e-6, "{}", area(finish));
    // All copper in one: the track from x = 0 (with its round end) to the pad's edge at x = 9,
    // and the pad.
    let copper = area(named(&list, "Copper top"));
    let exact = 9.0 * 0.5 + PI * 0.25 * 0.25 / 2.0 + 4.0;
    assert!(copper < exact && copper > exact - 0.01, "{copper}");
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
    let o = named(&list, "Finish top");
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
    // Silkscreen z = T + 0.045 (the top of the mask) = 1.645; its thickness is 0.015.
    assert_eq!(o["z"], 1.645);
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
    let o = named(&list, "Finish top");
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
        tented: [None, None],
        mask_expansion: None,
        layers: None,
    };
    let (list, _) = overlays(&board(vec![via], vec![line]), 1.6, "Board");
    let o = named(&list, "Silkscreen top");
    assert!(!covers(o, 5.0, 0.0), "the drill is open");
    assert!(covers(o, 1.0, 0.0));
}

/// A bare 1 x 1 SMD pad at (5, 0) on `layer`.
fn square_pad(layer: usize) -> CopperItem {
    CopperItem::Pad {
        layer,
        at: p(5.0, 0.0),
        shape: PadShape::Rectangle,
        size_x: 1.0,
        size_y: 1.0,
        rotation_deg: 0.0,
        drill: None,
        plated: false,
        owner: None,
    }
}

#[test]
fn the_mask_opening_is_the_pad_grown_by_the_boards_expansion() {
    // A 20 x 10 board (200 mm2) and a bare 2 x 1 pad. With an expansion of 0.25 mm the opening
    // grows 0.25 on each side: (2 + 0.5) * (1 + 0.5) = 3.75 mm2, so the mask is 200 - 3.75 =
    // 196.25. With an expansion of 0 it is the pad itself: 200 - 2 = 198.
    let mut b = board(
        vec![CopperItem::Pad {
            layer: 0,
            at: p(5.0, 5.0),
            shape: PadShape::Rectangle,
            size_x: 2.0,
            size_y: 1.0,
            rotation_deg: 0.0,
            drill: None,
            plated: false,
            owner: None,
        }],
        vec![],
    );
    b.outline.outer = square(0.0, 0.0, 20.0, 10.0);
    b.solder_mask_expansion = Some(0.25);
    let (list, _) = overlays(&b, 1.6, "Board");
    let top = area(named(&list, "Solder mask top"));
    assert!((top - 196.25).abs() < 1e-5, "{top}");
    // The copper itself is not grown.
    let pads = area(named(&list, "Finish top"));
    assert!((pads - 2.0).abs() < 1e-5, "{pads}");
    b.solder_mask_expansion = Some(0.0);
    let (list, _) = overlays(&b, 1.6, "Board");
    let top = area(named(&list, "Solder mask top"));
    assert!((top - 198.0).abs() < 1e-5, "{top}");
}

#[test]
fn a_tented_via_gets_no_mask_expansion() {
    // A 20 x 10 board and a via 1 mm across. The via is tented: however large the expansion,
    // the mask has no opening and lies over the via's copper whole: 200.
    let mut b = board(
        vec![CopperItem::Via {
            at: p(5.0, 5.0),
            diameter: 1.0,
            drill: 0.5,
            tented: [None, None],
            mask_expansion: None,
            layers: None,
        }],
        vec![],
    );
    b.outline.outer = square(0.0, 0.0, 20.0, 10.0);
    b.solder_mask_expansion = Some(0.5);
    let (list, _) = overlays(&b, 1.6, "Board");
    let top = area(named(&list, "Solder mask top"));
    assert!((top - 200.0).abs() < 1e-5, "{top}");
}

#[test]
fn silkscreen_is_clipped_from_the_expanded_mask_openings() {
    // A silkscreen line 10 mm long and 0.2 mm wide along y = 0, and a bare 1 x 1 pad at (5, 0)
    // with a mask expansion of 0.25 mm. The opening spans x from 4.25 to 5.75, so 1.5 mm of
    // the line, 0.2 mm wide, is cut away: 1.5 * 0.2 = 0.3 mm2 less than the line alone. The
    // line is clear of the ring of laminate round the pad, not just of the copper.
    let line = Graphic::Line {
        layer: 3,
        from: p(0.0, 0.0),
        to: p(10.0, 0.0),
        width: 0.2,
        owner: None,
    };
    let mut alone = board(vec![], vec![line.clone()]);
    alone.solder_mask_expansion = Some(0.25);
    let mut padded = board(vec![square_pad(0)], vec![line]);
    padded.solder_mask_expansion = Some(0.25);
    let (list, _) = overlays(&alone, 1.6, "Board");
    let whole = area(named(&list, "Silkscreen top"));
    let (list, _) = overlays(&padded, 1.6, "Board");
    let o = named(&list, "Silkscreen top");
    assert!((whole - area(o) - 0.3).abs() < 1e-5, "{whole} {}", area(o));
    assert!(!covers(o, 5.0, 0.0), "the pad is clear");
    assert!(!covers(o, 5.6, 0.0), "the ring round the pad is clear");
    assert!(!covers(o, 4.4, 0.0), "the ring round the pad is clear");
    assert!(covers(o, 5.9, 0.0));
    assert!(covers(o, 4.1, 0.0));
}

#[test]
fn the_bottom_side_is_expanded_and_clipped_the_same_way() {
    // The same line and pad on the bottom face (layers 6 and 1), default expansion 0.1 mm: the
    // opening spans 5 +- (0.5 + 0.1), so the silkscreen is clear at x = 5.55 and present at
    // x = 5.7; the bottom mask loses 1.2^2 = 1.44 mm2 from 200.
    let mut b = board(
        vec![square_pad(1)],
        vec![Graphic::Line {
            layer: 6,
            from: p(0.0, 0.0),
            to: p(10.0, 0.0),
            width: 0.2,
            owner: None,
        }],
    );
    b.layers.push(Layer {
        name: "Bottom Overlay".into(),
        kind: LayerKind::Silkscreen,
        side: Some(Side::Bottom),
        position: None,
    });
    b.outline.outer = square(0.0, -5.0, 20.0, 5.0);
    let (list, _) = overlays(&b, 1.6, "Board");
    let silk = named(&list, "Silkscreen bottom");
    assert!(!covers(silk, 5.55, 0.0));
    assert!(covers(silk, 5.7, 0.0));
    let mask = area(named(&list, "Solder mask bottom"));
    assert!((mask - 198.56).abs() < 1e-5, "{mask}");
    let top = area(named(&list, "Solder mask top"));
    assert!((top - 200.0).abs() < 1e-5, "{top}");
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
        anchor: board_model::TextAnchor::default(),
        line_pitch: None,
        font: board_model::TextFont::Stroke {
            width: 0.2,
            font: board_model::StrokeFont::Default,
        },
        component: None,
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
    assert!(o.get("finish").is_none());
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
            bold: false,
            italic: false,
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
        position: None,
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
fn each_overlay_has_its_z_thickness_and_color() {
    // T = 1.6: laminate, copper and mask start at the face; the finish on top of the copper
    // (T + 0.035); the mask reaches 0.010 above the copper (0.035 + 0.010 = 0.045 thick) and the
    // silkscreen lies on it (T + 0.045).
    let (list, notes) = overlays(&stacked_board(0, 3), 1.6, "Board");
    assert!(notes.is_empty(), "{notes:?}");
    // Top: laminate, copper, finish, mask, silkscreen; bottom: laminate and mask.
    assert_eq!(list.len(), 7);
    for (name, z, thickness, color) in [
        ("Laminate top", 1.6, 0.005, "#2a2f1c"),
        ("Copper top", 1.6, 0.035, "#b87333"),
        ("Finish top", 1.635, 0.004, "#d4af37"),
        ("Solder mask top", 1.6, 0.045, "#1f5f3a"),
        ("Silkscreen top", 1.645, 0.015, "#f2f2f2"),
    ] {
        let o = named(&list, name);
        assert_eq!(o["z"], z, "{name}");
        assert_eq!(o["thickness"], thickness, "{name}");
        assert_eq!(o["color"], color, "{name}");
    }
    assert_eq!(named(&list, "Solder mask top")["opacity"], 0.75);
    assert_eq!(named(&list, "Laminate top")["kind"], "laminate");
    assert!(named(&list, "Laminate top").get("opacity").is_none());
}

#[test]
fn the_mask_opacity_is_the_files_but_at_least_the_floor() {
    let mut b = stacked_board(0, 3);
    b.appearance.top_solder_mask = Some(board_model::Color {
        r: 17,
        g: 85,
        b: 204,
        opacity: 0.6,
    });
    let (list, _) = overlays(&b, 1.6, "Board");
    // 0.6 is under the 0.75 floor, so the floor is used.
    assert_eq!(named(&list, "Solder mask top")["opacity"], 0.75);
    assert_eq!(named(&list, "Solder mask top")["color"], "#1155cc");
    // A file value above the floor is kept.
    b.appearance.top_solder_mask = Some(board_model::Color {
        r: 17,
        g: 85,
        b: 204,
        opacity: 0.9,
    });
    let (list, _) = overlays(&b, 1.6, "Board");
    assert_eq!(named(&list, "Solder mask top")["opacity"], 0.9);
    // The other side has none, so the default.
    assert_eq!(named(&list, "Solder mask bottom")["opacity"], 0.75);
    // A copper stays copper-colored whatever the mask.
    assert_eq!(named(&list, "Copper top")["color"], "#b87333");
}

#[test]
fn silkscreen_is_real_ink_and_starts_above_the_copper_and_the_mask() {
    // On each side the silkscreen is 0.015 mm thick and its bottom is at or above the top of
    // the copper (0.035), the finish (0.039) and the mask (0.045), so it intersects none.
    for (side, sign, board) in [
        ("top", 1.0, stacked_board(0, 3)),
        ("bottom", -1.0, stacked_board(1, 6)),
    ] {
        let (list, _) = overlays(&board, 1.6, "Board");
        let face = if sign > 0.0 { 1.6 } else { 0.0 };
        // The height of an overlay's lower and upper face away from the board face.
        let span = |name: &str| {
            let o = named(&list, &format!("{name} {side}"));
            let z = (o["z"].as_f64().unwrap() - face) * sign;
            (z, z + o["thickness"].as_f64().unwrap())
        };
        let (silk_bottom, silk_top) = span("Silkscreen");
        let (_, mask_top) = span("Solder mask");
        let (_, copper_top) = span("Copper");
        let (_, finish_top) = span("Finish");
        assert!((finish_top - 0.039).abs() < 1e-9, "{side}: {finish_top}");
        assert!((mask_top - 0.045).abs() < 1e-9, "{side}: {mask_top}");
        assert!(
            mask_top > copper_top && finish_top < mask_top,
            "{side}: the mask covers the copper and the finish"
        );
        assert!(
            silk_bottom >= copper_top - 1e-9,
            "{side}: {silk_bottom} < {copper_top}"
        );
        assert!(
            silk_bottom >= mask_top - 1e-9,
            "{side}: {silk_bottom} < {mask_top}"
        );
        assert!((silk_top - silk_bottom - 0.015).abs() < 1e-9, "{side}");
    }
}

#[test]
fn the_mask_covers_a_track_and_opens_over_a_pad() {
    let (list, _) = overlays(&stacked_board(0, 3), 1.6, "Board");
    let mask = named(&list, "Solder mask top");
    let copper = named(&list, "Copper top");
    // On the track (y = 8): copper under the mask. Beside it (y = 8.5, the track ends at 8.25):
    // mask over bare laminate.
    assert!(covers(copper, 5.0, 8.0));
    assert!(covers(mask, 5.0, 8.0));
    assert!(covers(mask, 5.0, 8.5));
    assert!(!covers(copper, 5.0, 8.5));
    // The pad is copper too, and the mask is open over it, and over its ring of expansion
    // (the pad spans x 14 to 16, the opening 13.9 to 16.1) where there is laminate only.
    assert!(covers(copper, 15.0, 5.0));
    assert!(!covers(mask, 15.0, 5.0));
    assert!(!covers(mask, 16.05, 5.0));
    assert!(covers(mask, 16.2, 5.0));
    let finish = named(&list, "Finish top");
    assert!(covers(finish, 15.0, 5.0));
    assert!(!covers(finish, 16.05, 5.0));
    // The laminate is the whole face.
    assert!(covers(named(&list, "Laminate top"), 5.0, 8.5));
    assert!(covers(named(&list, "Laminate top"), 15.0, 5.0));
}

#[test]
fn the_bottom_side_mirrors_the_top() {
    let (top, _) = overlays(&stacked_board(0, 3), 1.6, "Board");
    let (bottom, notes) = overlays(&stacked_board(1, 6), 1.6, "Board");
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(bottom.len(), top.len());
    for (name, z) in [
        ("Laminate bottom", 0.0),
        ("Copper bottom", 0.0),
        ("Finish bottom", -0.035),
        ("Solder mask bottom", 0.0),
        ("Silkscreen bottom", -0.045),
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
    assert!(covers(mask, 5.0, 8.0));
    assert!(covers(mask, 5.0, 8.5));
    assert!(!covers(mask, 15.0, 5.0));
}

#[test]
fn the_largest_overlays_are_drawn_flat_past_the_face_budget() {
    let b = stacked_board(0, 3);
    let (full, notes) = overlays(&b, 1.6, "Board");
    assert!(notes.is_empty(), "{notes:?}");
    let total: usize = full.iter().map(faces_of).sum();
    let biggest = full.iter().map(faces_of).max().unwrap();
    let largest: Vec<&str> = full
        .iter()
        .filter(|o| faces_of(o) == biggest)
        .map(|o| o["name"].as_str().unwrap())
        .collect();
    assert_eq!(largest.len(), 1, "{largest:?}");
    let largest = largest[0];
    // A budget just under the total sends the largest overlay flat, and with it every other
    // overlay of its side, so that no flat sheet hides inside a solid one (a flat mask under
    // solid copper). The other side keeps its solids.
    let (list, notes) = overlays_within(&b, 1.6, "Board", 20, MAX_HOLE_VERTICES, total - 1);
    let side_of = |name: &str| {
        if name.ends_with("top") {
            "top"
        } else {
            "bottom"
        }
    };
    let largest_side = side_of(largest);
    for o in &list {
        let name = o["name"].as_str().unwrap();
        assert_eq!(
            o.get("thickness").is_none(),
            side_of(name) == largest_side,
            "{largest}: {o}"
        );
    }
    assert_eq!(list.len(), full.len());
    let degraded: Vec<String> = notes
        .iter()
        .filter(|n| n["severity"] == "Degraded")
        .map(|n| n["message"].as_str().unwrap().to_string())
        .collect();
    // One note for each overlay of that side, the largest first among them.
    let flat_names: Vec<&str> = full
        .iter()
        .map(|o| o["name"].as_str().unwrap())
        .filter(|n| side_of(n) == largest_side)
        .collect();
    assert_eq!(degraded.len(), flat_names.len(), "{notes:?}");
    for name in flat_names {
        assert!(
            degraded.iter().any(|m| m.contains(name)),
            "{name}: {degraded:?}"
        );
    }
    // No room at all: every overlay is flat and each is named.
    let (list, notes) = overlays_within(&b, 1.6, "Board", 20, MAX_HOLE_VERTICES, 0);
    assert!(list.iter().all(|o| o.get("thickness").is_none()));
    // Flat sheets are lifted off the face, away from the board, so they do not z-fight it.
    for (name, z) in [
        ("Laminate top", 1.601),
        ("Copper top", 1.603),
        ("Finish top", 1.605),
        ("Solder mask top", 1.607),
        ("Silkscreen top", 1.61),
        ("Solder mask bottom", -0.007),
    ] {
        assert_eq!(named(&list, name)["z"], z, "{name}");
    }
    let all = messages(&notes).join("\n");
    for o in &list {
        assert!(all.contains(o["name"].as_str().unwrap()), "{all}");
    }
}

// ---- TrueType outlines ----

type Fonts = Vec<board_model::EmbeddedFont>;

/// A test board read through the Altium reader: its board and the font files it embeds.
#[cfg(feature = "altium")]
fn read_board(name: &str) -> (Board, Fonts) {
    let path = format!(
        "{}/../altium-pcbdoc/tests/boards/{name}.PcbDoc",
        env!("CARGO_MANIFEST_DIR")
    );
    let bytes = std::fs::read(path).unwrap();
    let outcome = crate::detect(&bytes, Some("PcbDoc"))
        .unwrap()
        .read(&bytes)
        .unwrap();
    (outcome.board, outcome.fonts)
}

/// The box (x0, y0, x1, y1) around every outer ring of an overlay.
fn bounds(o: &Value) -> (f64, f64, f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for a in o["areas"].as_array().unwrap() {
        for q in a["outer"].as_array().unwrap() {
            let (x, y) = (q[0].as_f64().unwrap(), q[1].as_f64().unwrap());
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    (x0, y0, x1, y1)
}

/// A one-text silkscreen board: "one-dejavu-text" with its text changed.
#[cfg(feature = "altium")]
fn dejavu_board(content: &str, rotation_deg: f64, mirrored: bool) -> (Board, Fonts) {
    let (mut b, fonts) = read_board("one-dejavu-text");
    let t = &mut b.texts[0];
    t.content = content.into();
    t.rotation_deg = rotation_deg;
    t.mirrored = mirrored;
    (b, fonts)
}

#[cfg(feature = "altium")]
#[test]
fn dejavu_texts_fill_the_boxes_altium_reports() {
    // The README's boxes. The layout takes the left edge of the ink from the glyphs' bounding
    // boxes, while the boxes here come from the flattened outline, which can lie inside them
    // by up to one chord (0.02 mm) on curved edges. The 0.01 mm tolerance is empirical for these
    // three fonts and letters (the largest difference measured is about 0.004 mm); it is not
    // guaranteed by the chord.
    for (name, want) in [
        ("one-dejavu-text", (10.000, 9.643, 15.936, 11.305)),
        ("one-dejavu-bold-text", (10.000, 9.977, 13.905, 11.305)),
        ("one-dejavu-italic-text", (10.000, 9.977, 16.075, 11.313)),
    ] {
        let (b, fonts) = read_board(name);
        assert_eq!(b.texts.len(), 1, "{name}");
        let (list, notes) = super::overlays(&b, &fonts, 1.6, "Board", Copper::Outer);
        let got = bounds(named(&list, "Silkscreen top"));
        eprintln!("{name}: {got:?} against {want:?}");
        let pairs = [
            (got.0, want.0),
            (got.1, want.1),
            (got.2, want.2),
            (got.3, want.3),
        ];
        for (g, w) in pairs {
            assert!((g - w).abs() < 0.01, "{name}: {got:?} against {want:?}");
        }
        let m = messages(&notes);
        assert!(
            m.iter()
                .all(|m| !m.contains("single strokes") && !m.contains("Hershey")),
            "a text in its own font is not degraded: {m:?}"
        );
    }
}

#[cfg(feature = "altium")]
#[test]
fn a_hole_in_a_letter_stays_a_hole() {
    // "O" is a ring: one area with one hole.
    let (b, fonts) = dejavu_board("O", 0.0, false);
    let (list, _) = super::overlays(&b, &fonts, 1.6, "Board", Copper::Outer);
    let o = named(&list, "Silkscreen top");
    let areas = o["areas"].as_array().unwrap();
    assert_eq!(areas.len(), 1);
    assert_eq!(areas[0]["holes"].as_array().unwrap().len(), 1);
    assert!(area(o) < ring_area(&areas[0]["outer"]) * 0.8);
    // "B" has two holes.
    let (b, fonts) = dejavu_board("B", 0.0, false);
    let (list, _) = super::overlays(&b, &fonts, 1.6, "Board", Copper::Outer);
    let o = named(&list, "Silkscreen top");
    let areas = o["areas"].as_array().unwrap();
    assert_eq!(areas.len(), 1);
    assert_eq!(areas[0]["holes"].as_array().unwrap().len(), 2);
}

#[cfg(feature = "altium")]
#[test]
fn a_mirrored_text_runs_left_and_a_turned_one_runs_up() {
    let flat = {
        let (b, fonts) = dejavu_board("DejaVu", 0.0, false);
        let (list, _) = super::overlays(&b, &fonts, 1.6, "Board", Copper::Outer);
        bounds(named(&list, "Silkscreen top"))
    };
    let height = flat.3 - flat.1;
    // Mirrored (a bottom text): the ink's left edge becomes its right edge, at the position.
    let (mut b, fonts) = dejavu_board("DejaVu", 0.0, true);
    b.layers.push(board_model::Layer {
        name: "Bottom Overlay".into(),
        kind: LayerKind::Silkscreen,
        side: Some(Side::Bottom),
        position: None,
    });
    b.texts[0].layer = b.layers.len() - 1;
    let (list, _) = super::overlays(&b, &fonts, 1.6, "Board", Copper::Outer);
    let m = bounds(named(&list, "Silkscreen bottom"));
    assert!((m.2 - (10.0 - (flat.0 - 10.0))).abs() < 1e-5, "{m:?}");
    assert!((m.0 - (10.0 - (flat.2 - 10.0))).abs() < 1e-5, "{m:?}");
    assert!(
        (m.1 - flat.1).abs() < 1e-5 && (m.3 - flat.3).abs() < 1e-5,
        "{m:?}"
    );
    // Turned 90 degrees counter-clockwise: the text runs up from the position and its letters
    // stand to the left of it (local (x, y) lands at (10 - y, 10 + x)).
    let (b, fonts) = dejavu_board("DejaVu", 90.0, false);
    let (list, _) = super::overlays(&b, &fonts, 1.6, "Board", Copper::Outer);
    let t = bounds(named(&list, "Silkscreen top"));
    assert!(
        (t.1 - flat.0).abs() < 1e-5 && (t.3 - flat.2).abs() < 1e-5,
        "{t:?}"
    );
    assert!((t.0 - (10.0 - (flat.3 - 10.0))).abs() < 1e-5, "{t:?}");
    assert!((t.2 - (10.0 - (flat.1 - 10.0))).abs() < 1e-5, "{t:?}");
    assert!((t.2 - t.0 - height).abs() < 1e-5, "{t:?}");
}

#[cfg(feature = "altium")]
#[test]
fn a_font_that_does_not_parse_falls_back_to_strokes_with_a_note() {
    let (b, mut fonts) = dejavu_board("DejaVu", 0.0, false);
    fonts[0].data = b"this is not a font".to_vec();
    let (list, notes) = super::overlays(&b, &fonts, 1.6, "Board", Copper::Outer);
    assert!(area(named(&list, "Silkscreen top")) > 0.0);
    let m = messages(&notes);
    assert!(
        m.iter().any(|m| m.contains("\"DejaVu Sans Mono\"")
            && m.contains("single strokes")
            && m.contains("cannot be read")),
        "{m:?}"
    );
}

#[cfg(feature = "altium")]
#[test]
fn a_font_the_board_does_not_embed_falls_back_to_strokes_with_a_note() {
    let (mut b, fonts) = dejavu_board("DejaVu", 0.0, false);
    b.texts[0].font = board_model::TextFont::TrueType {
        name: "Arial".into(),
        bold: false,
        italic: false,
    };
    let (list, notes) = super::overlays(&b, &fonts, 1.6, "Board", Copper::Outer);
    assert!(area(named(&list, "Silkscreen top")) > 0.0);
    let m = messages(&notes);
    assert!(
        m.iter()
            .any(|m| m.contains("\"Arial\"") && m.contains("does not embed")),
        "{m:?}"
    );
}

#[cfg(feature = "altium")]
#[test]
fn outline_points_count_against_the_text_budget() {
    // A budget of 20 points holds not even one letter: nothing is drawn, and all six glyphs
    // are counted.
    let (b, fonts) = dejavu_board("DejaVu", 0.0, false);
    let (list, notes) = super::overlays_within(
        &b,
        &fonts,
        1.6,
        "Board",
        Copper::Outer,
        20,
        MAX_HOLE_VERTICES,
        MAX_SOLID_FACES,
    );
    assert!(list.iter().all(|o| o["name"] != "Silkscreen top"));
    let m = messages(&notes);
    assert!(
        m.contains(&"6 glyph(s) past the drawing limit were not drawn".to_string()),
        "{m:?}"
    );
    // A budget for some glyphs but not all draws the first ones and counts the rest.
    let (list, notes) = super::overlays_within(
        &b,
        &fonts,
        1.6,
        "Board",
        Copper::Outer,
        150,
        MAX_HOLE_VERTICES,
        MAX_SOLID_FACES,
    );
    assert!(area(named(&list, "Silkscreen top")) > 0.0);
    assert!(
        messages(&notes)
            .iter()
            .any(|m| m.ends_with("glyph(s) past the drawing limit were not drawn")),
        "{notes:?}"
    );
}

#[cfg(feature = "altium")]
#[test]
fn a_character_the_font_lacks_is_noted() {
    // U+0E01 (Thai) is not in DejaVu Sans Mono; it draws as the .notdef glyph.
    let (b, fonts) = dejavu_board("A\u{0e01}", 0.0, false);
    let (list, notes) = super::overlays(&b, &fonts, 1.6, "Board", Copper::Outer);
    assert!(area(named(&list, "Silkscreen top")) > 0.0);
    assert!(
        messages(&notes)
            .iter()
            .any(|m| m.starts_with("1 character(s) the text's font has no glyph for")),
        "{notes:?}"
    );
}

/// A 20 x 10 rectangle with corners rounded to radius 3, counter-clockwise, as one contour.
fn rounded_rect() -> Contour {
    let line = |a: (f64, f64), b: (f64, f64)| Segment::Line {
        from: p(a.0, a.1),
        to: p(b.0, b.1),
    };
    let arc = |c: (f64, f64), s: f64, e: f64| Segment::Arc {
        center: p(c.0, c.1),
        radius: 3.0,
        start_deg: s,
        end_deg: e,
        clockwise: false,
    };
    Contour {
        segments: vec![
            line((-7.0, -5.0), (7.0, -5.0)),
            arc((7.0, -2.0), 270.0, 0.0),
            line((10.0, -2.0), (10.0, 2.0)),
            arc((7.0, 2.0), 0.0, 90.0),
            line((7.0, 5.0), (-7.0, 5.0)),
            arc((-7.0, 2.0), 90.0, 180.0),
            line((-10.0, 2.0), (-10.0, -2.0)),
            arc((-7.0, -2.0), 180.0, 270.0),
        ],
    }
}

/// The same outline walked the other way round, with clockwise arcs.
fn reversed(c: &Contour) -> Contour {
    Contour {
        segments: c
            .segments
            .iter()
            .rev()
            .map(|s| match *s {
                Segment::Line { from, to } => Segment::Line { from: to, to: from },
                Segment::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                    clockwise,
                } => Segment::Arc {
                    center,
                    radius,
                    start_deg: end_deg,
                    end_deg: start_deg,
                    clockwise: !clockwise,
                },
            })
            .collect(),
    }
}

/// Every point of every arc of `c`, taken `nudge` mm towards the arc's center, 400 to an arc.
fn arc_samples(c: &Contour, nudge: f64) -> Vec<geo::Point> {
    let mut out = Vec::new();
    for s in &c.segments {
        if let Segment::Arc {
            center,
            radius,
            start_deg,
            ..
        } = *s
        {
            let sweep = s.sweep_deg().unwrap();
            for k in 0..=400 {
                let a = (start_deg + sweep * f64::from(k) / 400.0).to_radians();
                let r = radius - nudge;
                out.push(geo::Point::new(
                    center.x + r * a.cos(),
                    center.y + r * a.sin(),
                ));
            }
        }
    }
    out
}

#[test]
fn the_covering_outline_holds_every_point_of_the_true_corner_arcs() {
    use geo::Intersects;
    // The corner arcs bulge out of the region: their chords (inscribed) fall short by up to
    // CHORD, so the covering polygon must go outside them, and by at most the bound derived at
    // `contour_covering`: CHORD / cos(5.625 degrees).
    for c in [rounded_rect(), reversed(&rounded_rect())] {
        let covering = contour_covering(&c).unwrap();
        for pt in arc_samples(&c, 1e-9) {
            assert!(
                covering.intersects(&pt),
                "{pt:?} is outside the covering polygon"
            );
        }
        // No chord polygon gets there: the inscribed ring misses the arcs between its points.
        let inscribed = contour(&c).unwrap();
        let missed = arc_samples(&c, 1e-9)
            .iter()
            .filter(|pt| !inscribed.intersects(*pt))
            .count();
        assert!(missed > 0);
        // And it stays within the bound of the true arc: every vertex is within
        // r (1 / cos(d / 2) - 1) of a circle of its arc, d <= 11.25 degrees.
        let bound = CHORD / (5.625f64).to_radians().cos();
        for v in &covering.exterior().0 {
            let off = [(7.0, -2.0), (7.0, 2.0), (-7.0, 2.0), (-7.0, -2.0)]
                .iter()
                .map(|&(cx, cy)| ((v.x - cx).hypot(v.y - cy) - 3.0).abs())
                .fold(f64::MAX, f64::min);
            // A straight-edge vertex lies on a corner circle too (the arc ends), so only the
            // arc vertices count: those off every line of the rectangle.
            let on_line = v.x.abs() == 10.0 || v.y.abs() == 5.0;
            assert!(
                on_line || off <= bound + 1e-9,
                "{v:?} is {off} from its arc"
            );
        }
    }
}

#[test]
fn the_covering_outline_leaves_a_notch_arc_to_its_chords() {
    use geo::Intersects;
    // A 10 x 10 square with a round notch of radius 2 bitten into its top edge: the arc turns
    // clockwise round the square, with the notch's center outside the region, so the polygon
    // must hold the notch's chords (inscribed in it) and still hold the whole region.
    let c = Contour {
        segments: vec![
            Segment::Line {
                from: p(0.0, 0.0),
                to: p(10.0, 0.0),
            },
            Segment::Line {
                from: p(10.0, 0.0),
                to: p(10.0, 10.0),
            },
            Segment::Line {
                from: p(10.0, 10.0),
                to: p(7.0, 10.0),
            },
            Segment::Arc {
                center: p(5.0, 10.0),
                radius: 2.0,
                start_deg: 0.0,
                end_deg: 180.0,
                clockwise: true,
            },
            Segment::Line {
                from: p(3.0, 10.0),
                to: p(0.0, 10.0),
            },
            Segment::Line {
                from: p(0.0, 10.0),
                to: p(0.0, 0.0),
            },
        ],
    };
    let covering = contour_covering(&c).unwrap();
    // The true region lies on the far side of the notch arc from its center.
    for pt in arc_samples(&c, -1e-9) {
        assert!(covering.intersects(&pt), "{pt:?} is outside");
    }
    assert!(covering.exterior().0.len() < 80);
}

#[test]
fn a_covering_cutout_holds_its_whole_true_hole() {
    use geo::Intersects;
    // A round hole of radius 1.5 as one full-circle arc: the cut polygon holds the true disc, so
    // the mask never covers the hole's rim.
    let hole = Contour {
        segments: vec![Segment::Arc {
            center: p(4.0, 3.0),
            radius: 1.5,
            start_deg: 0.0,
            end_deg: 0.0,
            clockwise: false,
        }],
    };
    let covering = contour_covering(&hole).unwrap();
    for pt in arc_samples(&hole, 1e-9) {
        assert!(covering.intersects(&pt), "{pt:?} is outside the cut");
    }
}

#[test]
fn the_mask_face_holds_the_true_outline_and_its_holes_stay_open() {
    let mut b = board(vec![], vec![]);
    b.outline.outer = rounded_rect();
    b.outline.cutouts = vec![Contour {
        segments: vec![Segment::Arc {
            center: p(0.0, 0.0),
            radius: 1.5,
            start_deg: 0.0,
            end_deg: 0.0,
            clockwise: false,
        }],
    }];
    let (list, _) = overlays(&b, 1.6, "Board");
    let mask = named(&list, "Solder mask top");
    // The corner arc of the true outline is under the mask (a hair inside it), the hole's rim not.
    for pt in arc_samples(&b.outline.outer, 1e-6) {
        assert!(covers(mask, pt.x(), pt.y()), "{pt:?} is not under the mask");
    }
    for pt in arc_samples(&b.outline.cutouts[0], 1e-6) {
        assert!(!covers(mask, pt.x(), pt.y()), "{pt:?} is under the mask");
    }
}

#[test]
fn a_rounded_rectangle_pad_has_its_corners_cut_round() {
    // 3 x 2 with a radius of a quarter of the short side: 0.5 mm.
    let pad = CopperItem::Pad {
        layer: 0,
        at: p(10.0, 5.0),
        shape: PadShape::RoundedRectangle { radius_ratio: 0.25 },
        size_x: 3.0,
        size_y: 2.0,
        rotation_deg: 90.0,
        drill: None,
        plated: false,
        owner: None,
    };
    let (list, _) = overlays(&board(vec![pad], vec![]), 1.6, "Board");
    let pads = named(&list, "Finish top");
    let exact = 6.0 - 4.0 * 0.25 * (1.0 - PI / 4.0);
    assert!((area(pads) - exact).abs() < 0.01, "{}", area(pads));
    // Turned a quarter: 2 wide and 3 tall, and a corner of the plain rectangle is not covered.
    assert!(covers(pads, 10.0, 6.4) && !covers(pads, 10.9, 6.4));
    assert!(covers(pads, 10.9, 5.0) && !covers(pads, 11.1, 5.0));
}

#[test]
fn a_polygon_pad_is_the_union_of_its_pieces_with_its_hole_and_a_grown_opening() {
    // A 1 x 1 square and an L-shaped piece overlapping it by a quarter square: 3.75 mm2, with a
    // 0.4 mm hole at the center.
    let corners = |pts: &[(f64, f64)]| board_model::Contour {
        segments: pts
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| {
                let (nx, ny) = pts[(i + 1) % pts.len()];
                Segment::Line {
                    from: p(x, y),
                    to: p(nx, ny),
                }
            })
            .collect(),
    };
    let pad = CopperItem::PolygonPad {
        layer: 0,
        at: p(10.0, 5.0),
        shapes: vec![
            corners(&[(9.5, 4.5), (10.5, 4.5), (10.5, 5.5), (9.5, 5.5)]),
            corners(&[
                (10.0, 5.0),
                (12.0, 5.0),
                (12.0, 4.0),
                (11.0, 4.0),
                (11.0, 3.0),
                (10.0, 3.0),
            ]),
        ],
        drill: Some(0.4),
        plated: true,
        owner: Some("U1".into()),
    };
    let mut b = board(vec![pad], vec![]);
    b.outline.outer = square(0.0, 0.0, 20.0, 10.0);
    b.solder_mask_expansion = Some(0.25);
    let (list, _) = overlays(&b, 1.6, "Board");
    let pads = named(&list, "Finish top");
    let hole = PI * 0.2 * 0.2;
    assert!((area(pads) - (3.75 - hole)).abs() < 0.01, "{}", area(pads));
    assert!(covers(pads, 11.5, 4.5) && covers(pads, 10.5, 3.5) && !covers(pads, 11.5, 3.5));
    assert!(!covers(pads, 10.0, 5.0), "copper over the hole");
    // The mask opens 0.25 wider than the copper all round: open 0.2 outside the L's end, shut
    // 0.3 outside it.
    let mask = named(&list, "Solder mask top");
    assert!(!covers(mask, 12.2, 4.5) && covers(mask, 12.3, 4.5));
    assert!(!covers(mask, 10.5, 2.8) && covers(mask, 10.5, 2.7));
}

#[test]
fn a_rounded_rectangle_opening_is_the_true_offset_of_the_pad() {
    // 3 x 2, corner radius 0.5, mask expansion 0.25: the opening is the pad offset by 0.25, whose
    // corners have radius 0.75. Its area is A + P * e + pi * e^2 with A = 6 - 4 * 0.25 * (1 -
    // pi/4) = 5.7854 and P = 2 * (3 + 2) - 8 * 0.5 + 2 * pi * 0.5 = 9.1416, so 8.2671; the 20 x 10
    // board's mask is 200 less that.
    let pad = CopperItem::Pad {
        layer: 0,
        at: p(10.0, 5.0),
        shape: PadShape::RoundedRectangle { radius_ratio: 0.25 },
        size_x: 3.0,
        size_y: 2.0,
        rotation_deg: 0.0,
        drill: None,
        plated: false,
        owner: None,
    };
    let mut b = board(vec![pad], vec![]);
    b.outline.outer = square(0.0, 0.0, 20.0, 10.0);
    b.solder_mask_expansion = Some(0.25);
    let (list, _) = overlays(&b, 1.6, "Board");
    let mask = area(named(&list, "Solder mask top"));
    assert!((mask - (200.0 - 8.2671)).abs() < 0.02, "{mask}");
}

fn via_at(
    tented: [Option<bool>; 2],
    mask_expansion: Option<f64>,
    layers: Option<(usize, usize)>,
) -> Board {
    let mut b = board(
        vec![CopperItem::Via {
            at: p(5.0, 5.0),
            diameter: 1.0,
            drill: 0.5,
            tented,
            mask_expansion,
            layers,
        }],
        vec![],
    );
    b.outline.outer = square(0.0, 0.0, 20.0, 10.0);
    b
}

/// The area of an overlay that may be missing when nothing is drawn on it.
fn area_or_zero(list: &[Value], name: &str) -> f64 {
    list.iter().find(|o| o["name"] == name).map_or(0.0, area)
}

#[test]
fn an_open_via_side_has_a_mask_opening_a_ring_of_finish_and_an_open_drill() {
    // A 20 x 10 board (200 mm2), a via 1 mm across with a 0.5 mm drill, open on top and tented
    // on the bottom, with the default expansion 0.1: the top opening is a disc of radius
    // 0.5 + 0.1 = 0.6, pi * 0.36 = 1.1310, so the mask is 200 - 1.1310 = 198.8690. The top copper
    // is a ring, pi * (0.25 - 0.0625) = 0.5890 (the drill is open), and the finish is the copper
    // inside the opening: the same ring. The bottom is tented as before: a whole mask (200) over
    // a solid disc, pi * 0.25 = 0.7854.
    let b = via_at([Some(false), Some(true)], None, None);
    let (list, notes) = overlays(&b, 1.6, "Board");
    assert!(notes.is_empty(), "{notes:?}");
    let mask = named(&list, "Solder mask top");
    assert!((area(mask) - 198.869).abs() < 0.01, "{}", area(mask));
    assert!(!covers(mask, 5.0, 5.0));
    let ring = PI * 0.1875;
    let copper = named(&list, "Copper top");
    assert!((area(copper) - ring).abs() < 0.01, "{}", area(copper));
    assert!(!covers(copper, 5.0, 5.0), "the drill is open");
    let finish = named(&list, "Finish top");
    assert!((area(finish) - ring).abs() < 0.01, "{}", area(finish));
    assert!(!covers(finish, 5.0, 5.0));
    assert!(covers(finish, 5.0, 5.4), "the ring is finished");
    let laminate = named(&list, "Laminate top");
    assert!(!covers(laminate, 5.0, 5.0), "the drill cuts the laminate");
    let bottom_mask = named(&list, "Solder mask bottom");
    assert!((area(bottom_mask) - 200.0).abs() < 1e-5, "{}", area(bottom_mask));
    let bottom = named(&list, "Copper bottom");
    assert!((area(bottom) - PI * 0.25).abs() < 0.01, "{}", area(bottom));
    assert!(covers(bottom, 5.0, 5.0), "the tented side keeps its disc");
    assert!(covers(named(&list, "Laminate bottom"), 5.0, 5.0));
    assert_eq!(area_or_zero(&list, "Finish bottom"), 0.0);
}

#[test]
fn a_via_uses_its_own_mask_expansion_before_the_boards() {
    // Open both sides, the via's own expansion 0.3 (the board's 0.5): each opening has radius
    // 0.5 + 0.3 = 0.8, pi * 0.64 = 2.0106, so each mask is 200 - 2.0106 = 197.9894.
    let mut b = via_at([Some(false), Some(false)], Some(0.3), None);
    b.solder_mask_expansion = Some(0.5);
    let (list, _) = overlays(&b, 1.6, "Board");
    for side in ["top", "bottom"] {
        let mask = area(named(&list, &format!("Solder mask {side}")));
        assert!((mask - 197.9894).abs() < 0.01, "{side}: {mask}");
    }
    // Without its own, the board's: radius 0.5 + 0.5 = 1, pi, so 200 - 3.1416 = 196.8584.
    let b = {
        let mut b = via_at([Some(false), Some(false)], None, None);
        b.solder_mask_expansion = Some(0.5);
        b
    };
    let (list, _) = overlays(&b, 1.6, "Board");
    let mask = area(named(&list, "Solder mask top"));
    assert!((mask - 196.8584).abs() < 0.01, "{mask}");
}

#[test]
fn a_via_the_file_says_nothing_about_is_tented_with_an_info_note() {
    let (list, notes) = overlays(&via_at([None, Some(true)], None, None), 1.6, "Board");
    let mask = area(named(&list, "Solder mask top"));
    assert!((mask - 200.0).abs() < 1e-5, "{mask}");
    let note = notes
        .iter()
        .find(|n| n["message"] == "1 via(s) drawn tented; the file doesn't say")
        .unwrap_or_else(|| panic!("{notes:?}"));
    assert_eq!(note["severity"], "Info");
    // Said both ways: no note.
    let (_, notes) = overlays(&via_at([Some(true), Some(true)], None, None), 1.6, "Board");
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn a_blind_via_is_drawn_only_on_the_face_its_span_reaches() {
    // Span from Top Layer (index 0) to Multi-Layer (index 2, no side): it reaches the top face
    // only. Open on both sides in the file, it opens the top mask (200 - pi * 0.36 = 198.869) and
    // has no ring, opening or drill on the bottom: the bottom mask is whole (200) and the
    // bottom copper is empty.
    let b = via_at([Some(false), Some(false)], None, Some((0, 2)));
    let (list, notes) = overlays(&b, 1.6, "Board");
    assert!(notes.is_empty(), "{notes:?}");
    let top = area(named(&list, "Solder mask top"));
    assert!((top - 198.869).abs() < 0.01, "{top}");
    let bottom = area(named(&list, "Solder mask bottom"));
    assert!((bottom - 200.0).abs() < 1e-5, "{bottom}");
    assert_eq!(area_or_zero(&list, "Copper bottom"), 0.0);
    assert_eq!(area_or_zero(&list, "Finish bottom"), 0.0);
    assert!(covers(named(&list, "Laminate bottom"), 5.0, 5.0));
}

#[test]
fn inner_rings_are_only_on_the_layers_in_the_span() {
    // Four copper layers, 1.6 mm: top (depth 0), In1 (0.5), In2 (1.0), bottom (1.565). A blind
    // via from top to In1 reaches In1 (depth 0.5 is within 0 to 0.5) and not In2 (1.0): In1 has
    // the ring (the disc, pi * 0.25 = 0.7854 less the drill pi * 0.0625 = 0.1963: 0.5890) and In2
    // has nothing. A through via (no span) is on both.
    let layer = |name: &str, side, depth: f64| Layer {
        name: name.into(),
        kind: LayerKind::Copper,
        side,
        position: Some(board_model::StackPosition {
            depth,
            thickness: 0.035,
        }),
    };
    let mut b = via_at([Some(true), Some(true)], None, Some((0, 1)));
    b.layers = vec![
        layer("F.Cu", Some(Side::Top), 0.0),
        layer("In1.Cu", None, 0.5),
        layer("In2.Cu", None, 1.0),
        layer("B.Cu", Some(Side::Bottom), 1.565),
    ];
    let run = |b: &Board| super::overlays(b, &[], 1.6, "Board", Copper::All).0;
    let list = run(&b);
    let ring = PI * 0.1875;
    assert!((area(named(&list, "Copper In1.Cu")) - ring).abs() < 0.01);
    assert_eq!(area_or_zero(&list, "Copper In2.Cu"), 0.0);
    // No bottom ring either: the span does not reach it.
    assert_eq!(area_or_zero(&list, "Copper bottom"), 0.0);
    assert!(area(named(&list, "Copper top")) > 0.5);
    if let CopperItem::Via { layers, .. } = &mut b.copper[0] {
        *layers = None;
    }
    let list = run(&b);
    for name in ["Copper In1.Cu", "Copper In2.Cu"] {
        assert!((area(named(&list, name)) - ring).abs() < 0.01, "{name}");
    }
}
