//! `read <file> --out <dir> --json`: a board file into a placed-parts package
//! (`dutchman.parts/1`, dutchman-cad/dutchman#1005) that Dutchman writes into SolidCraft.
//!
//! The package is a folder of STEP files plus a small manifest printed on stdout:
//! - `models`: each geometry file once, `{id, file}` (file names relative to the folder),
//! - `parts`: placed instances, `{name, model, transform, notes}`; `name` is the stable
//!   identity (the board itself is `Board`, each component its designator),
//! - `overlays`: copper, silkscreen and solder mask as flat areas on the board's faces (see
//!   `overlay`), each with a colour,
//! - a `Plating` part when the board has plated holes: the copper lining them, one tube each, all
//!   in one STEP file,
//! - `notes`: everything read but not in the package, as `Info`, `Degraded` or `Dropped`.

use std::io::Read;
use std::path::Path;

use board_model::{
    Board, CopperItem, EmbeddedModel, ModelSource, Note, Placement, Severity, Side, MAX_INPUT_BYTES,
};
use serde_json::{json, Value};

use crate::colors::{BOARD_COLOR, PLATING_COLOR};

/// Schema of the package manifest.
pub const PARTS_SCHEMA: &str = "dutchman.parts/1";
/// The board's own part name, stable across conversions.
pub const BOARD_PART: &str = "Board";
/// The board's model id and file.
pub const BOARD_MODEL: &str = "board";
/// The manifest's file in the package folder (the same JSON the command prints).
pub const MANIFEST_FILE: &str = "manifest.json";
pub const BOARD_FILE: &str = "board.step";
/// The plating's part name, reserved like [`BOARD_PART`]: no component takes it.
pub const PLATING_PART: &str = "Plating";
/// The plating's model id and file.
pub const PLATING_MODEL: &str = "plating";
pub const PLATING_FILE: &str = "plating.step";
/// How thick the copper lining a plated hole is, in mm (a common 25 micrometres). Board files do
/// not give it, so it is assumed and a note says so.
pub const PLATING_THICKNESS: f64 = 0.025;

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
    let board_notes: Vec<Value> = step.notes.iter().map(note).collect();
    let mut models = vec![json!({ "id": BOARD_MODEL, "file": BOARD_FILE })];
    let mut parts = vec![json!({
        "name": BOARD_PART,
        "model": BOARD_MODEL,
        "transform": IDENTITY,
        "color": BOARD_COLOR,
        "notes": board_notes,
    })];
    let mut files = vec![(BOARD_FILE.to_string(), step.text.into_bytes())];
    let placed = components(&board.components, &outcome.models, step.thickness);
    models.extend(
        placed
            .models
            .iter()
            .map(|(id, file, _)| json!({ "id": id, "file": file })),
    );
    files.extend(
        placed
            .models
            .into_iter()
            .map(|(_, file, bytes)| (file, bytes)),
    );
    parts.extend(placed.parts);
    notes.extend(placed.notes);
    let plated = plating(&board, &step.holes, step.thickness);
    notes.extend(plated.notes);
    if let Some(text) = plated.text {
        models.push(json!({ "id": PLATING_MODEL, "file": PLATING_FILE }));
        parts.push(json!({
            "name": PLATING_PART,
            "model": PLATING_MODEL,
            "transform": IDENTITY,
            "color": PLATING_COLOR,
            "notes": [],
        }));
        files.push((PLATING_FILE.to_string(), text.into_bytes()));
    }
    let (overlays, overlay_notes) = crate::overlay::overlays(&board, step.thickness, BOARD_PART);
    notes.extend(overlay_notes);
    let manifest = json!({
        "schema": PARTS_SCHEMA,
        "ok": true,
        "name": name,
        "units": "mm",
        "source": { "format": format.id(), "format_name": format.name() },
        "models": models,
        "parts": parts,
        "overlays": overlays,
        "notes": notes,
    });
    Ok(Package { manifest, files })
}

/// The plating's STEP file (when there is any) and the notes on it.
struct Plating {
    text: Option<String>,
    notes: Vec<Value>,
}

/// The copper lining the plated holes: one tube for each, from the board's bottom (z 0) to its top
/// (z = `thickness`), in the Board's frame. Plated holes are every via, and every pad whose drill
/// the board model marks `plated`, but only those the board solid was actually cut with (`cut`,
/// from `board_step::write`): a tube in a hole that was not cut would float in the air or run
/// through the substrate. A tube's outer radius is the drill radius, its inner one that less
/// [`PLATING_THICKNESS`]. No text when no hole is plated, or when the tubes cannot be written;
/// a hole not cut, too small to line, or past the cap, or a failure, says so in a note.
fn plating(board: &Board, cut: &[board_step::CutHole], thickness: f64) -> Plating {
    let wanted = board
        .copper
        .iter()
        .filter(|item| match item {
            CopperItem::Via { .. } => true,
            CopperItem::Pad { drill, plated, .. } => drill.is_some() && *plated,
            _ => false,
        })
        .count();
    let plated_cut: Vec<&board_step::CutHole> = cut.iter().filter(|h| h.plated).collect();
    let not_cut = wanted.saturating_sub(plated_cut.len());
    let mut list = Vec::new();
    let mut left_out = 0usize;
    for hole in plated_cut {
        let inner_radius = hole.radius - PLATING_THICKNESS;
        if inner_radius <= board_step::TOLERANCE || list.len() >= board_step::MAX_TUBES {
            left_out = left_out.saturating_add(1);
            continue;
        }
        list.push(board_step::Tube {
            at: hole.at,
            outer_radius: hole.radius,
            inner_radius,
        });
    }
    let mut notes = Vec::new();
    if not_cut > 0 {
        notes.push(json!({
            "severity": "Dropped",
            "message": format!(
                "{not_cut} plated hole(s) were not cut into the board, so have no plating"
            ),
        }));
    }
    if left_out > 0 {
        notes.push(json!({
            "severity": "Dropped",
            "message": format!(
                "{left_out} plated hole(s) are too small to line with {PLATING_THICKNESS} mm of \
                 plating, or past the first {} tubes, and have no plating",
                board_step::MAX_TUBES
            ),
        }));
    }
    if list.is_empty() {
        return Plating { text: None, notes };
    }
    match board_step::tubes(&list, 0.0, thickness, PLATING_PART) {
        Ok(step) => {
            notes.push(json!({
                "severity": "Info",
                "message": format!(
                    "the plating inside each plated hole (every via, and every pad with a \
                     plated drill) is assumed {PLATING_THICKNESS} mm thick: the board file does \
                     not give it"
                ),
            }));
            Plating {
                text: Some(step.text),
                notes,
            }
        }
        Err(why) => {
            notes.push(json!({
                "severity": "Dropped",
                "message": format!("the plating of plated holes could not be written: {why}"),
            }));
            Plating { text: None, notes }
        }
    }
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

/// The placed components: model files `(id, file, STEP file)`, parts, and package notes.
#[derive(Default)]
struct Placed {
    models: Vec<(String, String, Vec<u8>)>,
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
fn components(list: &[Placement], embedded: &[EmbeddedModel], thickness: f64) -> Placed {
    let mut placed = Placed::default();
    let mut used =
        std::collections::BTreeSet::from([BOARD_PART.to_string(), PLATING_PART.to_string()]);
    // Equal extruded bodies share one model file; so do parts naming the same embedded model.
    let mut extruded: std::collections::BTreeMap<String, String> = Default::default();
    let mut unpacked: std::collections::BTreeMap<&str, String> = Default::default();
    let (mut no_model, mut missing, mut external, mut failed) =
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
            ModelSource::Embedded { id } => match unpacked.get(id.as_str()) {
                Some(model_id) => model_id.clone(),
                None => match embedded.iter().find(|m| &m.id == id) {
                    Some(m) if m.step.starts_with(b"ISO-10303-21") => {
                        let model_id = format!("model-{}", placed.models.len().saturating_add(1));
                        let file = format!("{model_id}.step");
                        placed.models.push((model_id.clone(), file, m.step.clone()));
                        unpacked.insert(m.id.as_str(), model_id.clone());
                        model_id
                    }
                    Some(m) => {
                        failed.push(format!("{name}: {} is not a STEP file", m.name));
                        continue;
                    }
                    None => {
                        missing.push(name);
                        continue;
                    }
                },
            },
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
                                placed
                                    .models
                                    .push((id.clone(), file, step.text.into_bytes()));
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
        // A tilt about X is confirmed on top- and bottom-side parts by the tenth batch of test
        // boards; a tilt about Y is on none of them yet.
        if ry != 0.0 {
            part_notes.push(json!({
                "severity": "Degraded",
                "message": "the model is tilted about Y on its part; no test board confirms \
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
        (
            no_model,
            "component(s) have no 3D model and are not in the package",
        ),
        (
            missing,
            "component(s) name a 3D model the board does not hold",
        ),
        (
            external,
            "component(s) refer to a 3D model file outside the board and are not in the package",
        ),
        (
            failed,
            "component(s) have a 3D model that could not be used",
        ),
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
    let put = |name: &str, contents: &[u8]| {
        let target = dir.join(name);
        let partial = dir.join(format!(".{name}.partial"));
        std::fs::write(&partial, contents)
            .and_then(|()| std::fs::rename(&partial, &target))
            .map_err(|e| format!("cannot write {}: {e}", target.display()))
    };
    for (name, contents) in &package.files {
        let plain = Path::new(name)
            .file_name()
            .is_some_and(|f| f == name.as_str());
        if !plain || name == MANIFEST_FILE {
            return Err(format!("refusing a package file named {name}"));
        }
        put(name, contents)?;
    }
    // Last, so a folder that holds a manifest holds every file it names.
    let manifest = serde_json::to_vec_pretty(&package.manifest).map_err(|e| e.to_string())?;
    put(MANIFEST_FILE, &manifest)
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
        let clash = Package {
            manifest: Value::Null,
            files: vec![(MANIFEST_FILE.into(), b"x".to_vec())],
        };
        let clashed = write(&dir, &clash);
        std::fs::remove_dir_all(&dir).ok();
        assert!(result.is_err());
        assert!(
            clashed.is_err(),
            "a model file may not take the manifest's name"
        );
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
        let manifest = std::fs::read(dir.join(MANIFEST_FILE));
        let leftovers = std::fs::read_dir(&dir).map(|d| d.count());
        std::fs::remove_dir_all(&dir).ok();
        assert!(result.is_ok());
        assert_eq!(written.unwrap(), b"ISO");
        assert_eq!(manifest.unwrap(), b"null");
        assert_eq!(leftovers.unwrap(), 2, "no partial file left behind");
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
        let placed = components(&[part("R1", 25.0, 15.0, 0.0, Side::Top)], &[], 1.6);
        assert_eq!(placed.models.len(), 1);
        assert!(placed.notes.is_empty(), "{:?}", placed.notes);
        let p = &placed.parts[0];
        assert_eq!(p["name"], "R1");
        assert_eq!(p["model"], "model-1");
        assert_eq!(
            p["transform"],
            json!([
                [1.0, 0.0, 0.0, 25.0],
                [0.0, 1.0, 0.0, 15.0],
                [0.0, 0.0, 1.0, 1.6]
            ])
        );
        // one-resistor's README: the box spans x 24.2 to 25.8, y 14.6 to 15.4, 0.45 high.
        assert!(close(
            apply(&p["transform"], [0.8, 0.4, 0.45]),
            [25.8, 15.4, 2.05]
        ));
    }

    #[test]
    fn a_turned_part_turns_its_model() {
        let placed = components(&[part("R1", 25.0, 15.0, 90.0, Side::Top)], &[], 1.6);
        let t = &placed.parts[0]["transform"];
        // one-rotated-step-resistor's README: the +X end of the model turned onto +Y.
        assert!(close(apply(t, [1.6, 0.0, 0.0]), [25.0, 16.6, 1.6]));
    }

    #[test]
    fn a_bottom_side_part_is_flipped_onto_the_bottom_face() {
        // one-bottom-step-resistor's README: R1 flipped is at (25.4, 15) with rotation 180, and
        // the off-centre box (x 0 to 1.6 in the model) spans x 23.8 to 25.4, projecting down.
        let placed = components(&[part("R1", 25.4, 15.0, 180.0, Side::Bottom)], &[], 1.6);
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
        let placed = components(&[r1], &[], 1.6);
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
        let placed = components(&list, &[], 1.6);
        assert_eq!(placed.models.len(), 1);
        let names: Vec<&str> = placed
            .parts
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
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
            e.source = ModelSource::Embedded { id: "{M}".into() };
            x.source = ModelSource::External {
                file: "soic8.step".into(),
            };
            f.source = ModelSource::Extruded {
                outline: rect(0.0, 0.0, 1.0, 1.0),
                standoff: 0.5,
                height: 0.5,
            };
        }
        let placed = components(&[none, embedded, external, flat], &[], 1.6);
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
        assert!(messages[1].ends_with("name a 3D model the board does not hold: U1"));
        assert!(
            messages[2].ends_with("outside the board and are not in the package: U2 (soic8.step)")
        );
        assert!(messages[3].contains("U3: cannot make the board solid"));
    }

    #[test]
    fn an_embedded_model_is_written_once_for_every_part_that_uses_it() {
        let mut r1 = part("R1", 10.0, 0.0, 0.0, Side::Top);
        let mut r2 = part("R2", 20.0, 0.0, 0.0, Side::Top);
        let mut r3 = part("R3", 30.0, 0.0, 0.0, Side::Top);
        for (p, id) in [(&mut r1, "A"), (&mut r2, "A"), (&mut r3, "B")] {
            if let Some(m) = &mut p.model {
                m.source = ModelSource::Embedded { id: id.into() };
            }
        }
        let models = [
            EmbeddedModel {
                id: "A".into(),
                name: "box.step".into(),
                step: b"ISO-10303-21;\nHEADER;".to_vec(),
            },
            EmbeddedModel {
                id: "B".into(),
                name: "notes.txt".into(),
                step: b"hello".to_vec(),
            },
        ];
        let placed = components(&[r1, r2, r3], &models, 1.6);
        assert_eq!(placed.models.len(), 1);
        assert_eq!(placed.models[0].2, b"ISO-10303-21;\nHEADER;");
        let used: Vec<(&str, &str)> = placed
            .parts
            .iter()
            .map(|p| (p["name"].as_str().unwrap(), p["model"].as_str().unwrap()))
            .collect();
        assert_eq!(used, [("R1", "model-1"), ("R2", "model-1")]);
        assert_eq!(placed.notes.len(), 1);
        assert!(placed.notes[0]["message"]
            .as_str()
            .unwrap()
            .ends_with("could not be used: R3: notes.txt is not a STEP file"));
    }

    /// The box a model's box spans on the board once placed: (min, max).
    fn placed_box(t: &Value, lo: [f64; 3], hi: [f64; 3]) -> ([f64; 3], [f64; 3]) {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for k in 0..8 {
            let corner = [0, 1, 2].map(|a| if k >> a & 1 == 0 { lo[a] } else { hi[a] });
            let p = apply(t, corner);
            for a in 0..3 {
                min[a] = min[a].min(p[a]);
                max[a] = max[a].max(p[a]);
            }
        }
        (min, max)
    }

    /// The part with this name in a manifest.
    #[cfg(feature = "altium")]
    fn part_named<'a>(m: &'a Value, name: &str) -> &'a Value {
        m["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == name)
            .unwrap()
    }

    #[cfg(feature = "altium")]
    fn package_of(name: &str) -> Package {
        let path = format!(
            "{}/../altium-pcbdoc/tests/boards/{name}.PcbDoc",
            env!("CARGO_MANIFEST_DIR")
        );
        let bytes = std::fs::read(&path).unwrap();
        build(Path::new(&path), &bytes).unwrap()
    }

    /// Equal to the 0.001 mm the boards' README.txt gives.
    #[cfg(feature = "altium")]
    fn near(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.0015)
    }

    #[cfg(feature = "altium")]
    #[test]
    fn two_parts_sharing_one_step_model_share_one_file() {
        // shared-step-model's README: R1 and R2 hold the same embedded box-1608.step.
        let package = package_of("shared-step-model");
        let m = &package.manifest;
        let mut parts: Vec<(&str, &str)> = m["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p["name"].as_str().unwrap(), p["model"].as_str().unwrap()))
            .collect();
        parts.sort();
        assert_eq!(
            parts,
            [("Board", "board"), ("R1", "model-1"), ("R2", "model-1")]
        );
        assert_eq!(package.files.len(), 2);
        assert!(package.files[1].1.starts_with(b"ISO-10303-21"));
        // box-1608.step is x -0.8 to 0.8; R2's body spans x 34.2 to 35.8, y 14.6 to 15.4.
        let (lo, hi) = placed_box(
            &part_named(m, "R2")["transform"],
            [-0.8, -0.4, 0.0],
            [0.8, 0.4, 0.45],
        );
        assert!(
            near(lo, [34.2, 14.6, 1.6]) && near(hi, [35.8, 15.4, 2.05]),
            "{lo:?} {hi:?}"
        );
    }

    #[cfg(feature = "altium")]
    #[test]
    fn two_different_step_models_give_two_files() {
        let package = package_of("two-step-models");
        assert_eq!(package.files.len(), 3);
        let m = &package.manifest;
        assert_ne!(part_named(m, "R1")["model"], part_named(m, "R2")["model"]);
        // R2 holds box-offset.step (x 0 to 1.6): its body spans x 35 to 36.6.
        let (lo, hi) = placed_box(
            &part_named(m, "R2")["transform"],
            [0.0, -0.4, 0.0],
            [1.6, 0.4, 0.45],
        );
        assert!(
            near(lo, [35.0, 14.6, 1.6]) && near(hi, [36.6, 15.4, 2.05]),
            "{lo:?} {hi:?}"
        );
    }

    #[cfg(feature = "altium")]
    #[test]
    fn turned_and_flipped_step_models_span_what_altium_shows() {
        // The eighth batch: box-offset.step (x 0 to 1.6, y -0.4 to 0.4, 0.45 high) turned 30
        // degrees on R1, then R1 flipped, then turned to 90. The README gives each body's span.
        let cases = [
            (
                "one-turned-step-resistor",
                [24.45, 15.260, 1.6],
                [26.236, 16.753, 2.05],
            ),
            (
                "one-bottom-turned-step-resistor",
                [23.8, 15.260, -0.45],
                [25.586, 16.753, 0.0],
            ),
            (
                "one-bottom-rotated-turned-step-resistor",
                [24.007, 14.45, -0.45],
                [25.5, 16.236, 0.0],
            ),
        ];
        for (board, want_lo, want_hi) in cases {
            let package = package_of(board);
            let r1 = &package.manifest["parts"][1];
            assert_eq!(r1["name"], "R1", "{board}");
            let (lo, hi) = placed_box(&r1["transform"], [0.0, -0.4, 0.0], [1.6, 0.4, 0.45]);
            assert!(
                near(lo, want_lo) && near(hi, want_hi),
                "{board}: {lo:?} {hi:?}"
            );
        }
    }

    #[cfg(feature = "altium")]
    #[test]
    fn tilted_step_models_span_what_altium_shows() {
        // The tenth batch: box-offset.step (x 0 to 1.6, y -0.4 to 0.4, 0.45 high) tilted 90
        // degrees about X, so it reaches 0.4 above and below the board face it sits on. The
        // README gives each body's span. A tilt about X needs no Degraded note any more.
        let cases = [
            (
                "one-tilted-step-resistor",
                [25.0, 14.6, 1.2],
                [26.6, 15.05, 2.0],
            ),
            (
                "one-bottom-tilted-step-resistor",
                [23.8, 14.6, -0.4],
                [25.4, 15.05, 0.4],
            ),
            (
                "one-turned-tilted-step-resistor",
                [24.975, 14.654, 1.2],
                [26.586, 15.843, 2.0],
            ),
        ];
        for (board, want_lo, want_hi) in cases {
            let package = package_of(board);
            let r1 = part_named(&package.manifest, "R1");
            let (lo, hi) = placed_box(&r1["transform"], [0.0, -0.4, 0.0], [1.6, 0.4, 0.45]);
            assert!(
                near(lo, want_lo) && near(hi, want_hi),
                "{board}: {lo:?} {hi:?}"
            );
            assert_eq!(r1["notes"], json!([]), "{board}");
        }
    }

    #[cfg(feature = "altium")]
    #[test]
    fn a_flipped_body_turned_to_90_lands_where_altium_drew_it() {
        // one-bottom-rotated-extruded-resistor's README: R1 at (25.4, 15), rotation 90, bottom;
        // the body's outline on the board is (25, 16.6), (25.4, 16.6), (25.8, 15), (25, 15). In
        // R1's frame it is (0, -0.4), (1.6, -0.4), (1.6, 0), (0, 0.4).
        let package = package_of("one-bottom-rotated-extruded-resistor");
        let t = &package.manifest["parts"][1]["transform"];
        let on_board = [
            ([0.0, -0.4, 0.0], [25.0, 15.0, 0.0]),
            ([1.6, -0.4, 0.0], [25.0, 16.6, 0.0]),
            ([1.6, 0.0, 0.0], [25.4, 16.6, 0.0]),
            ([0.0, 0.4, 0.45], [25.8, 15.0, -0.45]),
        ];
        for (model, board) in on_board {
            assert!(
                close(apply(t, model), board),
                "{model:?} -> {:?}",
                apply(t, model)
            );
        }
    }

    /// The net area of an overlay (outer rings minus holes), by the shoelace formula.
    #[cfg(feature = "altium")]
    fn overlay_area(o: &Value) -> f64 {
        let ring = |r: &Value| {
            let pts: Vec<(f64, f64)> = r
                .as_array()
                .unwrap()
                .iter()
                .map(|q| (q[0].as_f64().unwrap(), q[1].as_f64().unwrap()))
                .collect();
            let n = pts.len();
            (0..n)
                .map(|i| pts[i].0 * pts[(i + 1) % n].1 - pts[(i + 1) % n].0 * pts[i].1)
                .sum::<f64>()
                / 2.0
        };
        o["areas"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                ring(&a["outer"]) + a["holes"].as_array().unwrap().iter().map(ring).sum::<f64>()
            })
            .sum()
    }

    #[cfg(feature = "altium")]
    #[test]
    fn every_test_board_gives_a_package_with_sound_overlays() {
        let dir = format!(
            "{}/../altium-pcbdoc/tests/boards",
            env!("CARGO_MANIFEST_DIR")
        );
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("PcbDoc") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let package = build(&path, &bytes).unwrap();
            // The board's thickness T: the top face of its STEP solid.
            let (_, step) = package
                .files
                .iter()
                .find(|(name, _)| name == BOARD_FILE)
                .unwrap();
            let t = point_heights(&String::from_utf8(step.clone()).unwrap())
                .into_iter()
                .fold(f64::MIN, f64::max);
            for o in package.manifest["overlays"].as_array().unwrap() {
                assert_eq!(o["part"], "Board");
                assert!(overlay_area(o) > 0.0, "{}: {o}", path.display());
                let z = o["z"].as_f64().unwrap();
                // Copper and mask start at the face, silkscreen on top of the copper, and the
                // bottom side mirrors the top below z = 0.
                let (offset, thickness) = match o["kind"].as_str().unwrap() {
                    "copper" => (0.0, 0.035),
                    "mask" => (0.0, 0.020),
                    _ => (0.035, 0.015),
                };
                let want = if o["side"] == "top" {
                    t + offset
                } else {
                    -offset
                };
                assert!(
                    (z - want).abs() < 1e-5,
                    "{}: z {z}, want {want}",
                    path.display()
                );
                assert_eq!(o["thickness"], thickness, "{}: {o}", path.display());
                let color = o["color"].as_str().unwrap_or_default();
                assert!(
                    color.len() == 7 && color.starts_with('#'),
                    "{}: {o}",
                    path.display()
                );
                if o["kind"] == "mask" {
                    assert_eq!(o["opacity"], 0.85);
                } else {
                    assert!(o.get("opacity").is_none(), "{}: {o}", path.display());
                }
            }
            // Every test board has a usable outline, so each has both masks.
            let masks = package.manifest["overlays"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["kind"] == "mask")
                .count();
            assert_eq!(masks, 2, "{}", path.display());
            seen += 1;
        }
        assert!(seen >= 37, "{seen} boards");
    }

    #[cfg(feature = "altium")]
    fn overlay_named<'a>(m: &'a Value, board: &str, name: &str) -> &'a Value {
        m["overlays"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["name"] == name)
            .unwrap_or_else(|| panic!("{board}: no {name}"))
    }

    #[cfg(feature = "altium")]
    #[test]
    fn the_solder_mask_is_the_board_less_pads_copper_and_drills() {
        let mask = |board: &str, side: &str| {
            let package = package_of(board);
            overlay_area(overlay_named(
                &package.manifest,
                board,
                &format!("Solder mask {side}"),
            ))
        };
        // The outline is 50 x 30 = 1500 mm2 (README.txt, empty-outline).
        assert!((mask("empty-outline", "top") - 1500.0).abs() < 1e-3);
        // one-resistor: two bare SMD pads of 0.8 x 1.0 = 0.8 mm2 each on the top. The top mask is
        // 1500 - 2 * 0.8 = 1498.4; the bottom has no pads, so it stays 1500.
        assert!((mask("one-resistor", "top") - 1498.4).abs() < 1e-3);
        assert!((mask("one-resistor", "bottom") - 1500.0).abs() < 1e-3);
        // one-via: the mask stops at the via's copper (1.0 mm across, under the mask) and covers
        // no drill, so a whole 1.0 mm circle is open: 1500 - pi * 0.5^2 = 1499.21460. It is drawn
        // as a 32-sided polygon, a little under the circle (16 * 0.5^2 * sin(11.25 deg) =
        // 0.78036 mm2 against 0.78540), so the tolerance is 0.01.
        let open_via = 1500.0 - std::f64::consts::PI * 0.25;
        assert!((mask("one-via", "top") - open_via).abs() < 0.01);
        assert!((mask("one-via", "bottom") - open_via).abs() < 0.01);
        // one-plated-pad: a round 2.0 mm pad on all layers with a 1.0 mm hole. Its bare copper
        // and its drill together open the whole pad, on both faces: 1500 - pi * 1^2 = 1496.85841,
        // less a little for the 32-sided polygon (16 * sin(11.25 deg) = 3.12145 mm2 against
        // 3.14159), so the tolerance is 0.03.
        let open = 1500.0 - std::f64::consts::PI;
        assert!((mask("one-plated-pad", "top") - open).abs() < 0.03);
        assert!((mask("one-plated-pad", "bottom") - open).abs() < 0.03);
    }

    #[cfg(feature = "altium")]
    #[test]
    fn the_mask_cuts_the_boards_cutout_and_hole() {
        // outline-with-hole: a 10 x 10 cutout and a 3.0 mm unplated hole. The hole's pad is bare
        // copper (a ring of no width), so the hole is open all the same: 1500 - 100 - pi * 1.5^2
        // = 1392.93141. The hole is a 32-sided polygon, a little under the circle (16 * 1.5^2 *
        // sin(11.25 deg) = 7.0235 mm2 against 7.0686), so the tolerance is 0.08.
        let package = package_of("outline-with-hole");
        let top = overlay_named(&package.manifest, "outline-with-hole", "Solder mask top");
        let area = overlay_area(top);
        let want = 1400.0 - std::f64::consts::PI * 2.25;
        assert!((area - want).abs() < 0.08, "{area}");
    }

    #[cfg(feature = "altium")]
    #[test]
    fn a_rounded_outline_with_a_round_cutout_gives_its_mask() {
        // outline-with-arcs: the 50 x 30 rectangle with corners rounded at radius 3, and a round
        // cutout of radius 2. Each corner loses (4 - pi) / 4 * 3^2 = 1.93142 mm2, four of them
        // 7.72567: 1500 - 7.72567 - pi * 2^2 = 1479.70796. The arcs are polylines a little inside
        // the true curves (the corners lose 0.18 mm2 more, the cutout 0.08 less), so the
        // tolerance is 0.3.
        let package = package_of("outline-with-arcs");
        let top = overlay_named(&package.manifest, "outline-with-arcs", "Solder mask top");
        let want = 1500.0 - (4.0 - std::f64::consts::PI) * 9.0 - std::f64::consts::PI * 4.0;
        assert!(
            (overlay_area(top) - want).abs() < 0.3,
            "{}",
            overlay_area(top)
        );
        assert_eq!(top["areas"][0]["holes"].as_array().unwrap().len(), 1);
    }

    #[cfg(feature = "altium")]
    #[test]
    fn overlays_and_the_board_part_carry_their_colours() {
        let package = package_of("one-plated-pad");
        let m = &package.manifest;
        let colour = |name: &str| overlay_named(m, "one-plated-pad", name)["color"].clone();
        for side in ["top", "bottom"] {
            assert_eq!(colour(&format!("Solder mask {side}")), "#1f5f3a");
            assert_eq!(colour(&format!("Copper {side} pads")), "#d4af37");
        }
        assert_eq!(part_named(m, "Board")["color"], "#c8b27a");
        let mask = overlay_named(m, "one-plated-pad", "Solder mask top");
        assert_eq!(mask["opacity"], 0.85);
        assert_eq!(mask["kind"], "mask");
        // The board is 1.6 mm thick (the file stores 1.600002 after its unit conversion).
        assert!((mask["z"].as_f64().unwrap() - 1.6).abs() < 1e-4);
        let bottom = overlay_named(m, "one-plated-pad", "Solder mask bottom");
        assert_eq!(bottom["z"].as_f64().unwrap(), 0.0);
        // Masked copper and silkscreen: boards that have them.
        let track = package_of("one-track");
        assert_eq!(
            overlay_named(&track.manifest, "one-track", "Copper top")["color"],
            "#2b6e43"
        );
        let silk = package_of("one-overlay-line");
        assert_eq!(
            overlay_named(&silk.manifest, "one-overlay-line", "Silkscreen top")["color"],
            "#f2f2f2"
        );
        // Components carry no colour.
        let resistor = package_of("one-resistor");
        assert!(part_named(&resistor.manifest, "R1").get("color").is_none());
    }

    /// The radius of each cylinder in a STEP text, in mm.
    #[cfg(feature = "altium")]
    fn cylinder_radii(step: &str) -> Vec<f64> {
        step.lines()
            .filter(|l| l.contains("CYLINDRICAL_SURFACE("))
            .map(|l| {
                let tail = l.trim_end_matches(");").rsplit(',').next().unwrap();
                tail.parse().unwrap()
            })
            .collect()
    }

    /// The z of every `CARTESIAN_POINT` in a STEP text.
    #[cfg(feature = "altium")]
    fn point_heights(step: &str) -> Vec<f64> {
        step.lines()
            .filter(|l| l.contains("CARTESIAN_POINT("))
            .filter_map(|l| {
                let inner = l.split_once("'',(")?.1.split_once(')')?.0;
                inner.rsplit(',').next()?.parse().ok()
            })
            .collect()
    }

    #[cfg(feature = "altium")]
    #[test]
    fn a_via_gets_one_plating_tube_with_the_drills_radius() {
        // one-via: a through via at (25, 15), hole 0.5 mm, in a board 1.6 mm thick. Drill radius
        // 0.25 mm; the plating is 0.025 mm thick, so the inside radius is 0.225 mm.
        let package = package_of("one-via");
        let m = &package.manifest;
        // The overlays draw the via tented (a solid disc under the mask), but the board keeps
        // its drill: a 0.25 mm cylinder in board.step.
        let top = overlay_named(m, "one-via", "Copper top");
        assert_eq!(top["areas"][0]["holes"].as_array().unwrap().len(), 0);
        assert!((overlay_area(top) - 0.78).abs() < 0.01);
        let (_, board_step) = package
            .files
            .iter()
            .find(|(name, _)| name == BOARD_FILE)
            .unwrap();
        let radii = cylinder_radii(&String::from_utf8(board_step.clone()).unwrap());
        assert!(radii.iter().any(|r| (r - 0.25).abs() < 1e-3), "{radii:?}");
        let part = part_named(m, "Plating");
        assert_eq!(part["model"], "plating");
        assert_eq!(part["color"], "#d4af37");
        assert_eq!(part["transform"], json!(IDENTITY));
        assert!(m["models"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["id"] == "plating" && x["file"] == "plating.step"));
        let (_, bytes) = package
            .files
            .iter()
            .find(|(name, _)| name == "plating.step")
            .unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert_eq!(text.matches("MANIFOLD_SOLID_BREP(").count(), 1);
        let mut radii = cylinder_radii(&text);
        radii.sort_by(f64::total_cmp);
        // Two half cylinders on each of the two circles (to the 0.001 mm of the README).
        let want = [0.225, 0.225, 0.25, 0.25];
        assert!(
            radii.iter().zip(want).all(|(r, w)| (r - w).abs() < 1e-3),
            "{radii:?}"
        );
        // From z 0 up to the board's thickness.
        let zs = point_heights(&text);
        let top = zs.iter().copied().fold(f64::MIN, f64::max);
        let bottom = zs.iter().copied().fold(f64::MAX, f64::min);
        assert!(
            bottom.abs() < 1e-9 && (top - 1.6).abs() < 1e-3,
            "{bottom} {top}"
        );
        let assumed = m["notes"].as_array().unwrap().iter().any(|n| {
            n["severity"] == "Info" && n["message"].as_str().unwrap().contains("0.025 mm thick")
        });
        assert!(assumed, "{}", m["notes"]);
    }

    #[cfg(feature = "altium")]
    #[test]
    fn a_plated_pad_is_plated_and_an_unplated_hole_is_not() {
        // one-plated-pad: a 1.0 mm plated hole: radii 0.5 and 0.475.
        let package = package_of("one-plated-pad");
        let (_, bytes) = package
            .files
            .iter()
            .find(|(name, _)| name == "plating.step")
            .unwrap();
        let mut radii = cylinder_radii(&String::from_utf8(bytes.clone()).unwrap());
        radii.sort_by(f64::total_cmp);
        let want = [0.475, 0.475, 0.5, 0.5];
        assert!(
            radii.iter().zip(want).all(|(r, w)| (r - w).abs() < 1e-3),
            "{radii:?}"
        );
        // outline-with-hole: an unplated hole, and one-resistor: no hole at all.
        for board in ["outline-with-hole", "one-resistor"] {
            let package = package_of(board);
            let m = &package.manifest;
            assert!(m["parts"]
                .as_array()
                .unwrap()
                .iter()
                .all(|p| p["name"] != "Plating"));
            assert!(m["models"]
                .as_array()
                .unwrap()
                .iter()
                .all(|x| x["id"] != "plating"));
            assert!(package.files.iter().all(|(name, _)| name != "plating.step"));
        }
    }

    #[test]
    fn no_component_takes_the_plating_part_name() {
        let placed = components(
            &[
                part("Plating", 1.0, 1.0, 0.0, Side::Top),
                part("Board", 2.0, 1.0, 0.0, Side::Top),
            ],
            &[],
            1.6,
        );
        let names: Vec<&str> = placed
            .parts
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["Plating (2)", "Board (2)"]);
    }

    fn cut(x: f64, radius: f64, plated: bool) -> board_step::CutHole {
        board_step::CutHole {
            at: Point { x, y: 1.0 },
            radius,
            plated,
        }
    }

    #[test]
    fn holes_too_small_to_line_are_a_note() {
        use board_model::{Board, CopperItem};
        let via = |drill| CopperItem::Via {
            at: Point { x: 1.0, y: 1.0 },
            diameter: 1.0,
            drill,
        };
        // A 0.05 mm drill has radius 0.025: nothing left inside the plating.
        let small = Board {
            copper: vec![via(0.1)],
            ..Board::default()
        };
        let p = plating(&small, &[cut(1.0, 0.025, true)], 1.6);
        assert!(p.text.is_none());
        assert_eq!(p.notes.len(), 1);
        assert_eq!(p.notes[0]["severity"], "Dropped");
        // A usable one beside it still gets its tube.
        let both = Board {
            copper: vec![via(0.05), via(0.3)],
            ..Board::default()
        };
        let p = plating(&both, &[cut(1.0, 0.025, true), cut(3.0, 0.15, true)], 1.6);
        assert!(p.text.is_some());
        assert!(p.notes.iter().any(|n| n["severity"] == "Dropped"));
    }

    #[test]
    fn a_plated_hole_that_was_not_cut_gets_no_tube() {
        use board_model::{Board, CopperItem, Outline};
        let via = |x: f64, y: f64| CopperItem::Via {
            at: Point { x, y },
            diameter: 1.0,
            drill: 0.5,
        };
        // A 20 x 10 board with a 4 x 4 cutout from (8, 3). Via 1 is clear; via 2 touches the
        // board edge (its centre is 0.1 from x = 0, less than its 0.25 radius); via 3 is inside
        // the cutout. board-step cuts only the first, so only it is plated.
        let board = Board {
            thickness: Some(1.6),
            outline: Outline {
                outer: rect(0.0, 0.0, 20.0, 10.0),
                cutouts: vec![rect(8.0, 3.0, 12.0, 7.0)],
            },
            copper: vec![via(5.0, 5.0), via(0.1, 5.0), via(10.0, 5.0)],
            ..Board::default()
        };
        let step = board_step::write(&board, "t").unwrap();
        assert_eq!(step.holes.len(), 1);
        let p = plating(&board, &step.holes, step.thickness);
        let text = p.text.unwrap();
        assert_eq!(text.matches("MANIFOLD_SOLID_BREP(").count(), 1);
        let said = p.notes.iter().any(|n| {
            n["severity"] == "Dropped"
                && n["message"]
                    == "2 plated hole(s) were not cut into the board, so have no plating"
        });
        assert!(said, "{:?}", p.notes);
        // A board whose only plated hole is not cut has no plating at all.
        let edge = Board {
            copper: vec![via(0.1, 5.0)],
            ..board
        };
        let step = board_step::write(&edge, "t").unwrap();
        let p = plating(&edge, &step.holes, step.thickness);
        assert!(p.text.is_none());
        assert_eq!(p.notes.len(), 1);
    }

    /// The bounding box of an overlay's outer rings: (min x, min y, max x, max y).
    #[cfg(feature = "altium")]
    fn overlay_bounds(o: &Value) -> (f64, f64, f64, f64) {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for a in o["areas"].as_array().unwrap() {
            for q in a["outer"].as_array().unwrap() {
                let (x, y) = (q[0].as_f64().unwrap(), q[1].as_f64().unwrap());
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
        (x0, y0, x1, y1)
    }

    #[cfg(feature = "altium")]
    #[test]
    fn texts_are_drawn_on_their_silkscreen_where_the_board_puts_them() {
        // README.txt, third batch: stroke texts 2.000 high with a 0.200 pen. Capitals stand on
        // the baseline at the position, so with the pen's 0.1 radius they reach from y - 0.1 up
        // to y + 2.1.
        let near = |a: f64, b: f64| (a - b).abs() < 1e-3;
        let silk = |board: &str, name: &str| {
            let package = package_of(board);
            let m = &package.manifest;
            let o = m["overlays"]
                .as_array()
                .unwrap()
                .iter()
                .find(|o| o["name"] == name)
                .unwrap_or_else(|| panic!("{board}: no {name}"))
                .clone();
            let degraded = m["notes"].as_array().unwrap().iter().any(|n| {
                n["severity"] == "Degraded"
                    && n["message"]
                        .as_str()
                        .unwrap()
                        .starts_with("1 text(s) are drawn in the Hershey")
            });
            assert!(degraded, "{board}: {}", m["notes"]);
            overlay_bounds(&o)
        };
        // "TEST" at (10, 10): runs right from x 10. The T's bar starts one font unit (0.095)
        // into its cell, less the pen's radius: x 9.995.
        let (x0, y0, x1, y1) = silk("one-text", "Silkscreen top");
        assert!(near(y0, 9.9) && near(y1, 12.1), "{y0} {y1}");
        assert!(near(x0, 9.995) && x1 > 14.0 && x1 < 18.0, "{x0} {x1}");
        // "ROT" at (25, 15), turned 90 degrees: runs up from y 15, capitals left of x 25.
        let (x0, y0, x1, y1) = silk("one-rotated-text", "Silkscreen top");
        assert!(near(x0, 22.9) && near(x1, 25.1), "{x0} {x1}");
        assert!(y0 > 15.0 && y1 > 18.0, "{y0} {y1}");
        // "BOT" at (40, 10), mirrored on the bottom: runs left from x 40.
        let (x0, y0, x1, y1) = silk("one-bottom-text", "Silkscreen bottom");
        assert!(near(y0, 9.9) && near(y1, 12.1), "{y0} {y1}");
        assert!(x1 < 40.0 && x0 < 37.0, "{x0} {x1}");
    }

    #[cfg(feature = "altium")]
    #[test]
    fn the_test_boards_copper_and_silkscreen_have_their_readme_areas() {
        let area_of = |board: &str, overlay: &str| {
            let package = package_of(board);
            let list = package.manifest["overlays"].as_array().unwrap().clone();
            let o = list
                .iter()
                .find(|o| o["name"] == overlay)
                .unwrap_or_else(|| {
                    panic!("{board}: no {overlay} in {list:?}");
                });
            overlay_area(o)
        };
        // one-track: 10 long, 0.5 wide, round ends.
        let track = 5.0 + std::f64::consts::PI * 0.0625;
        assert!((area_of("one-track", "Copper top") - track).abs() < 0.01);
        assert!((area_of("one-bottom-track", "Copper bottom") - track).abs() < 0.01);
        // one-resistor: two SMD pads 0.8 x 1.0 on Top Layer.
        assert!((area_of("one-resistor", "Copper top pads") - 1.6).abs() < 1e-3);
        // one-plated-pad: round 2, hole 1, on both faces.
        let ring = std::f64::consts::PI * 0.75;
        assert!((area_of("one-plated-pad", "Copper bottom pads") - ring).abs() < 0.02);
        // one-overlay-line: (10, 10) to (20, 10), 0.2 wide.
        let line = 2.0 + std::f64::consts::PI * 0.01;
        assert!((area_of("one-overlay-line", "Silkscreen top") - line).abs() < 0.01);
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
        assert_eq!(
            m["models"][1],
            json!({ "id": "model-1", "file": "model-1.step" })
        );
    }

    #[cfg(feature = "altium")]
    #[test]
    fn the_flipped_off_centre_body_lands_where_altium_drew_it() {
        // one-bottom-extruded-resistor's README: the outline on the board is (23.8, 14.6),
        // (23.8, 15), (25.4, 15.4), (25.4, 14.6), projecting from the bottom. In R1's own frame
        // that body is (1.6, -0.4), (1.6, 0), (0, 0.4), (0, -0.4) (the same as on the top-side
        // one-offset-extruded-resistor), 0.45 high.
        let bytes =
            include_bytes!("../../altium-pcbdoc/tests/boards/one-bottom-extruded-resistor.PcbDoc");
        let package = build(Path::new("one-bottom-extruded-resistor.PcbDoc"), bytes).unwrap();
        let t = &package.manifest["parts"][1]["transform"];
        let on_board = [
            ([1.6, -0.4, 0.0], [23.8, 14.6, 0.0]),
            ([1.6, 0.0, 0.0], [23.8, 15.0, 0.0]),
            ([0.0, 0.4, 0.0], [25.4, 15.4, 0.0]),
            ([0.0, -0.4, 0.45], [25.4, 14.6, -0.45]),
        ];
        for (model, board) in on_board {
            assert!(
                close(apply(t, model), board),
                "{model:?} -> {:?}",
                apply(t, model)
            );
        }
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
        assert!(close(
            [span(0).0, span(1).0, span(2).0],
            [24.2, 14.6, -0.45]
        ));
        assert!(close([span(0).1, span(1).1, span(2).1], [25.8, 15.4, 0.0]));
    }

    /// True when `(x, y)` is inside an overlay area's outer ring and in none of its holes.
    #[cfg(feature = "altium")]
    fn covered(overlay: &Value, x: f64, y: f64) -> bool {
        let inside = |ring: &Value| {
            let pts = ring_points(ring);
            let mut odd = false;
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                if (a.1 > y) != (b.1 > y) && x < a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1) {
                    odd = !odd;
                }
            }
            odd
        };
        overlay["areas"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| inside(&a["outer"]) && !a["holes"].as_array().unwrap().iter().any(inside))
    }

    #[cfg(feature = "altium")]
    fn ring_points(ring: &Value) -> Vec<(f64, f64)> {
        ring.as_array()
            .unwrap()
            .iter()
            .map(|q| (q[0].as_f64().unwrap(), q[1].as_f64().unwrap()))
            .collect()
    }

    /// The area of a ring (negative for a clockwise one).
    #[cfg(feature = "altium")]
    fn ring_area(ring: &Value) -> f64 {
        let pts = ring_points(ring);
        (0..pts.len())
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                a.0 * b.1 - b.0 * a.1
            })
            .sum::<f64>()
            / 2.0
    }

    #[cfg(feature = "altium")]
    #[test]
    fn the_small_boards_top_pour_has_its_clearances_cut_out() {
        let package = package_of("small-board");
        let copper = overlay_named(&package.manifest, "small-board", "Copper top");
        let areas = copper["areas"].as_array().unwrap();
        let holes: usize = areas
            .iter()
            .map(|a| a["holes"].as_array().unwrap().len())
            .sum();
        assert!(holes > 0, "the pour has no holes");
        // The SIG track (25.8, 15) to (30, 15), 0.25 wide, lies in a clearance gap of the GND
        // pour: its centre is copper only because of the track, and the gap shows on both sides.
        assert!(covered(copper, 27.9, 15.0));
        assert!(!covered(copper, 27.9, 15.2));
        assert!(!covered(copper, 27.9, 14.8));
        // Open pour, well away from every net.
        assert!(covered(copper, 45.0, 10.0));
        // The net area is less than the pour's outline (46 x 26) alone.
        let net: f64 = areas
            .iter()
            .map(|a| {
                ring_area(&a["outer"])
                    + a["holes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(ring_area)
                        .sum::<f64>()
            })
            .sum();
        assert!(net < 46.0 * 26.0 - 10.0, "{net}");
    }

    #[test]
    fn a_file_that_is_not_a_board_is_an_error() {
        assert!(build(Path::new("notes.txt"), b"hello").is_err());
    }
}
