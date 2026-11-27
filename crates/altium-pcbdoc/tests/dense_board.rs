//! The dense board (`dense-board`, "DUTCHMAN NODE 1"): every kind of object the reader knows, many
//! times over, on both sides. Checked against the numbers in tests/boards/README.txt (to 0.001 mm,
//! as AGENTS.md asks).

use altium_pcbdoc::Altium;
use board_model::{
    Color, CopperItem, InputFormat, ModelSource, Placement, ReadOutcome, Segment, Severity, Side,
    TextFont,
};

const TOLERANCE: f64 = 0.001;

fn read() -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/boards/dense-board.PcbDoc");
    Ok(Altium.read(&std::fs::read(path)?)?)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOLERANCE
}

fn part<'a>(out: &'a ReadOutcome, designator: &str) -> Result<&'a Placement, String> {
    out.board
        .components
        .iter()
        .find(|p| p.designator == designator)
        .ok_or_else(|| format!("no {designator}"))
}

#[test]
fn the_dense_board_reads_without_a_warning_or_a_dropped_item(
) -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    let bad: Vec<_> = out
        .notes
        .iter()
        .filter(|n| n.severity != Severity::Info)
        .collect();
    assert!(bad.is_empty(), "{bad:?}");
    assert_eq!(out.board.thickness.map(|t| near(t, 1.6)), Some(true));
    Ok(())
}

#[test]
fn its_outline_has_four_rounded_corners_and_no_cutout() -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    assert!(
        out.board.outline.cutouts.is_empty(),
        "{:?}",
        out.board.outline.cutouts
    );
    let segments = &out.board.outline.outer.segments;
    let radii: Vec<f64> = segments
        .iter()
        .filter_map(|s| match s {
            Segment::Arc { radius, .. } => Some(*radius),
            Segment::Line { .. } => None,
        })
        .collect();
    assert_eq!((segments.len(), radii.len()), (8, 4), "{segments:?}");
    assert!(radii.iter().all(|&r| near(r, 3.0)), "{radii:?}");
    Ok(())
}

#[test]
fn its_34_parts_sit_where_the_readme_says() -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    let parts = &out.board.components;
    let bottom = parts.iter().filter(|p| p.side == Side::Bottom).count();
    assert_eq!((parts.len(), bottom), (34, 10));
    // (designator, footprint, x, y, rotation, side), from README.txt
    let expected = [
        ("J1", "HDR1X4-DN", 3.5, 15.0, 0.0, Side::Top),
        ("J2", "HDR1X4-DN", 46.5, 15.0, 0.0, Side::Top),
        ("U1", "TQFP32-DN", 27.0, 15.0, 0.0, Side::Top),
        ("U2", "SOIC8-DN", 13.5, 7.0, 0.0, Side::Top),
        ("Q1", "SOT23-DN", 20.0, 25.0, 0.0, Side::Top),
        ("R1", "R0603-DN", 17.5, 12.5, 90.0, Side::Top),
        ("C1", "C0603-DN", 5.5, 25.0, 90.0, Side::Top),
        ("C11", "C0402-DN", 24.5, 18.0, 180.0, Side::Bottom),
        ("R12", "R0402-DN", 35.0, 15.0, 180.0, Side::Bottom),
    ];
    for (designator, footprint, x, y, rotation, side) in expected {
        let p = part(&out, designator)?;
        assert_eq!(
            (p.footprint.as_str(), p.side),
            (footprint, side),
            "{designator}"
        );
        assert!(
            near(p.at.x, x) && near(p.at.y, y) && near(p.rotation_deg, rotation),
            "{designator}: {:?} {}",
            p.at,
            p.rotation_deg
        );
    }
    Ok(())
}

#[test]
fn its_bodies_have_their_colors_and_two_parts_carry_the_step_box(
) -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    let color = |r, g, b| {
        Some(Color {
            r,
            g,
            b,
            opacity: 1.0,
        })
    };
    for (designator, want, height) in [
        ("U1", color(30, 30, 30), 1.2),
        ("J1", color(20, 20, 20), 2.5),
        ("D1", color(250, 250, 235), 0.6),
        ("C2", color(190, 150, 100), 0.8),
        ("R1", color(40, 40, 40), 0.45),
        ("C11", color(190, 150, 100), 0.5),
    ] {
        let model = part(&out, designator)?.models.first().ok_or("no model")?;
        let ModelSource::Extruded { height: h, .. } = &model.source else {
            return Err(format!("{designator}: {model:?}").into());
        };
        assert!(near(*h, height), "{designator}: {h}");
        assert_eq!(model.color, want, "{designator}");
    }
    let id = |designator| -> Result<String, String> {
        match &part(&out, designator)?
            .models
            .first()
            .ok_or("no model")?
            .source
        {
            ModelSource::Embedded { id } => Ok(id.clone()),
            other => Err(format!("{designator}: {other:?}")),
        }
    };
    // Each was given the file on its own, so the board holds two copies of the same STEP file.
    assert_ne!(id("R3")?, id("R4")?);
    let [first, second] = out.models.as_slice() else {
        return Err(format!("expected two embedded models, got {}", out.models.len()).into());
    };
    assert_eq!(
        (first.name.as_str(), second.name.as_str()),
        ("box-1608.step", "box-1608.step")
    );
    assert_eq!(first.step, second.step);
    Ok(())
}

#[test]
fn its_copper_holds_every_pad_track_and_via() -> Result<(), Box<dyn std::error::Error>> {
    let out = read()?;
    let copper = &out.board.copper;
    let count = |f: fn(&CopperItem) -> bool| copper.iter().filter(|c| f(c)).count();
    let tracks = count(|c| matches!(c, CopperItem::Track { .. }));
    let vias = count(|c| matches!(c, CopperItem::Via { .. }));
    let plated_holes = count(|c| {
        matches!(
            c,
            CopperItem::Pad {
                drill: Some(_),
                plated: true,
                ..
            }
        )
    });
    let pours = count(|c| matches!(c, CopperItem::Region { .. }));
    // 162 track segments and 40 vias (README.txt); 8 header pins plus 2 mounting holes.
    assert_eq!((tracks, vias, plated_holes), (162, 40, 10));
    assert!(pours >= 2, "{pours}");
    let owned_pads = count(|c| matches!(c, CopperItem::Pad { owner: Some(_), .. }));
    assert_eq!(owned_pads, 110);
    Ok(())
}

#[test]
fn its_silkscreen_has_every_designator_and_the_board_name() -> Result<(), Box<dyn std::error::Error>>
{
    let out = read()?;
    let texts = &out.board.texts;
    assert_eq!(
        texts.len(),
        36,
        "{:?}",
        texts.iter().map(|t| &t.content).collect::<Vec<_>>()
    );
    assert!(texts
        .iter()
        .all(|t| matches!(t.font, TextFont::Stroke { .. })));
    let name = texts
        .iter()
        .find(|t| t.content == "DUTCHMAN NODE 1")
        .ok_or("no board name")?;
    assert!(near(name.at.x, 14.0) && near(name.at.y, 1.2) && near(name.height, 1.2));
    assert!(!name.mirrored);
    let back = texts
        .iter()
        .find(|t| t.content == "v1 CC0")
        .ok_or("no back text")?;
    assert!(back.mirrored && near(back.at.x, 44.0) && near(back.at.y, 2.6));
    for p in &out.board.components {
        assert!(
            texts.iter().any(|t| t.content == p.designator),
            "no designator text for {}",
            p.designator
        );
    }
    Ok(())
}
