use std::collections::HashMap;

use board_model::{Board, Contour, CopperItem, Outline, PadShape, Point, Segment, Severity};

use super::*;

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn polyline(points: &[(f64, f64)]) -> Contour {
    let segments = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| Segment::Line {
            from: p(a.0, a.1),
            to: p(b.0, b.1),
        })
        .collect();
    Contour { segments }
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Contour {
    polyline(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
}

fn board(outer: Contour, cutouts: Vec<Contour>, copper: Vec<CopperItem>) -> Board {
    Board {
        name: "test".into(),
        thickness: Some(1.6),
        outline: Outline { outer, cutouts },
        copper,
        ..Board::default()
    }
}

fn hole(x: f64, y: f64, drill: f64) -> CopperItem {
    CopperItem::Pad {
        layer: 0,
        at: p(x, y),
        shape: PadShape::Round,
        size_x: drill,
        size_y: drill,
        rotation_deg: 0.0,
        drill: Some(drill),
        owner: None,
    }
}

/// The entities of a STEP file by number: (name, arguments).
fn entities(text: &str) -> HashMap<usize, (String, String)> {
    let mut out = HashMap::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix('#') else {
            continue;
        };
        let (id, body) = rest.split_once('=').unwrap();
        let body = body.trim_end_matches(';');
        let (name, args) = body.split_once('(').unwrap();
        out.insert(id.parse().unwrap(), (name.to_string(), args.to_string()));
    }
    out
}

fn refs(args: &str) -> Vec<usize> {
    args.split(|c: char| !(c.is_ascii_digit() || c == '#'))
        .filter_map(|t| t.strip_prefix('#'))
        .filter_map(|t| t.parse().ok())
        .collect()
}

/// Checks the solid's topology from the STEP text alone: every edge is used exactly twice, in
/// opposite directions; every loop is closed head to tail; V - E + F = 2.
fn check_topology(text: &str) {
    let all = entities(text);
    let of = |name: &'static str| all.iter().filter(move |(_, (n, _))| n == name);
    let vertices = of("VERTEX_POINT").count();
    let edges: HashMap<usize, (usize, usize)> = of("EDGE_CURVE")
        .map(|(id, (_, args))| {
            let r = refs(args);
            (*id, (r[0], r[1]))
        })
        .collect();
    let faces = of("ADVANCED_FACE").count();
    let oriented: HashMap<usize, (usize, bool)> = of("ORIENTED_EDGE")
        .map(|(id, (_, args))| (*id, (refs(args)[0], args.ends_with(".T.)"))))
        .collect();

    let mut uses: HashMap<usize, (usize, usize)> = HashMap::new();
    for (edge, forward) in oriented.values() {
        let u = uses.entry(*edge).or_default();
        if *forward {
            u.0 += 1;
        } else {
            u.1 += 1;
        }
    }
    assert_eq!(uses.len(), edges.len(), "every edge is used");
    for (edge, u) in &uses {
        assert_eq!(*u, (1, 1), "edge #{edge} must be used once each way");
    }

    for (id, (_, args)) in of("EDGE_LOOP") {
        let chain: Vec<(usize, usize)> = refs(args)
            .iter()
            .map(|o| {
                let (edge, forward) = oriented[o];
                let (a, b) = edges[&edge];
                if forward {
                    (a, b)
                } else {
                    (b, a)
                }
            })
            .collect();
        for (k, step) in chain.iter().enumerate() {
            let next = chain[(k + 1) % chain.len()];
            assert_eq!(step.1, next.0, "loop #{id} is not closed head to tail");
        }
    }

    let (v, e, f) = (vertices as i64, edges.len() as i64, faces as i64);
    assert_eq!(v - e + f, 2, "Euler: V={v} E={e} F={f}");
}

#[test]
fn a_plain_rectangle_is_a_box() {
    let out = write(
        &board(rect(0.0, 0.0, 50.0, 30.0), vec![], vec![]),
        "empty-outline",
    )
    .unwrap();
    assert_eq!(
        out.stats,
        Stats {
            vertices: 8,
            edges: 12,
            faces: 6,
            openings: 0
        }
    );
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    assert!(out.text.starts_with("ISO-10303-21;"));
    assert!(out.text.ends_with("END-ISO-10303-21;\n"));
    assert!(out.text.contains("MANIFOLD_SOLID_BREP('empty-outline',"));
    assert!(out.text.contains("CARTESIAN_POINT('',(50.,30.,1.6))"));
    assert!(out.text.contains("SI_UNIT(.MILLI.,.METRE.)"));
    check_topology(&out.text);
}

#[test]
fn a_hole_and_a_cutout_go_through() {
    let b = board(
        rect(0.0, 0.0, 50.0, 30.0),
        vec![rect(30.0, 10.0, 40.0, 20.0)],
        vec![hole(10.0, 15.0, 3.0)],
    );
    let out = write(&b, "outline-with-hole").unwrap();
    assert_eq!(out.stats.openings, 2);
    // Box (8, 12, 6) + square cutout (8, 12, 4) + round hole as two half cylinders (4, 6, 2).
    assert_eq!(
        (out.stats.vertices, out.stats.edges, out.stats.faces),
        (20, 30, 12)
    );
    assert_eq!(out.text.matches("CYLINDRICAL_SURFACE(").count(), 2);
    assert!(out.text.contains("CYLINDRICAL_SURFACE('',#"));
    assert!(out.text.contains(",1.5)"), "radius 1.5");
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    check_topology(&out.text);
}

#[test]
fn a_clockwise_outline_gives_the_same_solid() {
    let mut cw = rect(0.0, 0.0, 50.0, 30.0);
    cw.segments.reverse();
    for s in &mut cw.segments {
        if let Segment::Line { from, to } = s {
            std::mem::swap(from, to);
        }
    }
    let out = write(&board(cw, vec![], vec![hole(25.0, 15.0, 2.0)]), "cw").unwrap();
    assert_eq!(out.stats.faces, 8);
    check_topology(&out.text);
}

#[test]
fn arcs_become_cylinders() {
    // A 40 x 20 slot: two straight sides and two half-circle ends.
    let outer = Contour {
        segments: vec![
            Segment::Line {
                from: p(10.0, 0.0),
                to: p(30.0, 0.0),
            },
            Segment::Arc {
                center: p(30.0, 10.0),
                radius: 10.0,
                start_deg: -90.0,
                end_deg: 90.0,
            },
            Segment::Line {
                from: p(30.0, 20.0),
                to: p(10.0, 20.0),
            },
            Segment::Arc {
                center: p(10.0, 10.0),
                radius: 10.0,
                start_deg: 90.0,
                end_deg: 270.0,
            },
        ],
    };
    let out = write(&board(outer, vec![], vec![]), "slot").unwrap();
    assert_eq!(out.stats.faces, 6);
    assert_eq!(out.text.matches("CYLINDRICAL_SURFACE(").count(), 2);
    check_topology(&out.text);
}

#[test]
fn a_round_board_from_one_full_circle() {
    let outer = Contour {
        segments: vec![Segment::Arc {
            center: p(0.0, 0.0),
            radius: 25.0,
            start_deg: 0.0,
            end_deg: 360.0,
        }],
    };
    let out = write(&board(outer, vec![], vec![]), "round").unwrap();
    assert_eq!(
        (out.stats.vertices, out.stats.edges, out.stats.faces),
        (4, 6, 4)
    );
    check_topology(&out.text);
}

#[test]
fn holes_that_cannot_be_cut_are_reported() {
    let b = board(
        rect(0.0, 0.0, 50.0, 30.0),
        vec![rect(30.0, 10.0, 40.0, 20.0)],
        vec![
            hole(10.0, 15.0, 3.0),
            hole(11.0, 15.0, 3.0),  // overlaps the first
            hole(0.5, 15.0, 3.0),   // crosses the board edge
            hole(35.0, 15.0, 2.0),  // inside the cutout
            hole(90.0, 90.0, 1.0),  // off the board
            hole(20.0, 15.0, -1.0), // unusable size
        ],
    );
    let out = write(&b, "messy").unwrap();
    assert_eq!(out.stats.openings, 2, "the cutout and the first hole");
    let messages: Vec<&str> = out.notes.iter().map(|n| n.message.as_str()).collect();
    assert!(out.notes.iter().all(|n| n.severity == Severity::Dropped));
    assert!(messages
        .iter()
        .any(|m| m.starts_with("2 drilled hole(s) touch or cross the board edge")));
    assert!(messages
        .iter()
        .any(|m| m.starts_with("1 drilled hole(s) touch or fall inside a cutout")));
    assert!(messages
        .iter()
        .any(|m| m.starts_with("1 drilled hole(s) overlap another hole")));
    assert!(messages
        .iter()
        .any(|m| m.starts_with("1 drilled hole(s) with an unusable size")));
    check_topology(&out.text);
}

#[test]
fn vias_are_drilled_too() {
    let via = CopperItem::Via {
        at: p(5.0, 5.0),
        diameter: 0.6,
        drill: 0.3,
    };
    let out = write(&board(rect(0.0, 0.0, 50.0, 30.0), vec![], vec![via]), "via").unwrap();
    assert_eq!(out.stats.openings, 1);
    check_topology(&out.text);
}

#[test]
fn a_bad_cutout_is_dropped_not_fatal() {
    let open = Contour {
        segments: vec![Segment::Line {
            from: p(5.0, 5.0),
            to: p(10.0, 5.0),
        }],
    };
    let crossing = rect(45.0, 10.0, 55.0, 20.0);
    let b = board(rect(0.0, 0.0, 50.0, 30.0), vec![open, crossing], vec![]);
    let out = write(&b, "bad-cutouts").unwrap();
    assert_eq!(out.stats.openings, 0);
    assert_eq!(out.notes.len(), 2, "{:?}", out.notes);
    check_topology(&out.text);
}

#[test]
fn outline_problems_are_errors() {
    let empty = Board::default();
    assert!(matches!(write(&empty, "x"), Err(StepError::Outline(_))));

    let gap = Contour {
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
                to: p(0.0, 5.0),
            },
        ],
    };
    let err = write(&board(gap, vec![], vec![]), "gap").unwrap_err();
    assert!(err.to_string().contains("not closed"), "{err}");

    let nan = polyline(&[(0.0, 0.0), (f64::NAN, 0.0), (1.0, 1.0)]);
    assert!(write(&board(nan, vec![], vec![]), "nan").is_err());

    let mut flat = board(rect(0.0, 0.0, 10.0, 10.0), vec![], vec![]);
    flat.thickness = Some(f64::NAN);
    assert!(write(&flat, "flat").is_err());
}

#[test]
fn unknown_thickness_uses_the_default_with_a_warning() {
    let mut b = board(rect(0.0, 0.0, 10.0, 10.0), vec![], vec![]);
    b.thickness = None;
    let out = write(&b, "no-thickness").unwrap();
    assert_eq!(out.notes.len(), 1);
    assert_eq!(out.notes[0].severity, Severity::Warning);
    assert!(out.text.contains("CARTESIAN_POINT('',(10.,10.,1.6))"));
}

#[test]
fn the_same_board_gives_the_same_file() {
    let b = board(
        rect(0.0, 0.0, 50.0, 30.0),
        vec![rect(30.0, 10.0, 40.0, 20.0)],
        vec![hole(10.0, 15.0, 3.0)],
    );
    assert_eq!(
        write(&b, "same").unwrap().text,
        write(&b, "same").unwrap().text
    );
}

#[test]
fn step_reals_and_strings() {
    assert_eq!(real(50.0), "50.");
    assert_eq!(real(-0.0), "0.");
    assert_eq!(real(1e-13), "0.");
    assert_eq!(real(1.6), "1.6");
    assert_eq!(real(-2.5), "-2.5");
    assert_eq!(text("it's"), "'it''s'");
    assert_eq!(text("Ü-1"), "'_-1'");
}
