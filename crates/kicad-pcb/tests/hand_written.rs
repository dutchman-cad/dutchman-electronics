//! Small boards written by hand from KiCad's published file-format documentation, for the parts
//! of the format none of our KiCad-made boards use yet (the stack-up table, old versions, items
//! the reader does not read). Real boards that hold these are still wanted; see README.md.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::indexing_slicing
)]

use board_model::{InputFormat, ReadError, Severity};
use kicad_pcb::KiCad;

fn read(text: &str) -> Result<board_model::ReadOutcome, ReadError> {
    KiCad.read(text.as_bytes())
}

const HEAD: &str = "(kicad_pcb (version 20241229) (generator \"pcbnew\") (generator_version \"9.0\")\n\
  (general (thickness 1.6))\n\
  (layers (0 \"F.Cu\" signal) (2 \"B.Cu\" signal) (25 \"Edge.Cuts\" user) (17 \"Dwgs.User\" user))\n";

#[test]
fn the_stackup_gives_the_thickness() {
    let text = format!(
        "{HEAD}(setup (stackup\n\
          (layer \"F.Mask\" (type \"Top Solder Mask\") (thickness 0.01))\n\
          (layer \"F.Cu\" (type \"copper\") (thickness 0.035))\n\
          (layer \"dielectric 1\" (type \"core\") (thickness 1.51) (material \"FR4\"))\n\
          (layer \"B.Cu\" (type \"copper\") (thickness 0.035))\n\
          (layer \"B.Mask\" (type \"Bottom Solder Mask\") (thickness 0.01))\n\
          (layer \"F.SilkS\" (type \"Top Silk Screen\"))))\n)"
    );
    let o = read(&text).unwrap();
    assert!((o.board.thickness.unwrap() - 1.6).abs() < 1e-9);
    assert!(!o
        .notes
        .iter()
        .any(|n| n.message.contains("stack-up adds up")));
}

#[test]
fn a_stackup_that_disagrees_with_the_general_thickness_wins_with_a_warning() {
    let text = format!(
        "{HEAD}(setup (stackup\n\
          (layer \"F.Cu\" 1 (type \"copper\") (thickness 0.035))\n\
          (layer \"dielectric 1\" 2 (type \"core\") (thickness 0.5))\n\
          (layer \"B.Cu\" 3 (type \"copper\") (thickness 0.035))))\n)"
    );
    let o = read(&text).unwrap();
    assert!((o.board.thickness.unwrap() - 0.57).abs() < 1e-9);
    assert!(o.notes.iter().any(
        |n| n.severity == Severity::Warning && n.message.contains("stack-up adds up to 0.570")
    ));
}

#[test]
fn an_older_file_gets_a_note_and_no_board() {
    let o =
        read("(kicad_pcb (version 20240108) (generator \"pcbnew\") (layers (0 \"F.Cu\" signal)))")
            .unwrap();
    assert!(o.board.outline.outer.segments.is_empty());
    assert!(o.board.layers.is_empty());
    assert_eq!(o.notes.len(), 1);
    assert_eq!(o.notes[0].severity, Severity::Dropped);
    assert!(o.notes[0].message.contains("older than 9"));
    let o = read("(kicad_pcb (generator \"pcbnew\"))").unwrap();
    assert!(o.notes[0].message.contains("no version"));
}

#[test]
fn a_newer_version_is_read() {
    let text = "(kicad_pcb (version 20261231) (generator \"pcbnew\") (generator_version \"10.0\")\n\
      (layers (0 \"F.Cu\" signal) (25 \"Edge.Cuts\" user))\n\
      (gr_rect (start 0 0) (end 10 5) (stroke (width 0.1) (type default)) (fill no) (layer \"Edge.Cuts\")))";
    let o = read(text).unwrap();
    assert_eq!(o.board.outline.outer.segments.len(), 4);
}

#[test]
fn what_is_not_read_is_named() {
    let text = format!(
        "{HEAD}(dimension (type aligned))\n\
         (gr_line (start 0 0) (end 1 1) (layer \"Dwgs.User\") (stroke (width 0.1) (type default)))\n\
         (zone (net 0) (net_name \"\") (layer \"F.Cu\") (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1))))\n\
         (zone (keepout (tracks not_allowed)) (layer \"F.Cu\"))\n)"
    );
    let o = read(&text).unwrap();
    let dropped: Vec<&str> = o
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Dropped)
        .map(|n| n.message.as_str())
        .collect();
    let has = |w: &str| dropped.iter().any(|m| m.contains(w));
    assert!(has("\"dimension\""), "{dropped:?}");
    assert!(has("layer Dwgs.User"), "{dropped:?}");
    assert!(has("no filled copper"), "{dropped:?}");
    assert!(has("keep-out"), "{dropped:?}");
    assert!(
        has("no closed outline")
            || o.notes
                .iter()
                .any(|n| n.message.contains("no closed outline"))
    );
}

#[test]
fn an_outline_that_does_not_close_is_left_out_with_a_note() {
    let text = format!(
        "{HEAD}(gr_line (start 0 0) (end 10 0) (layer \"Edge.Cuts\") (stroke (width 0.1) (type default)))\n\
         (gr_line (start 10 0) (end 10 10) (layer \"Edge.Cuts\") (stroke (width 0.1) (type default)))\n)"
    );
    let o = read(&text).unwrap();
    assert!(o.board.outline.outer.segments.is_empty());
    assert!(o.notes.iter().any(|n| n.message.contains("do not close")));
}

#[test]
fn net_names_given_as_strings_are_read() {
    let text = format!(
        "{HEAD}(segment (start 0 0) (end 1 0) (width 0.2) (layer \"F.Cu\") (net \"GND\"))\n)"
    );
    let o = read(&text).unwrap();
    assert!(matches!(
        o.board.copper.first(),
        Some(board_model::CopperItem::Track { net: Some(n), .. }) if n == "GND"
    ));
}

#[test]
fn text_that_is_not_a_board_is_an_error() {
    assert!(matches!(
        read("(kicad_sch (version 1))"),
        Err(ReadError::Invalid(_))
    ));
    assert!(matches!(read("hello"), Err(ReadError::Invalid(_))));
    assert!(matches!(read(""), Err(ReadError::Invalid(_))));
}

#[test]
fn a_model_turns_the_other_way_about_each_axis_and_is_not_scaled_silently() {
    let text = format!(
        "{HEAD}(footprint \"lib:U\" (layer \"F.Cu\") (at 10 10)\n\
           (property \"Reference\" \"U1\" (at 0 0 0) (layer \"F.SilkS\") (hide yes))\n\
           (model \"C:\\\\models\\\\box.step\" (offset (xyz 1 2 3)) (scale (xyz 2 1 1)) (rotate (xyz 90 0 -45))))\n)"
    );
    let o = read(&text).unwrap();
    let m = o.board.components[0].models.first().unwrap();
    assert_eq!(m.name, "box.step");
    assert_eq!(m.offset, [1.0, 2.0, 3.0]);
    assert_eq!(m.rotation_deg, [-90.0, 0.0, 45.0]);
    assert!(o.notes.iter().any(|n| n.message.contains("scale")));
    // A hidden reference is not drawn.
    assert!(o.board.texts.is_empty());
}

#[test]
fn absurdly_big_numbers_drop_their_items_with_a_note() {
    let text = format!(
        "{HEAD}(gr_rect (start 0 0) (end 10 5) (layer \"Edge.Cuts\") (stroke (width 0.1) (type default)))\n\
         (gr_rect (start 0 0) (end 1e300 5) (layer \"Edge.Cuts\") (stroke (width 0.1) (type default)))\n\
         (via (at 1e300 1) (size 0.8) (drill 0.4) (layers \"F.Cu\" \"B.Cu\"))\n\
         (via (at 1 1) (size 1e300) (drill 0.4) (layers \"F.Cu\" \"B.Cu\"))\n\
         (segment (start 0 0) (end 1 1) (width 1e300) (layer \"F.Cu\"))\n\
         (gr_text \"A\" (at 1 1) (layer \"F.Cu\") (effects (font (size 1e300 1e300))))\n)"
    );
    let o = read(&text).unwrap();
    // Only the good rectangle is left; nothing infinite or huge reaches the board.
    assert_eq!(o.board.outline.outer.segments.len(), 4);
    assert!(o.board.outline.cutouts.is_empty());
    assert!(o.board.copper.is_empty());
    // The text keeps its default height, since its size was unreadable.
    assert!(o.board.texts.iter().all(|t| t.height < 10.0));
    assert!(o
        .notes
        .iter()
        .any(|n| n.severity == Severity::Dropped && n.message.contains("beyond 100 m")));
}

#[test]
fn a_slotted_hole_is_as_wide_as_its_narrow_side_and_an_offset_is_noted() {
    let text = format!(
        "{HEAD}(footprint \"lib:J\" (layer \"F.Cu\") (at 10 10)\n\
           (property \"Reference\" \"J1\" (at 0 0 0) (layer \"F.SilkS\") (effects (font (size 1 1))))\n\
           (pad \"1\" thru_hole oval (at 0 0) (size 3 2) (drill oval 1.2 0.8 (offset 0.3 0)) (layers \"*.Cu\")))\n)"
    );
    let o = read(&text).unwrap();
    let drill = o.board.copper.iter().find_map(|c| match c {
        board_model::CopperItem::Pad { drill, .. } => *drill,
        _ => None,
    });
    assert_eq!(drill, Some(0.8));
    assert!(o.notes.iter().any(|n| n.message.contains("narrow side")));
    assert!(o
        .notes
        .iter()
        .any(|n| n.severity == Severity::Warning && n.message.contains("off-center")));
}

#[test]
fn hidden_and_empty_texts_are_noted() {
    let text = format!(
        "{HEAD}(gr_text \"\" (at 1 1) (layer \"F.Cu\"))\n\
         (gr_text \"X\" (at 1 1) (layer \"F.Cu\") (hide yes))\n)"
    );
    let o = read(&text).unwrap();
    assert!(o.board.texts.is_empty());
    assert!(o.notes.iter().any(|n| n.message.contains("hidden text")));
    assert!(o.notes.iter().any(|n| n.message.contains("empty text")));
}

#[test]
fn a_fill_that_pinches_at_a_corner_is_two_pieces_without_holes() {
    // Two squares that touch at (10, 10), written as one ring that passes it twice.
    let text = format!(
        "{HEAD}(zone (net 0) (net_name \"\") (layer \"F.Cu\") (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1)))\n\
         (filled_polygon (layer \"F.Cu\") (pts (xy 0 0) (xy 10 0) (xy 10 10) (xy 20 10) (xy 20 20) \
         (xy 10 20) (xy 10 10) (xy 0 10))))\n)"
    );
    let o = read(&text).unwrap();
    let holes: Vec<usize> = o
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            board_model::CopperItem::Region { holes, .. } => Some(holes.len()),
            _ => None,
        })
        .collect();
    assert_eq!(holes, vec![0, 0]);
}

#[test]
fn a_fill_with_one_bridge_gives_an_outer_contour_and_one_hole() {
    let text = format!(
        "{HEAD}(zone (net 0) (net_name \"\") (layer \"F.Cu\") (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1)))\n\
         (filled_polygon (layer \"F.Cu\") (pts (xy 0 0) (xy 10 0) (xy 10 10) (xy 0 10) (xy 0 5) \
         (xy 4 5) (xy 4 7) (xy 6 7) (xy 6 5) (xy 4 5) (xy 0 5))))\n)"
    );
    let o = read(&text).unwrap();
    let (contour, holes) = o
        .board
        .copper
        .iter()
        .find_map(|c| match c {
            board_model::CopperItem::Region { contour, holes, .. } => Some((contour, holes)),
            _ => None,
        })
        .unwrap();
    // The bridge's start (0, 5) stays a vertex of the outer contour (on its left edge).
    assert_eq!(contour.segments.len(), 5);
    assert_eq!(holes.len(), 1);
    assert_eq!(holes[0].segments.len(), 4);
}

#[test]
fn a_pad_with_a_size_that_is_not_positive_is_dropped_with_a_note() {
    for size in ["-3 2", "3 0", "0 0", "3 -2"] {
        let text = format!(
            "{HEAD}(footprint \"t:x\" (layer \"F.Cu\") (at 5 5) (property \"Reference\" \"U1\")\n\
             (pad \"1\" smd roundrect (at 0 0) (size {size}) (layers \"F.Cu\") (roundrect_rratio 0.25) \
             (chamfer_ratio 0.25) (chamfer top_left)))\n)"
        );
        let o = read(&text).unwrap();
        assert!(
            !o.board.copper.iter().any(|c| matches!(
                c,
                board_model::CopperItem::Pad { .. } | board_model::CopperItem::PolygonPad { .. }
            )),
            "{size}"
        );
        assert!(o.notes.iter().any(
            |n| n.severity == Severity::Dropped && n.message.contains("not a positive number")
        ));
    }
}

#[test]
fn hostile_pad_shapes_do_not_panic() {
    for body in [
        "(pad \"1\" smd trapezoid (at 0 0) (size 1 1) (rect_delta 5 5) (layers \"F.Cu\"))",
        "(pad \"1\" smd roundrect (at 0 0) (size 1 1) (roundrect_rratio 7) (chamfer_ratio -3) (chamfer top_left top_right bottom_left bottom_right) (layers \"F.Cu\"))",
        "(pad \"1\" smd custom (at 0 0) (size 1 1) (layers \"F.Cu\") (options (anchor circle)) (primitives (gr_poly (pts (xy 0 0) (arc (start 1 0) (mid 2 1) (end 3 0))) (width 0) (fill yes)) (gr_circle (center 0 0) (end 0 0) (fill yes)) (gr_line (start 0 0) (end 0 0) (width 0.1))))",
    ] {
        let text = format!(
            "{HEAD}(footprint \"t:x\" (layer \"F.Cu\") (at 5 5) (property \"Reference\" \"U1\")\n{body})\n)"
        );
        read(&text).unwrap();
    }
}

#[test]
fn an_arc_in_a_custom_pad_polygon_stays_an_arc() {
    let text = format!(
        "{HEAD}(footprint \"t:x\" (layer \"F.Cu\") (at 5 5) (property \"Reference\" \"U1\")\n\
         (pad \"1\" smd custom (at 0 0) (size 1 1) (layers \"F.Cu\") (options (anchor rect)) \
         (primitives (gr_poly (pts (xy 0 0) (xy 2 0) (arc (start 2 0) (mid 3 1) (end 2 2)) (xy 0 2)) \
         (width 0) (fill yes)))))\n)"
    );
    let o = read(&text).unwrap();
    assert!(!o.notes.iter().any(|n| n.message.contains("straight edges")));
    let shapes = o
        .board
        .copper
        .iter()
        .find_map(|c| match c {
            board_model::CopperItem::PolygonPad { shapes, .. } => Some(shapes),
            _ => None,
        })
        .unwrap();
    // The anchor, and the polygon: three lines and one arc of radius 1 about (2, -1) from the pad, which sits at (5, -5) (Y is up).
    let arcs: Vec<_> = shapes[1]
        .segments
        .iter()
        .filter_map(|s| match s {
            board_model::Segment::Arc { center, radius, .. } => Some((*center, *radius)),
            _ => None,
        })
        .collect();
    assert_eq!(arcs.len(), 1);
    assert!((arcs[0].1 - 1.0).abs() < 1e-9);
    assert!((arcs[0].0.x - 7.0).abs() < 1e-9 && (arcs[0].0.y + 6.0).abs() < 1e-9);
}

fn font_text(embedded: &str) -> String {
    format!(
        "{HEAD}(gr_text \"Hi\" (at 5 5 0) (layer \"F.Cu\") \
         (effects (font (face \"Test Sans\") (size 1 1))))\n{embedded})"
    )
}

#[test]
fn a_font_the_board_does_not_embed_keeps_the_stroke_font_with_a_note_naming_it() {
    let o = read(&font_text("")).unwrap();
    assert!(matches!(
        o.board.texts[0].font,
        board_model::TextFont::Stroke { .. }
    ));
    assert!(o.fonts.is_empty());
    assert!(o.notes.iter().any(|n| n.severity == Severity::Warning
        && n.message.contains("\"Test Sans\"")
        && n.message.contains("stroke font")));
}

#[test]
fn hostile_embedded_font_data_is_a_note_not_a_panic() {
    for data in [
        "",
        "|not base64!|",
        "|AAAA|",
        // zstd magic and a frame header declaring 4 GB.
        "|KLUv/aD/////|",
        // a valid zstd frame (an empty file) that is not a font.
        "|KLUv/QBYAAA=|",
    ] {
        let embedded = format!(
            "(embedded_fonts yes) (embedded_files (file (name \"x.ttf\") (type font) (data {data}) \
             (checksum \"00\")))"
        );
        let o = read(&font_text(&embedded)).unwrap();
        assert!(matches!(
            o.board.texts[0].font,
            board_model::TextFont::Stroke { .. }
        ));
        assert!(o.fonts.is_empty());
        assert!(o.notes.iter().any(|n| n.severity == Severity::Warning));
    }
}

fn zone_text(pts: &str) -> String {
    format!(
        "{HEAD}(zone (net 0) (net_name \"\") (layer \"F.Cu\") (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1)))\n\
         (filled_polygon (layer \"F.Cu\") (pts {pts})))\n)"
    )
}

#[test]
fn a_fill_with_two_pieces_and_two_thousand_holes_is_split_and_assigned() {
    // A tall piece with 2000 bridged holes down its left side, pinched at one corner to a small
    // second piece.
    let height = 8000.0;
    let mut ring = format!(
        "(xy 0 0) (xy 1000 0) (xy 1000 {height}) (xy 1010 {height}) (xy 1010 {h2}) (xy 1000 {h2}) \
         (xy 1000 {height}) (xy 0 {height}) ",
        h2 = height + 10.0
    );
    for k in 1..=2000 {
        let y = height - 4.0 * f64::from(k);
        ring.push_str(&format!(
            "(xy 0 {y}) (xy 2 {y}) (xy 2 {y1}) (xy 3 {y1}) (xy 3 {y}) (xy 2 {y}) (xy 0 {y}) ",
            y1 = y + 1.0
        ));
    }
    let o = read(&zone_text(&ring)).unwrap();
    let mut regions: Vec<usize> = o
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            board_model::CopperItem::Region { holes, .. } => Some(holes.len()),
            _ => None,
        })
        .collect();
    regions.sort_unstable();
    assert_eq!(regions, vec![0, 2000]);
    assert!(!o
        .notes
        .iter()
        .any(|n| n.message.contains("kept as the one contour")));
}

#[test]
fn a_hole_inside_no_piece_is_left_out_with_a_note() {
    // Two pinched squares and a hole that hangs off the first one's corner, outside both.
    let ring =
        "(xy 0 0) (xy 10 0) (xy 10 10) (xy 20 10) (xy 20 20) (xy 10 20) (xy 10 10) (xy 0 10) \
        (xy -5 10) (xy -5 11) (xy -4 11) (xy -4 10) (xy -5 10) (xy 0 10)";
    let o = read(&zone_text(ring)).unwrap();
    let holes: Vec<usize> = o
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            board_model::CopperItem::Region { holes, .. } => Some(holes.len()),
            _ => None,
        })
        .collect();
    assert_eq!(holes, vec![0, 0]);
    assert!(o
        .notes
        .iter()
        .any(|n| n.severity == Severity::Warning && n.message.contains("inside none")));
}

#[test]
fn a_fill_with_arcs_is_kept_whole_with_a_note() {
    let ring = "(xy 0 0) (xy 10 0) (arc (start 10 0) (mid 11 5) (end 10 10)) (xy 0 10)";
    let o = read(&zone_text(ring)).unwrap();
    assert!(o.notes.iter().any(|n| {
        n.severity == Severity::Warning && n.message.contains("kept as the one contour")
    }));
}

fn via_layers(span: &str) -> board_model::ReadOutcome {
    read(&format!(
        "{HEAD}(via blind (at 1 2) (size 0.8) (drill 0.4) (layers {span}) (net 0))\n)"
    ))
    .unwrap()
}

#[test]
fn a_blind_via_needs_two_different_copper_layers_or_it_reads_as_through() {
    let warning = "blind or micro via(s) whose layers are not two different copper layers of the \
                   board were read as through vias";
    for span in [
        "\"F.Cu\" \"F.Cu\"",
        "\"F.Cu\" \"Dwgs.User\"",
        "\"F.Cu\" \"In9.Cu\"",
        "\"F.Cu\"",
    ] {
        let o = via_layers(span);
        let layers = o.board.copper.iter().find_map(|c| match c {
            board_model::CopperItem::Via { layers, .. } => Some(*layers),
            _ => None,
        });
        assert_eq!(layers, Some(None), "{span}");
        assert!(
            o.notes
                .iter()
                .any(|n| n.severity == Severity::Warning && n.message.contains(warning)),
            "{span}: {:?}",
            o.notes
        );
    }
    // Two different copper layers are a span (here the only two layers, so it is read as one
    // whatever the board does with it).
    let o = via_layers("\"F.Cu\" \"B.Cu\"");
    assert!(o.board.copper.iter().any(|c| matches!(
        c,
        board_model::CopperItem::Via {
            layers: Some(_),
            ..
        }
    )));
}

#[test]
fn an_empty_tenting_side_reads_as_covered() {
    // `(front)` with no value is read as `(front yes)`, as elsewhere in the reader.
    let o = read(&format!(
        "{HEAD}(via (at 1 2) (size 0.8) (drill 0.4) (layers \"F.Cu\" \"B.Cu\") (tenting (front) (back no)) (net 0))\n)"
    ))
    .unwrap();
    assert!(o.board.copper.iter().any(|c| matches!(
        c,
        board_model::CopperItem::Via {
            tented: [Some(true), Some(false)],
            ..
        }
    )));
}

fn arc_pad_board(prim: &str) -> board_model::ReadOutcome {
    let text = format!(
        "{HEAD}(footprint \"t:x\" (layer \"F.Cu\") (at 5 5) (property \"Reference\" \"U1\")\n\
         (pad \"1\" smd custom (at 0 0) (size 1 1) (layers \"F.Cu\") (options (anchor rect)) \
         (primitives {prim})))\n)"
    );
    read(&text).unwrap()
}

fn pad_shapes(o: &board_model::ReadOutcome) -> Vec<board_model::Contour> {
    o.board
        .copper
        .iter()
        .find_map(|c| match c {
            board_model::CopperItem::PolygonPad { shapes, .. } => Some(shapes.clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn a_clockwise_arc_stroke_and_its_counter_clockwise_twin_are_the_same_band() {
    // A quarter circle of radius 2 about the pad's center, written both ways round, and the
    // same with a half circle and with three quarters.
    for (a, m, b) in [
        ("2 0", "1.4142136 -1.4142136", "0 -2"),
        ("2 0", "0 -2", "-2 0"),
        ("2 0", "-2 0", "0 2"),
    ] {
        let one = arc_pad_board(&format!(
            "(gr_arc (start {a}) (mid {m}) (end {b}) (width 0.3))"
        ));
        let twin = arc_pad_board(&format!(
            "(gr_arc (start {b}) (mid {m}) (end {a}) (width 0.3))"
        ));
        // Shape 0 is the anchor; the stroke is what follows.
        let (s1, s2) = (pad_shapes(&one), pad_shapes(&twin));
        assert!(s1.len() >= 2, "{a} {m} {b}");
        // The two read the circle from the same points in another order, so they agree to the
        // last digits only: compare to six decimals.
        let show = |v: &[board_model::Contour]| format!("{v:.6?}").replace("-0.000000", "0.000000");
        assert_eq!(show(&s1), show(&s2), "{a} {m} {b}");
        assert!(!one.notes.iter().any(|n| n.severity == Severity::Dropped));
    }
}

#[test]
fn a_gr_arc_through_points_on_one_line_is_a_straight_stroke() {
    let o = arc_pad_board("(gr_arc (start 0 0) (mid 1 0) (end 2 0) (width 0.3))");
    let shapes = pad_shapes(&o);
    // The anchor and one capsule: two lines and two arcs, 2 + 0.3 long.
    assert_eq!(shapes.len(), 2);
    assert_eq!(shapes[1].segments.len(), 4);
    assert!(!o.notes.iter().any(|n| n.severity == Severity::Dropped));
}

#[test]
fn a_pads_solder_mask_margin_wins_over_its_footprints() {
    let text = format!(
        "{HEAD}(footprint \"t:x\" (layer \"F.Cu\") (at 5 5) (solder_mask_margin 0.07) \
         (property \"Reference\" \"U1\")\n\
         (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (solder_mask_margin -0.05))\n\
         (pad \"2\" smd rect (at 2 0) (size 1 1) (layers \"F.Cu\")))\n\
         (footprint \"t:y\" (layer \"F.Cu\") (at 15 5) (property \"Reference\" \"U2\")\n\
         (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\")))\n)"
    );
    let o = read(&text).unwrap();
    let got: Vec<_> = o
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            board_model::CopperItem::Pad {
                mask_expansion,
                tented,
                ..
            } => Some((*mask_expansion, *tented)),
            _ => None,
        })
        .collect();
    assert_eq!(
        got,
        vec![
            ([Some(-0.05); 2], [None; 2]),
            ([Some(0.07); 2], [None; 2]),
            ([None; 2], [None; 2]),
        ]
    );
}

#[test]
fn a_copper_polygon_with_an_arc_keeps_it_filled_or_as_its_edge() {
    // On F.Cu: from (10, 9) an arc through (11, 10) to (10, 11) (radius 1 about (10, 10)), then
    // straight to (9.5, 11), (9.5, 9) and back. Filled, it is one region whose edge holds the arc;
    // not filled, its edge is three tracks and a track arc.
    let shape = |fill: &str| {
        format!(
            "{HEAD}(gr_poly (pts (arc (start 10 9) (mid 11 10) (end 10 11)) (xy 9.5 11) (xy 9.5 9))\n\
               (stroke (width 0.1) (type solid)) (fill {fill}) (layer \"F.Cu\"))\n)"
        )
    };
    let o = read(&shape("yes")).unwrap();
    assert!(
        o.notes.iter().all(|n| n.severity != Severity::Dropped),
        "{:?}",
        o.notes
    );
    let region = o.board.copper.iter().find_map(|c| match c {
        board_model::CopperItem::Region { contour, holes, .. } => Some((contour, holes)),
        _ => None,
    });
    let (contour, holes) = region.unwrap();
    assert!(holes.is_empty());
    assert!(contour.segments.iter().any(|s| matches!(
        s,
        board_model::Segment::Arc { radius, .. } if (radius - 1.0).abs() < 1e-9
    )));
    let o = read(&shape("no")).unwrap();
    assert!(
        o.notes.iter().all(|n| n.severity != Severity::Dropped),
        "{:?}",
        o.notes
    );
    let arcs = o
        .board
        .copper
        .iter()
        .filter(|c| matches!(c, board_model::CopperItem::TrackArc { radius, start_deg, end_deg, .. }
            if (radius - 1.0).abs() < 1e-9 && (start_deg - 270.0).abs() < 1e-6 && (end_deg - 90.0).abs() < 1e-6))
        .count();
    assert_eq!(arcs, 1);
    let tracks = o
        .board
        .copper
        .iter()
        .filter(|c| matches!(c, board_model::CopperItem::Track { .. }))
        .count();
    assert_eq!(tracks, 3);
    assert!(!o
        .board
        .copper
        .iter()
        .any(|c| matches!(c, board_model::CopperItem::Region { .. })));
}

#[test]
fn a_footprint_zone_fill_off_copper_is_named_and_not_copper() {
    let text = format!(
        "{HEAD}(footprint \"lib:Z\" (layer \"F.Cu\") (at 10 10)\n\
           (zone (layer \"Dwgs.User\") (polygon (pts (xy 9 9) (xy 11 9) (xy 11 11)))\n\
             (filled_polygon (layer \"Dwgs.User\") (pts (xy 9 9) (xy 11 9) (xy 11 11)))))\n)"
    );
    let o = read(&text).unwrap();
    assert!(!o
        .board
        .copper
        .iter()
        .any(|c| matches!(c, board_model::CopperItem::Region { .. })));
    assert!(o.notes.iter().any(|n| n.severity == Severity::Dropped
        && n.message == "1 zone fill(s) on layer Dwgs.User (only copper zones are read)"));
}
