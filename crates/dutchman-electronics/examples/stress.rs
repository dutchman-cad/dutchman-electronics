//! Synthetic scaling measurements; no board files or external models are opened.
//! Run with --release. Optional arguments: via count and output directory.
use std::path::PathBuf;
use std::time::Instant;

use board_model::{
    Board, Contour, CopperItem, Detection, InputFormat, Layer, LayerKind, PadShape, Point,
    ReadError, ReadOutcome, Segment, Side, StrokeFont, Text, TextAnchor, TextFont,
};
use dutchman_electronics::{overlay, package};

struct Synthetic;
impl InputFormat for Synthetic {
    fn id(&self) -> &'static str {
        "synthetic"
    }
    fn name(&self) -> &'static str {
        "Synthetic stress board"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }
    fn detect(&self, _: &[u8], _: Option<&str>) -> Detection {
        Detection::No
    }
    fn read(&self, _: &[u8]) -> Result<ReadOutcome, ReadError> {
        Err(ReadError::Invalid(
            "we generate this board in memory".into(),
        ))
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Contour {
    let points = [
        Point { x, y },
        Point { x: x + w, y },
        Point { x: x + w, y: y + h },
        Point { x, y: y + h },
    ];
    Contour {
        segments: points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .map(|(&from, &to)| Segment::Line { from, to })
            .collect(),
    }
}

fn board(vias: usize) -> Board {
    let mut b = Board {
        name: format!("stress-{vias}"),
        thickness: Some(1.6),
        layers: vec![
            Layer {
                name: "Top".into(),
                kind: LayerKind::Copper,
                side: Some(Side::Top),
                position: None,
            },
            Layer {
                name: "Bottom".into(),
                kind: LayerKind::Copper,
                side: Some(Side::Bottom),
                position: None,
            },
            Layer {
                name: "Silk".into(),
                kind: LayerKind::Silkscreen,
                side: Some(Side::Top),
                position: None,
            },
        ],
        ..Board::default()
    };
    b.outline.outer = rect(0.0, 0.0, 802.0, 802.0);
    for i in 0..vias {
        b.copper.push(CopperItem::Via {
            at: Point {
                x: 2.0 + (i % 400) as f64 * 2.0,
                y: 2.0 + (i / 400) as f64 * 2.0,
            },
            diameter: 0.7,
            drill: 0.3,
            tented: [Some(false); 2],
            mask_expansion: [None; 2],
            layers: None,
        });
    }
    for i in 0..50_000 {
        b.copper.push(CopperItem::Pad {
            layer: 0,
            at: Point {
                x: 3.0 + (i % 400) as f64 * 2.0,
                y: 3.0 + (i / 400) as f64 * 2.0,
            },
            shape: PadShape::Rectangle,
            size_x: 0.8,
            size_y: 0.6,
            rotation_deg: 0.0,
            bottom: None,
            drill: None,
            plated: false,
            owner: None,
            tented: [None; 2],
            mask_expansion: [None; 2],
        });
    }
    b.copper.push(CopperItem::Region {
        layer: 0,
        contour: rect(1.0, 1.0, 800.0, 800.0),
        holes: (0..5000)
            .map(|i| {
                rect(
                    1.5 + f64::from(i % 100) * 8.0,
                    1.5 + f64::from(i / 100) * 8.0,
                    1.0,
                    1.0,
                )
            })
            .collect(),
    });
    for i in 0..2000 {
        b.texts.push(Text {
            layer: 2,
            at: Point {
                x: 4.0 + f64::from(i % 100) * 8.0,
                y: 520.0 + f64::from(i / 100) * 8.0,
            },
            content: format!("R{i}"),
            height: 1.0,
            rotation_deg: 0.0,
            mirrored: false,
            anchor: TextAnchor::default(),
            line_pitch: None,
            font: TextFont::Stroke {
                width: 0.15,
                font: StrokeFont::SansSerif,
            },
            component: None,
        });
    }
    b
}

fn fixtures(output: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let fixtures: &[(&str, &[u8])] = &[
        (
            "dense-board.PcbDoc",
            include_bytes!("../../altium-pcbdoc/tests/boards/dense-board.PcbDoc"),
        ),
        (
            "all-features.PcbDoc",
            include_bytes!("../../altium-pcbdoc/tests/boards/all-features.PcbDoc"),
        ),
        (
            "k10-all-features.kicad_pcb",
            include_bytes!("../../kicad-pcb/tests/boards/k10-all-features.kicad_pcb"),
        ),
    ];
    for (name, bytes) in fixtures {
        let package = package::build_accepting(
            std::path::Path::new(name),
            bytes,
            &[],
            &[],
            overlay::Copper::Outer,
            package::Bodies::default(),
            package::Accept { laminate: true },
        )?;
        let root = output.join(name);
        std::fs::create_dir_all(&root)?;
        std::fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(&package.manifest)?,
        )?;
        for (name, bytes) in package.files {
            std::fs::write(root.join(name), bytes)?;
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--fixtures") {
        let output = args.get(1).ok_or("we need a fixture output directory")?;
        return fixtures(std::path::Path::new(output));
    }
    let counts = match args.first() {
        Some(n) => vec![n.parse::<usize>()?],
        None => vec![20_000, 100_000],
    };
    let output = args.get(1).map(PathBuf::from);
    for n in counts {
        if n > 100_000 {
            return Err("we cap the synthetic board at 100000 vias".into());
        }
        let start = Instant::now();
        let b = board(n);
        println!(
            "vias={n} synthetic_read={:.3}s",
            start.elapsed().as_secs_f64()
        );
        let start = Instant::now();
        let (overlays, notes) = overlay::overlays(&b, &[], 1.6, "Board", overlay::Copper::Outer);
        println!(
            "vias={n} overlays={:.3}s overlays={} notes={}",
            start.elapsed().as_secs_f64(),
            overlays.len(),
            notes.len()
        );
        drop((overlays, notes));
        let start = Instant::now();
        let step = board_step::write(&b, &b.name)?;
        println!(
            "vias={n} board_step={:.3}s bytes={} holes={}",
            start.elapsed().as_secs_f64(),
            step.text.len(),
            step.holes.len()
        );
        drop(step);
        let outcome = ReadOutcome {
            board: b,
            ..ReadOutcome::default()
        };
        let start = Instant::now();
        let package = package::build_outcome(
            outcome,
            &Synthetic,
            "stress",
            overlay::Copper::Outer,
            package::Bodies::default(),
            package::Accept { laminate: true },
        )?;
        println!(
            "vias={n} package_build={:.3}s files={} bytes={}",
            start.elapsed().as_secs_f64(),
            package.files.len(),
            package.files.iter().map(|(_, b)| b.len()).sum::<usize>()
        );
        if let Some(root) = &output {
            let root = root.join(n.to_string());
            std::fs::create_dir_all(&root)?;
            std::fs::write(
                root.join("manifest.json"),
                serde_json::to_vec(&package.manifest)?,
            )?;
            for (name, bytes) in package.files {
                std::fs::write(root.join(name), bytes)?;
            }
        }
    }
    Ok(())
}
