//! Internal planes, checked against the numbers in tests/boards/README.txt (to 0.001 mm, as
//! AGENTS.md asks): `four-layer-planes` has two plane layers, Layer 1 (GND) and Layer 2 (VCC,
//! split in two at x = 25, its right half without a net).

use altium_pcbdoc::Altium;
use board_model::{Contour, CopperItem, InputFormat, LayerKind, Point, ReadOutcome, Segment};

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

/// The smallest x, largest x, smallest y and largest y of a contour's straight edges.
type Bounds = (f64, f64, f64, f64);

/// The [`Bounds`] of a contour.
fn bounds(c: &Contour) -> Bounds {
    let mut points = Vec::new();
    for s in &c.segments {
        match *s {
            Segment::Line { from, to } => points.extend([from, to]),
            Segment::Arc { .. } => {}
        }
    }
    let xs = points.iter().map(|p| p.x);
    let ys = points.iter().map(|p| p.y);
    (
        xs.clone().fold(f64::INFINITY, f64::min),
        xs.fold(f64::NEG_INFINITY, f64::max),
        ys.clone().fold(f64::INFINITY, f64::min),
        ys.fold(f64::NEG_INFINITY, f64::max),
    )
}

/// A hole as what it cuts: a round clearance (center, radius), or one piece of a thermal relief
/// (center, inner radius, outer radius).
#[derive(Debug, PartialEq)]
enum Cut {
    Clearance(Point, f64),
    Relief(Point, f64, f64),
}

fn cut(hole: &Contour) -> Option<Cut> {
    let arcs: Vec<(Point, f64)> = hole
        .segments
        .iter()
        .filter_map(|s| match *s {
            Segment::Arc { center, radius, .. } => Some((center, radius)),
            Segment::Line { .. } => None,
        })
        .collect();
    let (center, first) = *arcs.first()?;
    if hole.segments.len() == arcs.len() {
        Some(Cut::Clearance(center, first))
    } else {
        let (_, second) = *arcs.get(1)?;
        Some(Cut::Relief(center, first.min(second), first.max(second)))
    }
}

/// The name of layer `index`.
fn layer_name(out: &ReadOutcome, index: usize) -> &str {
    out.board.layers.get(index).map_or("", |l| l.name.as_str())
}

/// The cuts of the plane region on layer `name` that contains `inside`, sorted.
fn cuts_of(out: &ReadOutcome, name: &str, inside: Point) -> Option<Vec<Cut>> {
    let index = out.board.layers.iter().position(|l| l.name == name)?;
    let region = out.board.copper.iter().find_map(|c| match c {
        CopperItem::Region {
            layer,
            contour,
            holes,
        } if *layer == index => {
            let (x0, x1, y0, y1) = bounds(contour);
            (x0 < inside.x && inside.x < x1 && y0 < inside.y && inside.y < y1).then_some(holes)
        }
        _ => None,
    })?;
    region.iter().map(cut).collect()
}

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn same(a: &Cut, b: &Cut) -> bool {
    let pt = |a: Point, b: Point| near(a.x, b.x) && near(a.y, b.y);
    match (a, b) {
        (Cut::Clearance(c, r), Cut::Clearance(d, s)) => pt(*c, *d) && near(*r, *s),
        (Cut::Relief(c, r1, r2), Cut::Relief(d, s1, s2)) => {
            pt(*c, *d) && near(*r1, *s1) && near(*r2, *s2)
        }
        _ => false,
    }
}

fn assert_cuts(got: &[Cut], want: &[Cut]) {
    assert_eq!(got.len(), want.len(), "{got:?}");
    for w in want {
        assert!(got.iter().any(|g| same(g, w)), "{w:?} not in {got:?}");
    }
}

#[test]
fn four_layer_planes_reads_its_two_planes() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("four-layer-planes.PcbDoc")?;
    let mil = 2.8 * 0.0254;
    for (name, depth) in [
        ("Layer 1", 0.010 + 0.035 + mil),
        ("Layer 2", 0.010 + 0.035 + mil + 0.035 + 1.510),
    ] {
        let layer = out
            .board
            .layers
            .iter()
            .find(|l| l.name == name)
            .ok_or(name)?;
        assert_eq!(layer.kind, LayerKind::Plane, "{name}");
        assert_eq!(layer.side, None, "{name}");
        let place = layer.position.ok_or(name)?;
        assert!(
            near(place.depth, depth) && near(place.thickness, 0.035),
            "{name}: {place:?}"
        );
    }

    // The copper of each region, inside the 0.508 pull-back and the 0.5 wide split line.
    let regions: Vec<(&str, Bounds)> = out
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Region { layer, contour, .. } => {
                Some((layer_name(&out, *layer), bounds(contour)))
            }
            _ => None,
        })
        .collect();
    assert_eq!(regions.len(), 3, "{regions:?}");
    let (lo, hi_x, hi_y) = (0.51046, 50.0 - 0.51046, 30.0 - 0.51046);
    for (name, (x0, x1)) in [
        ("Layer 1", (lo, hi_x)),
        ("Layer 2", (lo, 24.74784)),
        ("Layer 2", (25.25216, hi_x)),
    ] {
        assert!(
            regions.iter().any(|(n, b)| *n == name
                && near(b.0, x0)
                && near(b.1, x1)
                && near(b.2, lo)
                && near(b.3, hi_y)),
            "{name} {x0}..{x1} not in {regions:?}"
        );
    }

    // Clearance 0.3 from the hole; relief from 0.508 to 0.508 + 0.254 out from the hole.
    let (via, pad) = (0.3 / 2.0, 0.9 / 2.0);
    let relief = |at: Point, hole: f64| Cut::Relief(at, hole + 0.508, hole + 0.508 + 0.254);
    let reliefs = |at: Point, hole: f64| (0..4).map(move |_| relief(at, hole));
    // Layer 1 (GND): the GND via and the GND pad join it, the other two vias keep clear.
    let mut want: Vec<Cut> = reliefs(p(10.0, 10.0), via).collect();
    want.extend(reliefs(p(40.0, 10.0), pad));
    want.push(Cut::Clearance(p(10.0, 20.0), via + 0.3));
    want.push(Cut::Clearance(p(35.0, 20.0), via + 0.3));
    assert_cuts(
        &cuts_of(&out, "Layer 1", p(5.0, 5.0)).ok_or("Layer 1")?,
        &want,
    );
    // Layer 2, left half (VCC): the VCC via joins it, the GND via keeps clear.
    let mut want: Vec<Cut> = reliefs(p(10.0, 20.0), via).collect();
    want.push(Cut::Clearance(p(10.0, 10.0), via + 0.3));
    assert_cuts(
        &cuts_of(&out, "Layer 2", p(5.0, 5.0)).ok_or("Layer 2 left")?,
        &want,
    );
    // Layer 2, right half (no net): everything keeps clear.
    let want = [
        Cut::Clearance(p(35.0, 20.0), via + 0.3),
        Cut::Clearance(p(40.0, 10.0), pad + 0.3),
    ];
    assert_cuts(
        &cuts_of(&out, "Layer 2", p(45.0, 5.0)).ok_or("Layer 2 right")?,
        &want,
    );

    // Nothing else is copper on the planes: the pull-back and split tracks are left out.
    let planes: Vec<usize> = (0..out.board.layers.len())
        .filter(|&i| {
            out.board
                .layers
                .get(i)
                .is_some_and(|l| l.kind == LayerKind::Plane)
        })
        .collect();
    for c in &out.board.copper {
        if let CopperItem::Track { layer, .. } | CopperItem::TrackArc { layer, .. } = c {
            assert!(!planes.contains(layer), "{c:?}");
        }
    }
    for (layer, count) in [("Layer 1", 4), ("Layer 2", 5)] {
        let want = format!("{count} track(s) on plane layer {layer} are not drawn");
        assert!(
            out.notes.iter().any(|n| n.message.starts_with(&want)),
            "{want}: {:?}",
            out.notes
        );
    }
    assert!(!out
        .notes
        .iter()
        .any(|n| n.message.contains("polygon pour settings")));
    Ok(())
}

#[test]
fn a_four_layer_board_without_planes_has_none() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("four-layer.PcbDoc")?;
    assert!(out.board.layers.iter().all(|l| l.kind != LayerKind::Plane));
    Ok(())
}
