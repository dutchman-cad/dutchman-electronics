//! The all-features board (`all-features`): dense-board with everything else the reader handles
//! added to it, to see where the reader stands on one board. When the reader learns something
//! new, add it to this board and a check here (see tests/boards/README.txt). Numbers from
//! README.txt, to 0.001 mm as AGENTS.md asks.

use altium_pcbdoc::Altium;
use board_model::{
    Contour, CopperItem, InputFormat, LayerKind, PadShape, Point, ReadOutcome, Segment, Severity,
    TextFont,
};

const TOLERANCE: f64 = 0.001;

fn read() -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards/all-features.PcbDoc");
    Ok(Altium.read(&std::fs::read(path)?)?)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOLERANCE
}

/// Every corner and arc end of a contour.
fn points(c: &Contour) -> Vec<Point> {
    c.segments
        .iter()
        .flat_map(|s| match *s {
            Segment::Line { from, to } => [from, to],
            Segment::Arc { center, radius, .. } => [
                Point {
                    x: center.x - radius,
                    y: center.y - radius,
                },
                Point {
                    x: center.x + radius,
                    y: center.y + radius,
                },
            ],
        })
        .collect()
}

/// (smallest x, smallest y, largest x, largest y) of a contour.
fn bounds(c: &Contour) -> (f64, f64, f64, f64) {
    points(c).iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(a, b, c, d), p| (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
    )
}

fn layer_index(out: &ReadOutcome, name: &str) -> Result<usize, String> {
    out.board
        .layers
        .iter()
        .position(|l| l.name == name)
        .ok_or_else(|| format!("no layer {name}"))
}

/// Everything not read yet is named in a note; this is the list of what this board still loses.
/// A note that is not Info and not on this list is a regression; an item that drops off the list
/// is progress (remove it here and say so in the README).
#[test]
fn only_the_known_gaps_are_warned_about() -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    let known = [
        "1 track(s) on Mechanical 1 (not a copper or overlay layer) not read yet",
        "1 arc(s) on Mechanical 1 (not a copper or overlay layer) not read yet",
        "1 arc(s) on Mechanical 4 (not a copper or overlay layer) not read yet",
        "1 region(s) of KIND 0 on Mechanical 13 not read yet",
        "1 text(s) on Mechanical 2 (not a copper or overlay layer) not read yet",
        "2 copper item(s) on inner layers are left out",
    ];
    let unexpected: Vec<&str> = out
        .notes
        .iter()
        .filter(|n| n.severity != Severity::Info)
        .map(|n| n.message.as_str())
        .filter(|m| !known.iter().any(|k| m.starts_with(k)))
        .collect();
    assert!(unexpected.is_empty(), "{unexpected:?}");
    Ok(())
}

#[test]
fn four_copper_layers_two_of_them_planes_and_enig_on_both_faces(
) -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    let planes: Vec<&str> = out
        .board
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Plane)
        .map(|l| l.name.as_str())
        .collect();
    assert_eq!(planes, ["Layer 1", "Layer 2"]);
    let copper = out
        .board
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Copper && l.position.is_some())
        .count();
    assert_eq!(copper, 2, "Top Layer and Bottom Layer");
    let t = out.board.thickness.ok_or("no thickness")?;
    assert!(near(t, 1.8122), "{t}");
    for plane in planes {
        let i = layer_index(&out, plane)?;
        let regions = out
            .board
            .copper
            .iter()
            .filter(|c| matches!(c, CopperItem::Region { layer, .. } if *layer == i))
            .count();
        assert!(regions >= 1, "{plane} has no copper");
    }
    let finish = |f: &Option<board_model::SurfaceFinish>| f.as_ref().map(|f| f.process.clone());
    assert_eq!(
        finish(&out.board.appearance.top_finish).as_deref(),
        Some("ENIG")
    );
    assert_eq!(
        finish(&out.board.appearance.bottom_finish).as_deref(),
        Some("ENIG")
    );
    Ok(())
}

#[test]
fn the_solder_mask_is_blue_on_both_faces() -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    for mask in [
        out.board.appearance.top_solder_mask,
        out.board.appearance.bottom_solder_mask,
    ] {
        let c = mask.ok_or("no mask color")?;
        assert_eq!((c.r, c.g, c.b), (17, 85, 204));
    }
    Ok(())
}

#[test]
fn the_solder_mask_opens_0_1_mm_round_each_pad() -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    let e = out.board.solder_mask_expansion.ok_or("no mask expansion")?;
    assert!(near(e, 0.1), "{e}");
    Ok(())
}

#[test]
fn the_board_has_one_round_cutout() -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    let [cutout] = out.board.outline.cutouts.as_slice() else {
        return Err(format!("{:?}", out.board.outline.cutouts).into());
    };
    let (x1, y1, x2, y2) = bounds(cutout);
    assert!(
        near(x1, 45.0) && near(y1, 4.0) && near(x2, 48.0) && near(y2, 7.0),
        "{:?}",
        (x1, y1, x2, y2)
    );
    Ok(())
}

#[test]
fn pads_come_in_every_shape() -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    let pad = |x: f64, y: f64| {
        out.board.copper.iter().find_map(|c| match c {
            CopperItem::Pad {
                at,
                shape,
                size_x,
                size_y,
                ..
            } if near(at.x, x) && near(at.y, y) => Some((*shape, *size_x, *size_y)),
            _ => None,
        })
    };
    // U2: eight rounded rectangles, corner 25% (radius ratio 0.125).
    for (x, y) in [(10.8, 8.905), (10.8, 5.095), (16.2, 5.095), (16.2, 8.905)] {
        let (shape, w, h) = pad(x, y).ok_or(format!("no pad at ({x}, {y})"))?;
        assert_eq!(
            shape,
            PadShape::RoundedRectangle {
                radius_ratio: 0.125
            }
        );
        assert!(near(w, 1.5) && near(h, 0.6));
    }
    // J2: pad 1 a rounded rectangle (30%), pads 2 to 4 ovals 2.2 x 1.7.
    let (shape, ..) = pad(46.5, 18.81).ok_or("no J2.1")?;
    assert_eq!(shape, PadShape::RoundedRectangle { radius_ratio: 0.15 });
    for y in [16.27, 13.73, 11.19] {
        let (shape, w, h) = pad(46.5, y).ok_or(format!("no J2 pad at y {y}"))?;
        assert_eq!(shape, PadShape::Oval);
        assert!(near(w, 2.2) && near(h, 1.7));
    }
    // J1: pad 1 square, pads 2 to 4 octagons (polygon pads with a plated hole).
    let (shape, ..) = pad(3.5, 18.81).ok_or("no J1.1")?;
    assert_eq!(shape, PadShape::Rectangle);
    let octagons = out
        .board
        .copper
        .iter()
        .filter(
            |c| matches!(c, CopperItem::PolygonPad { at, drill: Some(_), .. } if near(at.x, 3.5)),
        )
        .count();
    assert_eq!(octagons, 3);
    Ok(())
}

#[test]
fn free_copper_fills_a_region_and_an_arc() -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    let top = layer_index(&out, "Top Layer")?;
    let regions: Vec<(f64, f64, f64, f64)> = out
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Region { layer, contour, .. } if *layer == top => Some(bounds(contour)),
            _ => None,
        })
        .collect();
    let has = |want: (f64, f64, f64, f64)| {
        regions.iter().any(|b| {
            near(b.0, want.0) && near(b.1, want.1) && near(b.2, want.2) && near(b.3, want.3)
        })
    };
    // The fill, the triangle, and the fill turned 30 degrees about its center (40, 2.4): its
    // corners (+-0.8, +-0.7) turned reach x +-1.0428 and y +-1.0062.
    assert!(has((35.8, 1.6, 37.4, 3.2)), "fill: {regions:?}");
    assert!(has((41.8, 1.4, 44.2, 3.6)), "triangle: {regions:?}");
    assert!(
        has((38.9572, 1.3938, 41.0428, 3.4062)),
        "turned fill: {regions:?}"
    );
    let arc = out.board.copper.iter().any(|c| {
        matches!(c, CopperItem::TrackArc { layer, center, radius, width, .. }
            if *layer == top && near(center.x, 25.0) && near(center.y, 3.6)
                && near(*radius, 1.0) && near(*width, 0.2))
    });
    assert!(arc, "no arc at (25, 3.6)");
    Ok(())
}

#[test]
fn texts_use_the_embedded_font_and_a_special_string() -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    let dejavu = out
        .board
        .texts
        .iter()
        .find(|t| t.content == "DejaVu")
        .ok_or("no DejaVu text")?;
    assert!(
        matches!(&dejavu.font, TextFont::TrueType { name, .. } if name == "DejaVu Sans Mono"),
        "{:?}",
        dejavu.font
    );
    assert!(out.font(&dejavu.font).is_some(), "the font is not embedded");
    assert_eq!(out.fonts.len(), 1, "only DejaVu Sans Mono is embedded");
    assert!(near(dejavu.at.x, 31.6) && near(dejavu.at.y, 1.3) && near(dejavu.height, 1.2));
    // ".Comment" on U1 is kept as the board stores it; the command resolves it to U1's comment
    // ("MCU") after reading.
    let mcu = out
        .board
        .texts
        .iter()
        .find(|t| t.content == ".Comment")
        .ok_or("no .Comment text")?;
    assert!(near(mcu.at.x, 25.4) && near(mcu.at.y, 12.7));
    Ok(())
}

#[test]
fn the_parts_of_dense_board_are_all_there() -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    assert_eq!(out.board.components.len(), 34);
    // R3 and R4 each hold their own embedded copy of box-1608.step (see dense-board), and J1
    // holds header-1x4.step.
    let mut names: Vec<&str> = out.models.iter().map(|m| m.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["box-1608.step", "box-1608.step", "header-1x4.step"]);
    Ok(())
}

/// J1 is the through-hole header of `one-step-header`: its body is the same embedded STEP model
/// (housing and four pins reaching 3 mm below the top face), on J1's centre, not turned or raised.
#[test]
fn j1_is_the_through_hole_header_step_model() -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    let j1 = out
        .board
        .components
        .iter()
        .find(|p| p.designator == "J1")
        .ok_or("no J1")?;
    let model = j1.model.as_ref().ok_or("J1 has no model")?;
    assert_eq!(model.name, "header-1x4.step");
    let board_model::ModelSource::Embedded { id } = &model.source else {
        return Err(format!("{:?}", model.source).into());
    };
    assert_eq!(model.offset, [0.0; 3]);
    assert_eq!(model.rotation_deg, [0.0; 3]);
    let step =
        |o: &ReadOutcome, id: &str| o.models.iter().find(|m| m.id == id).map(|m| m.step.clone());
    let header_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards/one-step-header.PcbDoc");
    let header = Altium.read(&std::fs::read(header_path)?)?;
    let original = header.models.first().map(|m| m.step.clone());
    assert!(original.is_some());
    assert_eq!(step(&out, id), original, "not the file of one-step-header");
    Ok(())
}

/// all-features.PrjPcb (beside the board): one board, PCBNumber DN-0001 and BoardRevision A.
/// With them, `.PCBNumber` on Top Overlay shows DN-0001 and `Rev .BoardRevision`, mirrored on
/// Bottom Overlay, shows Rev A, as Altium shows them (README.txt).
#[test]
fn the_project_fills_in_its_parameters() -> Result<(), Box<dyn std::error::Error>> {
    use altium_pcbdoc::{project, special_strings::resolve_texts};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards");
    let p = project::read(&std::fs::read(dir.join("all-features.PrjPcb"))?)?;
    assert_eq!(p.boards, ["all-features.PcbDoc"]);
    assert_eq!(
        p.parameters,
        [
            ("BoardRevision".to_string(), "A".to_string()),
            ("PCBNumber".to_string(), "DN-0001".to_string())
        ]
    );
    let mut out = read()?;
    let stored = |out: &ReadOutcome, s: &str| out.board.texts.iter().any(|t| t.content == s);
    assert!(stored(&out, ".PCBNumber") && stored(&out, "Rev .BoardRevision"));
    resolve_texts(&mut out.board, &p.parameters);
    let number = out
        .board
        .texts
        .iter()
        .find(|t| t.content == "DN-0001")
        .ok_or("no DN-0001")?;
    assert!(near(number.at.x, 6.2) && near(number.at.y, 27.6) && near(number.height, 0.7));
    assert!(!number.mirrored);
    let rev = out
        .board
        .texts
        .iter()
        .find(|t| t.content == "Rev A")
        .ok_or("no Rev A")?;
    assert!(near(rev.at.x, 44.0) && near(rev.at.y, 3.75) && near(rev.height, 0.6));
    assert!(rev.mirrored);
    Ok(())
}
