//! `read <file> --out <dir> --json`: a board file into a placed-parts package
//! (`dutchman.parts/1`, dutchman-cad/dutchman#1005) that Dutchman writes into SolidCraft.
//!
//! The package is a folder of STEP files plus a small manifest printed on stdout:
//! - `models`: each geometry file once, `{id, file}` (file names relative to the folder),
//! - `parts`: placed instances, `{name, model, transform, notes}`; `name` is the stable
//!   identity (the board itself is `Board`, each component its designator),
//! - `overlays`: reserved for copper and silkscreen, drawn flat on the board later,
//! - `notes`: everything read but not in the package, as `Info`, `Degraded` or `Dropped`.

use std::io::Read;
use std::path::Path;

use board_model::{ModelSource, Note, Placement, Severity, Side, MAX_INPUT_BYTES};
use serde_json::{json, Value};

/// Schema of the package manifest.
pub const PARTS_SCHEMA: &str = "dutchman.parts/1";
/// The board's own part name, stable across conversions.
pub const BOARD_PART: &str = "Board";
/// The board's model id and file.
pub const BOARD_MODEL: &str = "board";
pub const BOARD_FILE: &str = "board.step";

/// A package before it is written: the manifest and its files.
#[derive(Clone, Debug, PartialEq)]
pub struct Package {
    pub manifest: Value,
    /// (file name relative to the package folder, contents)
    pub files: Vec<(String, Vec<u8>)>,
}

/// A note as the manifest carries it.
fn note(n: &Note) -> Value {
    let severity = match n.severity {
        Severity::Dropped => "Dropped",
        Severity::Warning => "Degraded",
        Severity::Info => "Info",
    };
    json!({ "severity": severity, "message": n.message })
}

/// Reads a board file, refusing one bigger than the model's limit before loading it.
pub fn read_input(path: &Path) -> Result<Vec<u8>, String> {
    let shown = path.display();
    let file = std::fs::File::open(path).map_err(|e| format!("cannot read {shown}: {e}"))?;
    let limit = u64::try_from(MAX_INPUT_BYTES).unwrap_or(u64::MAX);
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read {shown}: {e}"))?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!("{shown} is larger than {MAX_INPUT_BYTES} bytes"));
    }
    Ok(bytes)
}

/// Builds the package for a board file's bytes. `path` gives the extension and, when the file
/// names no board, the board's name.
pub fn build(path: &Path, bytes: &[u8]) -> Result<Package, String> {
    let extension = path.extension().and_then(|e| e.to_str());
    let head = bytes.get(..bytes.len().min(4096)).unwrap_or_default();
    let format = crate::detect(head, extension)
        .ok_or_else(|| "not a board file this module reads".to_string())?;
    let outcome = format.read(bytes).map_err(|e| e.to_string())?;
    let board = outcome.board;
    let name = if board.name.trim().is_empty() {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("board")
            .to_string()
    } else {
        board.name.clone()
    };
    let step = board_step::write(&board, &name).map_err(|e| e.to_string())?;

    let mut notes: Vec<Value> = outcome.notes.iter().map(note).collect();
    let later = [
        (
            board.copper.len(),
            "copper item(s) (tracks, pads, vias, pours) are not in the package yet",
        ),
        (
            board.texts.len(),
            "text item(s) (silkscreen, copper text) are not in the package yet",
        ),
    ];
    for (count, what) in later {
        if count > 0 {
            notes.push(json!({ "severity": "Dropped", "message": format!("{count} {what}") }));
        }
    }
    let board_notes: Vec<Value> = step.notes.iter().map(note).collect();
    let mut models = vec![json!({ "id": BOARD_MODEL, "file": BOARD_FILE })];
    let mut parts = vec![json!({
        "name": BOARD_PART,
        "model": BOARD_MODEL,
        "transform": IDENTITY,
        "notes": board_notes,
    })];
    let mut files = vec![(BOARD_FILE.to_string(), step.text.into_bytes())];
    let placed = components(&board.components, step.thickness);
    models.extend(placed.models.iter().map(|(id, file, _)| json!({ "id": id, "file": file })));
    files.extend(placed.models.into_iter().map(|(_, file, text)| (file, text.into_bytes())));
    parts.extend(placed.parts);
    notes.extend(placed.notes);
    let manifest = json!({
        "schema": PARTS_SCHEMA,
        "ok": true,
        "name": name,
        "units": "mm",
        "source": { "format": format.id(), "format_name": format.name() },
        "models": models,
        "parts": parts,
        "overlays": [],
        "notes": notes,
    });
    Ok(Package { manifest, files })
}

/// A part's placement: 3 rows of 4, rotation then translation (mm) in the last column.
type Transform = [[f64; 4]; 3];

const IDENTITY: Transform = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];
/// The most designators one note lists by name.
const MAX_NAMED: usize = 20;

/// The transform that applies `b` first and then `a`.
fn compose(a: &Transform, b: &Transform) -> Transform {
    let mut out = [[0.0; 4]; 3];
    for (row, a_row) in out.iter_mut().zip(a) {
        for (col, cell) in row.iter_mut().enumerate() {
            let along: f64 = a_row
                .iter()
                .zip(b)
                .map(|(x, b_row)| x * b_row.get(col).copied().unwrap_or(0.0))
                .sum();
            let moved = if col == 3 {
                a_row.get(3).copied().unwrap_or(0.0)
            } else {
                0.0
            };
            *cell = along + moved;
        }
    }
    // Rounding noise such as 6e-17 for an exact zero would only make the manifest harder to read.
    for cell in out.iter_mut().flatten() {
        if cell.abs() < 1e-12 {
            *cell = 0.0;
        }
    }
    out
}

fn translation(x: f64, y: f64, z: f64) -> Transform {
    [[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, z]]
}

/// A turn of `deg` degrees about the X (0), Y (1) or Z (2) axis, counter-clockwise looking
/// down the axis.
fn turn(axis: usize, deg: f64) -> Transform {
    let (s, c) = deg.to_radians().sin_cos();
    match axis {
        0 => [[1.0, 0.0, 0.0, 0.0], [0.0, c, -s, 0.0], [0.0, s, c, 0.0]],
        1 => [[c, 0.0, s, 0.0], [0.0, 1.0, 0.0, 0.0], [-s, 0.0, c, 0.0]],
        _ => [[c, -s, 0.0, 0.0], [s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]],
    }
}

/// The component's own frame in board coordinates (see `board_model::Placement`).
fn part_frame(p: &Placement, thickness: f64) -> Transform {
    let (z, flip) = match p.side {
        Side::Top => (thickness, IDENTITY),
        Side::Bottom => (0.0, turn(0, 180.0)),
    };
    let turned = compose(&translation(p.at.x, p.at.y, z), &turn(2, p.rotation_deg));
    compose(&turned, &flip)
}

/// The placed components: model files `(id, file, STEP text)`, parts, and package notes.
#[derive(Default)]
struct Placed {
    models: Vec<(String, String, String)>,
    parts: Vec<Value>,
    notes: Vec<Value>,
}

/// "R1, R2 and 3 more"
fn named(list: &[String]) -> String {
    let shown: Vec<&str> = list.iter().take(MAX_NAMED).map(String::as_str).collect();
    let more = list.len().saturating_sub(MAX_NAMED);
    if more > 0 {
        format!("{} and {more} more", shown.join(", "))
    } else {
        shown.join(", ")
    }
}

/// Each component with a model the package can build becomes a part named by its designator.
fn components(list: &[Placement], thickness: f64) -> Placed {
    let mut placed = Placed::default();
    let mut used = std::collections::BTreeSet::from([BOARD_PART.to_string()]);
    // Equal extruded bodies share one model file.
    let mut extruded: std::collections::BTreeMap<String, String> = Default::default();
    let (mut no_model, mut embedded, mut external, mut failed) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (index, p) in list.iter().enumerate() {
        let base = match p.designator.trim() {
            "" => format!("Part {}", index.saturating_add(1)),
            d => d.to_string(),
        };
        let mut name = base.clone();
        let mut n = 2usize;
        while used.contains(&name) {
            name = format!("{base} ({n})");
            n = n.saturating_add(1);
        }
        used.insert(name.clone());
        let Some(model) = &p.model else {
            no_model.push(name);
            continue;
        };
        let id = match &model.source {
            ModelSource::Embedded { .. } => {
                embedded.push(name);
                continue;
            }
            ModelSource::External { file } => {
                external.push(format!("{name} ({file})"));
                continue;
            }
            ModelSource::Extruded {
                outline,
                standoff,
                height,
            } => {
                let key = format!("{outline:?} {standoff} {height}");
                match extruded.get(&key) {
                    Some(id) => id.clone(),
                    None => {
                        let id = format!("model-{}", placed.models.len().saturating_add(1));
                        match board_step::prism(outline, *standoff, *height, &id) {
                            Ok(step) => {
                                let file = format!("{id}.step");
                                placed.models.push((id.clone(), file, step.text));
                                extruded.insert(key, id.clone());
                                id
                            }
                            Err(why) => {
                                failed.push(format!("{name}: {why}"));
                                continue;
                            }
                        }
                    }
                }
            }
        };
        let [rx, ry, rz] = model.rotation_deg;
        let [dx, dy, dz] = model.offset;
        let on_part = compose(
            &translation(dx, dy, dz),
            &compose(&turn(2, rz), &compose(&turn(1, ry), &turn(0, rx))),
        );
        let mut part_notes = Vec::new();
        if rx != 0.0 || ry != 0.0 {
            part_notes.push(json!({
                "severity": "Degraded",
                "message": "the model is tilted about X or Y on its part; no test board confirms \
                            that turn yet",
            }));
        }
        placed.parts.push(json!({
            "name": name,
            "model": id,
            "transform": compose(&part_frame(p, thickness), &on_part),
            "notes": part_notes,
        }));
    }
    for (list, what) in [
        (no_model, "component(s) have no 3D model and are not in the package"),
        (
            embedded,
            "component(s) have a 3D model stored in the board, which is not unpacked yet",
        ),
        (
            external,
            "component(s) refer to a 3D model file outside the board and are not in the package",
        ),
        (failed, "component(s) have an extruded body that could not be built"),
    ] {
        if !list.is_empty() {
            placed.notes.push(json!({
                "severity": "Dropped",
                "message": format!("{} {what}: {}", list.len(), named(&list)),
            }));
        }
    }
    placed
}

/// Writes the package's files into `dir` (made if missing). Each file is written beside its
/// final name first and then renamed, so a reader never sees half a file.
pub fn write(dir: &Path, package: &Package) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    for (name, contents) in &package.files {
        let plain = Path::new(name)
            .file_name()
            .is_some_and(|f| f == name.as_str());
        if !plain {
            return Err(format!(
                "refusing a package file name with a folder in it: {name}"
            ));
        }
        let target = dir.join(name);
        let partial = dir.join(format!(".{name}.partial"));
        std::fs::write(&partial, contents)
            .and_then(|()| std::fs::rename(&partial, &target))
            .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
    }
    Ok(())
}

/// The failure reply.
pub fn error(message: &str) -> Value {
    crate::module::error(PARTS_SCHEMA, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_refuses_file_names_with_folders() {
        let dir = std::env::temp_dir().join(format!("de-pkg-{}", std::process::id()));
        let bad = Package {
            manifest: Value::Null,
            files: vec![("../escape.step".into(), b"x".to_vec())],
        };
        let result = write(&dir, &bad);
        std::fs::remove_dir_all(&dir).ok();
        assert!(result.is_err());
    }

    #[test]
    fn write_puts_files_in_the_folder() {
        let dir = std::env::temp_dir().join(format!("de-pkg-ok-{}", std::process::id()));
        let good = Package {
            manifest: Value::Null,
            files: vec![("board.step".into(), b"ISO".to_vec())],
        };
        let result = write(&dir, &good);
        let written = std::fs::read(dir.join("board.step"));
        let leftovers = std::fs::read_dir(&dir).map(|d| d.count());
        std::fs::remove_dir_all(&dir).ok();
        assert!(result.is_ok());
        assert_eq!(written.unwrap(), b"ISO");
        assert_eq!(leftovers.unwrap(), 1, "no partial file left behind");
    }

    #[test]
    fn notes_use_dutchmans_severities() {
        let n = |severity| Note {
            severity,
            message: "m".into(),
        };
        assert_eq!(note(&n(Severity::Dropped))["severity"], "Dropped");
        assert_eq!(note(&n(Severity::Warning))["severity"], "Degraded");
        assert_eq!(note(&n(Severity::Info))["severity"], "Info");
    }

    use board_model::{Contour, ModelRef, Point, Segment};

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Contour {
        let c = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        let next = c.iter().cycle().skip(1);
        Contour {
            segments: c
                .iter()
                .zip(next)
                .map(|(a, b)| Segment::Line {
                    from: Point { x: a.0, y: a.1 },
                    to: Point { x: b.0, y: b.1 },
                })
                .collect(),
        }
    }

    fn part(designator: &str, x: f64, y: f64, rotation_deg: f64, side: Side) -> Placement {
        Placement {
            designator: designator.into(),
            footprint: "0603".into(),
            at: Point { x, y },
            rotation_deg,
            side,
            model: Some(ModelRef {
                name: String::new(),
                source: ModelSource::Extruded {
                    outline: rect(-0.8, -0.4, 0.8, 0.4),
                    standoff: 0.0,
                    height: 0.45,
                },
                offset: [0.0; 3],
                rotation_deg: [0.0; 3],
            }),
        }
    }

    /// Where a point of the model lands on the board.
    fn apply(t: &Value, p: [f64; 3]) -> [f64; 3] {
        let m: Transform = serde_json::from_value(t.clone()).unwrap();
        let mut out = [0.0; 3];
        for (o, row) in out.iter_mut().zip(m) {
            *o = row[0] * p[0] + row[1] * p[1] + row[2] * p[2] + row[3];
        }
        out
    }

    /// Equal to 1e-5 mm: Altium stores lengths in 1/10000 mil, so 25 mm reads as 25.0000008.
    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn a_top_side_part_sits_on_the_top_face() {
        let placed = components(&[part("R1", 25.0, 15.0, 0.0, Side::Top)], 1.6);
        assert_eq!(placed.models.len(), 1);
        assert!(placed.notes.is_empty(), "{:?}", placed.notes);
        let p = &placed.parts[0];
        assert_eq!(p["name"], "R1");
        assert_eq!(p["model"], "model-1");
        assert_eq!(
            p["transform"],
            json!([[1.0, 0.0, 0.0, 25.0], [0.0, 1.0, 0.0, 15.0], [0.0, 0.0, 1.0, 1.6]])
        );
        // one-resistor's README: the box spans x 24.2 to 25.8, y 14.6 to 15.4, 0.45 high.
        assert!(close(apply(&p["transform"], [0.8, 0.4, 0.45]), [25.8, 15.4, 2.05]));
    }

    #[test]
    fn a_turned_part_turns_its_model() {
        let placed = components(&[part("R1", 25.0, 15.0, 90.0, Side::Top)], 1.6);
        let t = &placed.parts[0]["transform"];
        // one-rotated-step-resistor's README: the +X end of the model turned onto +Y.
        assert!(close(apply(t, [1.6, 0.0, 0.0]), [25.0, 16.6, 1.6]));
    }

    #[test]
    fn a_bottom_side_part_is_flipped_onto_the_bottom_face() {
        // one-bottom-step-resistor's README: R1 flipped is at (25.4, 15) with rotation 180, and
        // the off-centre box (x 0 to 1.6 in the model) spans x 23.8 to 25.4, projecting down.
        let placed = components(&[part("R1", 25.4, 15.0, 180.0, Side::Bottom)], 1.6);
        let t = &placed.parts[0]["transform"];
        assert!(close(apply(t, [0.0, 0.0, 0.0]), [25.4, 15.0, 0.0]));
        assert!(close(apply(t, [1.6, 0.0, 0.0]), [23.8, 15.0, 0.0]));
        assert!(close(apply(t, [0.0, 0.0, 0.45]), [25.4, 15.0, -0.45]));
        // Half a turn about X in the part's frame: its +Y is the board's -Y before the turn by
        // 180, so +Y after it.
        assert!(close(apply(t, [0.0, 0.4, 0.0]), [25.4, 15.4, 0.0]));
    }

    #[test]
    fn a_models_own_offset_and_turn_come_before_the_part() {
        let mut r1 = part("R1", 10.0, 0.0, 90.0, Side::Top);
        if let Some(m) = &mut r1.model {
            m.offset = [1.0, 0.0, 0.2];
            m.rotation_deg = [0.0, 0.0, 90.0];
        }
        let placed = components(&[r1], 1.6);
        let t = &placed.parts[0]["transform"];
        // Model +X: turned 90 on the part (+Y), moved by the offset to (1, 1), then the part
        // turns 90: (-1, 1), at (10, 0) on the top face, 0.2 up.
        assert!(close(apply(t, [1.0, 0.0, 0.0]), [9.0, 1.0, 1.8]));
        assert!(placed.parts[0]["notes"].as_array().unwrap().is_empty());
    }

    #[test]
    fn equal_bodies_share_one_model_and_names_stay_unique() {
        let list = [
            part("R1", 10.0, 10.0, 0.0, Side::Top),
            part("R1", 20.0, 10.0, 0.0, Side::Top),
            part("Board", 30.0, 10.0, 0.0, Side::Top),
            part(" ", 40.0, 10.0, 0.0, Side::Top),
        ];
        let placed = components(&list, 1.6);
        assert_eq!(placed.models.len(), 1);
        let names: Vec<&str> = placed.parts.iter().map(|p| p["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["R1", "R1 (2)", "Board (2)", "Part 4"]);
        assert!(placed.parts.iter().all(|p| p["model"] == "model-1"));
    }

    #[test]
    fn parts_without_a_buildable_model_are_named_in_a_note() {
        let mut none = part("R1", 0.0, 0.0, 0.0, Side::Top);
        none.model = None;
        let mut embedded = part("U1", 0.0, 0.0, 0.0, Side::Top);
        let mut external = part("U2", 0.0, 0.0, 0.0, Side::Top);
        let mut flat = part("U3", 0.0, 0.0, 0.0, Side::Top);
        if let (Some(e), Some(x), Some(f)) =
            (&mut embedded.model, &mut external.model, &mut flat.model)
        {
            e.source = ModelSource::Embedded {
                stream: "/Models/0".into(),
            };
            x.source = ModelSource::External {
                file: "soic8.step".into(),
            };
            f.source = ModelSource::Extruded {
                outline: rect(0.0, 0.0, 1.0, 1.0),
                standoff: 0.5,
                height: 0.5,
            };
        }
        let placed = components(&[none, embedded, external, flat], 1.6);
        assert!(placed.parts.is_empty());
        let messages: Vec<&str> = placed
            .notes
            .iter()
            .map(|n| {
                assert_eq!(n["severity"], "Dropped");
                n["message"].as_str().unwrap()
            })
            .collect();
        assert_eq!(messages.len(), 4, "{messages:?}");
        assert!(messages[0].ends_with("no 3D model and are not in the package: R1"));
        assert!(messages[1].ends_with("not unpacked yet: U1"));
        assert!(messages[2].ends_with("outside the board and are not in the package: U2 (soic8.step)"));
        assert!(messages[3].contains("U3: cannot make the board solid"));
    }

    #[cfg(feature = "altium")]
    #[test]
    fn the_one_resistor_board_gives_r1_as_a_part() {
        let bytes = include_bytes!("../../altium-pcbdoc/tests/boards/one-resistor.PcbDoc");
        let package = build(Path::new("one-resistor.PcbDoc"), bytes).unwrap();
        let m = &package.manifest;
        let names: Vec<&str> = m["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["Board", "R1"]);
        let r1 = &m["parts"][1];
        assert!(close(apply(&r1["transform"], [0.0; 3]), [25.0, 15.0, 1.6]));
        let files: Vec<&str> = package.files.iter().map(|(f, _)| f.as_str()).collect();
        assert_eq!(files, ["board.step", "model-1.step"]);
        assert_eq!(m["models"][1], json!({ "id": "model-1", "file": "model-1.step" }));
    }

    #[cfg(feature = "altium")]
    #[test]
    fn the_one_bottom_resistor_board_hangs_r1_below_the_board() {
        let bytes = include_bytes!("../../altium-pcbdoc/tests/boards/one-bottom-resistor.PcbDoc");
        let package = build(Path::new("one-bottom-resistor.PcbDoc"), bytes).unwrap();
        let r1 = &package.manifest["parts"][1];
        assert_eq!(r1["name"], "R1");
        // The README: the box spans x 24.2 to 25.8, y 14.6 to 15.4, 0.45 below the bottom face.
        let t = &r1["transform"];
        let corners = [[-0.8, -0.4, 0.0], [0.8, 0.4, 0.45]].map(|p| apply(t, p));
        let (lo, hi) = (corners[0], corners[1]);
        let span = |k: usize| (lo[k].min(hi[k]), lo[k].max(hi[k]));
        assert!(close([span(0).0, span(1).0, span(2).0], [24.2, 14.6, -0.45]));
        assert!(close([span(0).1, span(1).1, span(2).1], [25.8, 15.4, 0.0]));
    }

    #[test]
    fn a_file_that_is_not_a_board_is_an_error() {
        assert!(build(Path::new("notes.txt"), b"hello").is_err());
    }
}
