use super::*;

fn board(name: &str) -> ReadOutcome {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards")
        .join(format!("{name}.circuit.json"));
    CircuitJson.read(&std::fs::read(path).unwrap()).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.001
}

/// Every corner of a contour made of lines.
fn corners(c: &Contour) -> Vec<(f64, f64)> {
    c.segments
        .iter()
        .filter_map(|s| match s {
            Segment::Line { from, .. } => Some((from.x, from.y)),
            Segment::Arc { .. } => None,
        })
        .collect()
}

fn tracks(o: &ReadOutcome) -> Vec<&CopperItem> {
    o.board
        .copper
        .iter()
        .filter(|c| matches!(c, CopperItem::Track { .. }))
        .collect()
}

fn has_note(o: &ReadOutcome, start: &str) -> bool {
    o.notes.iter().any(|n| n.message.contains(start))
}

#[test]
fn outline_only_is_a_centred_rectangle() {
    let o = board("outline-only");
    assert_eq!(o.board.thickness, Some(1.6));
    assert_eq!(
        corners(&o.board.outline.outer),
        vec![(-25.0, -15.0), (25.0, -15.0), (25.0, 15.0), (-25.0, 15.0)]
    );
    assert!(o.board.outline.cutouts.is_empty());
    assert!(o.board.copper.is_empty());
    let copper: Vec<&str> = o
        .board
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Copper && l.side.is_some())
        .map(|l| l.name.as_str())
        .collect();
    assert_eq!(copper, vec!["top", "bottom"]);
    assert!(o.notes.is_empty(), "{:?}", o.notes);
}

#[test]
fn a_hole_and_a_cutout_become_openings() {
    let o = board("holes-cutout");
    let cutouts = &o.board.outline.cutouts;
    assert_eq!(cutouts.len(), 2);
    let round = cutouts
        .iter()
        .find_map(|c| match c.segments.first() {
            Some(Segment::Arc { center, radius, .. }) if c.segments.len() == 1 => {
                Some((*center, *radius))
            }
            _ => None,
        })
        .unwrap();
    assert!(close(round.0.x, -15.0) && close(round.0.y, 0.0) && close(round.1, 1.5));
    let square = cutouts.iter().find(|c| c.segments.len() == 4).unwrap();
    assert_eq!(
        corners(square),
        vec![(5.0, -5.0), (15.0, -5.0), (15.0, 5.0), (5.0, 5.0)]
    );
}

#[test]
fn a_trace_a_via_and_pads() {
    let o = board("traces-vias");
    let t = tracks(&o);
    assert_eq!(t.len(), 1);
    let CopperItem::Track {
        layer,
        from,
        to,
        width,
        ..
    } = t[0]
    else {
        panic!("not a track")
    };
    assert_eq!(o.board.layers[*layer].name, "top");
    assert!(close(from.x, -11.175) && close(to.x, 11.175) && close(*width, 0.15));
    let vias: Vec<_> = o
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Via {
                at,
                diameter,
                drill,
            } => Some((*at, *diameter, *drill)),
            _ => None,
        })
        .collect();
    assert_eq!(vias.len(), 1);
    assert!(close(vias[0].0.y, 8.0) && close(vias[0].1, 0.6) && close(vias[0].2, 0.3));
    let pads = o
        .board
        .copper
        .iter()
        .filter(|c| matches!(c, CopperItem::Pad { .. }))
        .count();
    assert_eq!(pads, 4);
    let names: Vec<&str> = o
        .board
        .components
        .iter()
        .map(|c| c.designator.as_str())
        .collect();
    assert_eq!(names, vec!["R1", "R2"]);
}

#[test]
fn silkscreen_text_and_graphics_are_read() {
    let o = board("silkscreen");
    assert_eq!(o.board.texts.len(), 1);
    let t = &o.board.texts[0];
    assert_eq!(t.content, "DUTCHMAN");
    assert!(close(t.at.x, 0.0) && close(t.at.y, 8.0) && close(t.height, 2.0));
    assert_eq!(o.board.layers[t.layer].kind, LayerKind::Silkscreen);
    // A line, a rectangle (four lines) and a circle (one whole arc).
    let lines = o
        .board
        .graphics
        .iter()
        .filter(|g| matches!(g, Graphic::Line { .. }))
        .count();
    let arcs = o
        .board
        .graphics
        .iter()
        .filter(|g| matches!(g, Graphic::Arc { .. }))
        .count();
    assert_eq!((lines, arcs), (5, 1));
    assert!(!has_note(&o, "silkscreen graphics"), "{:?}", o.notes);
}

/// Reads a hand-written list of elements.
fn read_json(json: &str) -> ReadOutcome {
    CircuitJson.read(json.as_bytes()).unwrap()
}

fn silk(o: &ReadOutcome) -> &[Graphic] {
    &o.board.graphics
}

#[test]
fn silkscreen_path_oval_pill_and_bottom_layer() {
    let o = read_json(
        r#"[
        {"type":"pcb_board","width":10,"height":10,"center":{"x":0,"y":0},"num_layers":2},
        {"type":"pcb_silkscreen_path","layer":"bottom","stroke_width":0.2,
         "route":[{"x":0,"y":0},{"x":1,"y":0},{"x":1,"y":1}]},
        {"type":"pcb_silkscreen_oval","layer":"top","center":{"x":0,"y":0},
         "radius_x":2,"radius_y":1,"stroke_width":0.1},
        {"type":"pcb_silkscreen_pill","layer":"top","center":{"x":0,"y":0},
         "width":4,"height":2,"stroke_width":0.1},
        {"type":"pcb_silkscreen_line","layer":"top","x1":0,"y1":0,"stroke_width":0.1}
    ]"#,
    );
    let g = silk(&o);
    // A path of three points is two lines, on the bottom silkscreen.
    let Graphic::Line { layer, width, to, .. } = &g[1] else {
        panic!("{g:?}")
    };
    assert_eq!(o.board.layers[*layer].side, Some(Side::Bottom));
    assert!(close(*width, 0.2) && close(to.x, 1.0) && close(to.y, 1.0));
    // An oval and a pill are each two lines and two arcs.
    assert_eq!(g.len(), 2 + 4 + 4);
    // The line with no end is unusable and counted.
    assert!(has_note(&o, "pcb_silkscreen_line"), "{:?}", o.notes);
}

#[test]
fn rounded_rectangle_and_polygon_pads() {
    let o = read_json(
        r#"[
        {"type":"pcb_board","width":10,"height":10,"center":{"x":0,"y":0},"num_layers":2},
        {"type":"pcb_smtpad","layer":"top","shape":"rect","x":1,"y":2,
         "width":2,"height":1,"rect_border_radius":0.25},
        {"type":"pcb_smtpad","layer":"top","shape":"rect","x":0,"y":0,"width":2,"height":1},
        {"type":"pcb_smtpad","layer":"top","shape":"polygon",
         "points":[{"x":0,"y":0},{"x":2,"y":0},{"x":1,"y":2}]},
        {"type":"pcb_smtpad","layer":"top","shape":"polygon","points":[{"x":0,"y":0}]}
    ]"#,
    );
    let c = &o.board.copper;
    assert_eq!(c.len(), 3);
    let CopperItem::Pad { shape, .. } = &c[0] else {
        panic!("{c:?}")
    };
    let PadShape::RoundedRectangle { radius_ratio } = shape else {
        panic!("{shape:?}")
    };
    // 0.25 mm on a 1 mm short side.
    assert!(close(*radius_ratio, 0.25));
    assert!(matches!(&c[1], CopperItem::Pad { shape: PadShape::Rectangle, .. }));
    let CopperItem::PolygonPad { shapes, drill, .. } = &c[2] else {
        panic!("{c:?}")
    };
    assert_eq!(shapes.len(), 1);
    assert_eq!(corners(&shapes[0]).len(), 3);
    assert!(drill.is_none());
    assert!(has_note(&o, "pcb_smtpad"), "{:?}", o.notes);
}

#[test]
fn a_rounded_radius_is_capped_at_half_the_short_side() {
    let o = read_json(
        r#"[
        {"type":"pcb_board","width":10,"height":10,"center":{"x":0,"y":0},"num_layers":2},
        {"type":"pcb_smtpad","layer":"top","shape":"rect","x":0,"y":0,
         "width":2,"height":1,"rect_border_radius":5}]"#,
    );
    let CopperItem::Pad { shape, .. } = &o.board.copper[0] else {
        panic!()
    };
    assert_eq!(*shape, PadShape::RoundedRectangle { radius_ratio: 0.5 });
}

#[test]
fn components_are_placed_by_designator() {
    let o = board("components");
    let by_name = |n: &str| {
        o.board
            .components
            .iter()
            .find(|c| c.designator == n)
            .unwrap()
    };
    assert_eq!(o.board.components.len(), 6);
    let c1 = by_name("C1");
    assert!(close(c1.at.x, -20.0) && close(c1.rotation_deg, 90.0));
    assert_eq!(c1.footprint, "cap0805_nonpolarized");
    assert_eq!(by_name("R2").side, Side::Bottom);
    assert_eq!(by_name("R1").side, Side::Top);
    // The header's four 1 mm plated holes: pin 1 has a square pad, so its drill is a board
    // opening; pins 2 to 4 are round pads that carry the drill themselves.
    let openings = o
        .board
        .outline
        .cutouts
        .iter()
        .filter(|c| {
            matches!(c.segments.first(), Some(Segment::Arc { radius, .. }) if close(*radius, 0.5))
        })
        .count();
    let drilled_pads = o
        .board
        .copper
        .iter()
        .filter(|c| matches!(c, CopperItem::Pad { drill: Some(d), .. } if close(*d, 1.0)))
        .count();
    assert_eq!((openings, drilled_pads), (1, 3));
    assert!(
        has_note(&o, "6 component(s) have no 3D model"),
        "{:?}",
        o.notes
    );
}

#[test]
fn a_shaped_outline_keeps_its_corners() {
    let o = board("shaped-outline");
    assert_eq!(
        corners(&o.board.outline.outer),
        vec![
            (0.0, 0.0),
            (40.0, 0.0),
            (40.0, 15.0),
            (20.0, 15.0),
            (20.0, 30.0),
            (0.0, 30.0)
        ]
    );
    assert_eq!(o.board.outline.cutouts.len(), 1);
}

#[test]
fn four_layers_have_two_inner_layers() {
    let o = board("four-layer");
    let copper: Vec<&str> = o
        .board
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Copper && l.name != "all copper layers")
        .map(|l| l.name.as_str())
        .collect();
    assert_eq!(copper, vec!["top", "inner1", "inner2", "bottom"]);
}

#[test]
fn detect_by_contents_and_name() {
    let head = br#"[{"type":"source_group","source_group_id":"g"}]"#;
    assert_eq!(CircuitJson.detect(head, Some("json")), Detection::Yes);
    assert_eq!(
        CircuitJson.detect(b"{}", Some("circuit.json")),
        Detection::MaybeByName
    );
    assert_eq!(CircuitJson.detect(b"[1,2]", Some("json")), Detection::No);
}

#[test]
fn hostile_input_is_an_error_not_a_panic() {
    assert!(CircuitJson.read(b"not json").is_err());
    assert!(CircuitJson.read(b"{}").is_err());
    assert!(CircuitJson.read(b"[]").is_err());
    let no_size = br#"[{"type":"pcb_board","thickness":1.6}]"#;
    assert!(CircuitJson.read(no_size).is_err());
    let huge = br#"[{"type":"pcb_board","center":{"x":0,"y":0},"width":1e300,"height":5}]"#;
    assert!(CircuitJson.read(huge).is_err());
    let bad_hole =
        br#"[{"type":"pcb_board","center":{"x":0,"y":0},"width":5,"height":5,"thickness":1},
        {"type":"pcb_hole","hole_shape":"circle","x":"nope","y":0,"hole_diameter":1}]"#;
    let o = CircuitJson.read(bad_hole).unwrap();
    assert!(o.board.outline.cutouts.is_empty());
    assert!(o.notes.iter().any(|n| n.message.contains("pcb_hole")));
}

#[test]
fn lengths_with_units() {
    assert_eq!(length_of(&Value::from("2mm")), Some(2.0));
    assert!(close(length_of(&Value::from("100mil")).unwrap(), 2.54));
    assert!(close(length_of(&Value::from("0.1in")).unwrap(), 2.54));
    assert_eq!(length_of(&Value::from(1.5)), Some(1.5));
    assert_eq!(length_of(&Value::from("abc")), None);
}

#[test]
fn a_rounded_rectangle_is_closed() {
    let c = rect(Point::default(), 10.0, 6.0, 30.0, 1.0);
    assert_eq!(c.segments.len(), 8);
    for (a, b) in c.segments.iter().zip(c.segments.iter().cycle().skip(1)) {
        let end = match a {
            Segment::Line { to, .. } => *to,
            Segment::Arc {
                center,
                radius,
                end_deg,
                ..
            } => Point {
                x: center.x + radius * end_deg.to_radians().cos(),
                y: center.y + radius * end_deg.to_radians().sin(),
            },
        };
        let start = match b {
            Segment::Line { from, .. } => *from,
            Segment::Arc {
                center,
                radius,
                start_deg,
                ..
            } => Point {
                x: center.x + radius * start_deg.to_radians().cos(),
                y: center.y + radius * start_deg.to_radians().sin(),
            },
        };
        assert!(close(end.x, start.x) && close(end.y, start.y));
    }
}
