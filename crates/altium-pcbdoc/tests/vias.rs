//! The via and stroke-font test boards (tests/boards/README.txt, "Vias and stroke fonts"): sizes,
//! places, tenting, per-via mask expansion and layer spans as Altium reports them, to 0.001 mm as
//! AGENTS.md asks, and the stroke font of each text.

use altium_pcbdoc::Altium;
use board_model::{CopperItem, InputFormat, ReadOutcome, Severity, StrokeFont, TextFont};

const TOLERANCE: f64 = 0.001;

fn read(name: &str) -> Result<ReadOutcome, Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/boards")
        .join(name);
    Ok(Altium.read(&std::fs::read(path)?)?)
}

/// One via as the board model holds it.
#[derive(Debug)]
struct Via {
    x: f64,
    y: f64,
    diameter: f64,
    drill: f64,
    tented: [Option<bool>; 2],
    mask_expansion: Option<f64>,
    layers: Option<(String, String)>,
}

/// Every via of a board, sorted by x then y; spans as layer names.
fn vias(out: &ReadOutcome) -> Vec<Via> {
    let name = |i: usize| {
        out.board
            .layers
            .get(i)
            .map(|l| l.name.clone())
            .unwrap_or_default()
    };
    let mut v: Vec<Via> = out
        .board
        .copper
        .iter()
        .filter_map(|c| match c {
            CopperItem::Via {
                at,
                diameter,
                drill,
                tented,
                mask_expansion,
                layers,
            } => Some(Via {
                x: at.x,
                y: at.y,
                diameter: *diameter,
                drill: *drill,
                tented: *tented,
                mask_expansion: *mask_expansion,
                layers: layers.map(|(a, b)| (name(a), name(b))),
            }),
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    v
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOLERANCE
}

/// (x, y, diameter, drill) of every via.
fn places(v: &[Via]) -> Vec<(f64, f64, f64, f64)> {
    v.iter().map(|v| (v.x, v.y, v.diameter, v.drill)).collect()
}

fn same(got: &[(f64, f64, f64, f64)], want: &[(f64, f64, f64, f64)]) -> bool {
    got.len() == want.len()
        && got
            .iter()
            .zip(want)
            .all(|(g, w)| near(g.0, w.0) && near(g.1, w.1) && near(g.2, w.2) && near(g.3, w.3))
}

#[test]
fn via_sizes_are_read_as_altium_reports_them() -> Result<(), Box<dyn std::error::Error>> {
    let want = [
        (8.0, 15.0, 0.4, 0.2),
        (16.0, 15.0, 0.6, 0.3),
        (24.0, 15.0, 0.8, 0.4),
        (32.0, 15.0, 1.0, 0.5),
        (41.0, 15.0, 1.5, 0.8),
    ];
    for board in ["vias-sizes.PcbDoc", "vias-tented-rule.PcbDoc"] {
        let got = places(&vias(&read(board)?));
        assert!(same(&got, &want), "{board}: {got:?}");
    }
    Ok(())
}

#[test]
fn open_vias_are_read_open_and_the_rule_tents_every_via() -> Result<(), Box<dyn std::error::Error>>
{
    for v in vias(&read("vias-sizes.PcbDoc")?) {
        assert_eq!(v.tented, [Some(false), Some(false)], "{v:?}");
        assert_eq!(v.mask_expansion, None, "{v:?}");
    }
    // The rule tents them; their expansion still comes from the rule, so none is given.
    for v in vias(&read("vias-tented-rule.PcbDoc")?) {
        assert_eq!(v.tented, [Some(true), Some(true)], "{v:?}");
        assert_eq!(v.mask_expansion, None, "{v:?}");
    }
    // The older boards' vias are open too (their rule does not tent).
    for v in vias(&read("one-via.PcbDoc")?) {
        assert_eq!(v.tented, [Some(false), Some(false)], "{v:?}");
    }
    Ok(())
}

#[test]
fn each_via_of_the_tenting_board_has_its_own_mask() -> Result<(), Box<dyn std::error::Error>> {
    let got = vias(&read("vias-tenting.PcbDoc")?);
    // (x, tented top, tented bottom, manual expansion)
    let want = [
        (8.0, false, false, None),
        (16.0, true, false, Some(0.1)),
        (24.0, false, true, Some(0.1)),
        (32.0, true, true, Some(0.1)),
        (40.0, false, false, Some(0.2)),
    ];
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (v, (x, top, bottom, expansion)) in got.iter().zip(want) {
        assert!(near(v.x, x) && near(v.y, 15.0), "{v:?}");
        assert!(near(v.diameter, 0.6) && near(v.drill, 0.3), "{v:?}");
        assert_eq!(v.tented, [Some(top), Some(bottom)], "{v:?}");
        match (v.mask_expansion, expansion) {
            (Some(g), Some(w)) => assert!(near(g, w), "{v:?}"),
            (g, w) => assert_eq!(g, w, "{v:?}"),
        }
        assert_eq!(v.layers, None, "{v:?}");
    }
    Ok(())
}

#[test]
fn blind_and_buried_vias_keep_their_span() -> Result<(), Box<dyn std::error::Error>> {
    let out = read("vias-blind-buried.PcbDoc")?;
    let got = vias(&out);
    let span = |a: &str, b: &str| Some((a.to_string(), b.to_string()));
    // (x, y, span), the upper layer first
    let want = [
        (10.0, 25.0, span("Top Layer", "Layer 1")),
        (20.0, 25.0, span("Layer 2", "Bottom Layer")),
        (30.0, 15.0, None),
        (30.0, 25.0, span("Layer 1", "Layer 2")),
        (40.0, 25.0, span("Top Layer", "Layer 2")),
    ];
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (v, (x, y, layers)) in got.iter().zip(want) {
        assert!(near(v.x, x) && near(v.y, y), "{v:?}");
        assert!(near(v.diameter, 0.6) && near(v.drill, 0.3), "{v:?}");
        assert_eq!(v.layers, layers, "{v:?}");
    }
    let messages: Vec<&str> = out.notes.iter().map(|n| n.message.as_str()).collect();
    assert!(
        messages.contains(&"4 blind or buried via(s) read with the copper layers they join"),
        "{messages:?}"
    );
    assert!(
        out.notes
            .iter()
            .all(|n| n.severity != Severity::Warning || !n.message.contains("read as running")),
        "{messages:?}"
    );
    Ok(())
}

#[test]
fn each_text_has_the_stroke_font_altium_shows() -> Result<(), Box<dyn std::error::Error>> {
    use StrokeFont::{Default, SansSerif, Serif};
    let out = read("stroke-fonts.PcbDoc")?;
    let mut texts: Vec<(f64, StrokeFont)> = out
        .board
        .texts
        .iter()
        .filter_map(|t| match &t.font {
            TextFont::Stroke { font, .. } => Some((t.at.y, *font)),
            TextFont::TrueType { .. } => None,
        })
        .collect();
    texts.sort_by(|a, b| b.0.total_cmp(&a.0));
    let fonts: Vec<StrokeFont> = texts.iter().map(|t| t.1).collect();
    assert_eq!(
        fonts,
        [Default, Default, Default, SansSerif, SansSerif, SansSerif, Serif, Serif, Serif]
    );
    assert!(out
        .notes
        .iter()
        .all(|n| !n.message.contains("stroke font other")));
    Ok(())
}
