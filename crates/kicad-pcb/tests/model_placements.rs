//! Where a model lands: the reader's placement, model offset and model rotation, composed the way
//! `board_model::Placement` says, against what KiCad itself does (its VRML export, numbers in
//! tests/boards/model-placements.README.txt) and the pad positions KiCad reports.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::indexing_slicing
)]

use board_model::{CopperItem, InputFormat, Placement, Side};
use kicad_pcb::KiCad;

type M = [[f64; 3]; 3];

fn mul(a: &M, b: &M) -> M {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

fn turn(axis: usize, deg: f64) -> M {
    let (s, c) = deg.to_radians().sin_cos();
    match axis {
        0 => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        1 => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        _ => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}

fn apply(m: &M, v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| (0..3).map(|k| m[i][k] * v[k]).sum())
}

/// A rotation from an axis (unit) and an angle in radians.
fn axis_angle(a: [f64; 3], t: f64) -> M {
    let (s, c) = t.sin_cos();
    let u = 1.0 - c;
    [
        [
            c + a[0] * a[0] * u,
            a[0] * a[1] * u - a[2] * s,
            a[0] * a[2] * u + a[1] * s,
        ],
        [
            a[1] * a[0] * u + a[2] * s,
            c + a[1] * a[1] * u,
            a[1] * a[2] * u - a[0] * s,
        ],
        [
            a[2] * a[0] * u - a[1] * s,
            a[2] * a[1] * u + a[0] * s,
            c + a[2] * a[2] * u,
        ],
    ]
}

/// What the reader says the model's turn and offset are, in the board's frame (Y up), by
/// `Placement`'s rules: the frame is turned by the angle about Z, and a bottom part's is then
/// flipped about its own X axis; the model is turned about X, then Y, then Z, then moved by its
/// offset.
fn model_in_board(p: &Placement) -> (M, [f64; 3]) {
    let m = p.models.first().unwrap();
    let flip: M = match p.side {
        Side::Top => turn(0, 0.0),
        Side::Bottom => turn(0, 180.0),
    };
    let frame = mul(&turn(2, p.rotation_deg), &flip);
    let own = mul(
        &turn(2, m.rotation_deg[2]),
        &mul(&turn(1, m.rotation_deg[1]), &turn(0, m.rotation_deg[0])),
    );
    (mul(&frame, &own), apply(&frame, m.offset))
}

struct Expect {
    part: &'static str,
    offset: [f64; 3],
    axis: [f64; 3],
    angle: f64,
    pads: [(f64, f64); 2],
    pad_turn: f64,
}

const EXPECT: [Expect; 4] = [
    Expect {
        part: "U1",
        offset: [2.0, 1.0, -0.5],
        axis: [0.57735027, 0.57735027, 0.57735027],
        angle: 2.0943951,
        pads: [(11.5, 17.0), (13.5, 14.0)],
        pad_turn: 90.0,
    },
    Expect {
        part: "U2",
        offset: [-0.5, 1.0, -0.25],
        axis: [0.57735027, -0.57735027, -0.57735027],
        angle: 4.1887902,
        pads: [(25.5, 13.0), (23.5, 16.0)],
        pad_turn: 270.0,
    },
    Expect {
        part: "U3",
        offset: [0.0, 0.0, 0.0],
        axis: [-0.57735027, -0.57735027, -0.57735027],
        angle: 2.0943951,
        pads: [(36.0, 15.5), (39.0, 13.5)],
        pad_turn: 0.0,
    },
    Expect {
        part: "U4",
        offset: [1.0, 1.5, 0.3],
        axis: [-0.86285621, -0.35740674, 0.35740674],
        angle: 1.7177715,
        pads: [(38.5, 27.0), (36.5, 24.0)],
        pad_turn: 90.0,
    },
];

fn check(name: &str) {
    let path = format!(
        "{}/tests/boards/{name}.kicad_pcb",
        env!("CARGO_MANIFEST_DIR")
    );
    let o = KiCad.read(&std::fs::read(path).unwrap()).unwrap();
    for e in &EXPECT {
        let p = o
            .board
            .components
            .iter()
            .find(|c| c.designator == e.part)
            .unwrap();
        let (rot, off) = model_in_board(p);
        let want = axis_angle(e.axis, e.angle);
        for i in 0..3 {
            assert!(
                (off[i] - e.offset[i]).abs() < 0.001,
                "{name} {}: offset {off:?} vs {:?}",
                e.part,
                e.offset
            );
            for j in 0..3 {
                assert!(
                    (rot[i][j] - want[i][j]).abs() < 0.001,
                    "{name} {}: rotation {rot:?} vs {want:?}",
                    e.part
                );
            }
        }
        // KiCad's own pad positions (KiCad's Y down, relative to the board corner).
        let mut found: Vec<(f64, f64, f64)> = o
            .board
            .copper
            .iter()
            .filter_map(|c| match c {
                CopperItem::Pad {
                    at,
                    owner: Some(d),
                    rotation_deg,
                    ..
                } if d == e.part => Some((at.x - 100.0, -at.y - 100.0, *rotation_deg)),
                _ => None,
            })
            .collect();
        found.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.total_cmp(&b.0)));
        assert_eq!(found.len(), 2, "{name} {}", e.part);
        let mut want_pads = e.pads.to_vec();
        want_pads.sort_by(|a, b| a.0.total_cmp(&b.0));
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (f, w) in found.iter().zip(&want_pads) {
            assert!(
                (f.0 - w.0).abs() < 0.001 && (f.1 - w.1).abs() < 0.001,
                "{name} {}: {f:?} {w:?}",
                e.part
            );
            assert!(
                ((f.2 - e.pad_turn).rem_euclid(360.0))
                    .min(360.0 - (f.2 - e.pad_turn).rem_euclid(360.0))
                    < 0.001
            );
        }
    }
}

#[test]
fn kicad_9_models_land_where_kicad_puts_them() {
    check("model-placements");
}

#[test]
fn kicad_10_models_land_where_kicad_puts_them() {
    check("k10-model-placements");
}

/// Items in a comparable order: KiCad 9 and 10 write the footprints in a different order.
fn sorted<T: std::fmt::Debug>(items: &[T]) -> Vec<String> {
    let mut v: Vec<String> = items.iter().map(|i| format!("{i:?}")).collect();
    v.sort();
    v
}

#[test]
fn kicad_10_boards_read_like_kicad_9_boards() {
    for name in [
        "empty-outline",
        "four-layer",
        "outline-shapes",
        "one-track-arc-via",
        "one-resistor",
        "one-bottom-resistor",
        "one-thru-hole-pads",
        "silkscreen",
        "model-placements",
    ] {
        let read = |n: &str| {
            let path = format!("{}/tests/boards/{n}.kicad_pcb", env!("CARGO_MANIFEST_DIR"));
            KiCad.read(&std::fs::read(path).unwrap()).unwrap().board
        };
        let (a, b) = (read(name), read(&format!("k10-{name}")));
        assert_eq!(a.layers.len(), b.layers.len(), "{name}");
        assert_eq!(a.thickness, b.thickness, "{name}");
        assert_eq!(a.outline, b.outline, "{name}");
        assert_eq!(sorted(&a.copper), sorted(&b.copper), "{name}");
        assert_eq!(a.graphics, b.graphics, "{name}");
        assert_eq!(sorted(&a.components), sorted(&b.components), "{name}");
        // KiCad 10 writes its default pen as 0.1524 where KiCad 9 writes 0.15, so the pen width
        // is left out; everything else about a text is the same.
        let plain = |b: &board_model::Board| {
            b.texts
                .iter()
                .map(|t| {
                    (
                        t.content.clone(),
                        t.at,
                        t.height,
                        t.rotation_deg,
                        t.mirrored,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(sorted(&plain(&a)), sorted(&plain(&b)), "{name}");
    }
}
