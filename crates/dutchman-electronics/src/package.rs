//! `read <file> --out <dir> --json`: a board file into a placed-parts package
//! (`dutchman.parts/1`, dutchman-cad/dutchman#1005) that Dutchman writes into SolidCraft.
//!
//! The package is a folder of STEP files plus a small manifest printed on stdout:
//! - `models`: each geometry file once, `{id, file}` (file names relative to the folder),
//! - `parts`: placed instances, `{name, model, transform, notes}`; `name` is the stable
//!   identity (the board itself is `Board`, components will use their designators),
//! - `overlays`: reserved for copper and silkscreen, drawn flat on the board later,
//! - `notes`: everything read but not in the package, as `Info`, `Degraded` or `Dropped`.

use std::io::Read;
use std::path::Path;

use board_model::{Note, Severity, MAX_INPUT_BYTES};
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
        (
            board.components.len(),
            "component(s) were read but are not placed yet",
        ),
    ];
    for (count, what) in later {
        if count > 0 {
            notes.push(json!({ "severity": "Dropped", "message": format!("{count} {what}") }));
        }
    }
    let board_notes: Vec<Value> = step.notes.iter().map(note).collect();
    let manifest = json!({
        "schema": PARTS_SCHEMA,
        "ok": true,
        "name": name,
        "units": "mm",
        "source": { "format": format.id(), "format_name": format.name() },
        "models": [ { "id": BOARD_MODEL, "file": BOARD_FILE } ],
        "parts": [ {
            "name": BOARD_PART,
            "model": BOARD_MODEL,
            "transform": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]],
            "notes": board_notes,
        } ],
        "overlays": [],
        "notes": notes,
    });
    Ok(Package {
        manifest,
        files: vec![(BOARD_FILE.to_string(), step.text.into_bytes())],
    })
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

    #[test]
    fn a_file_that_is_not_a_board_is_an_error() {
        assert!(build(Path::new("notes.txt"), b"hello").is_err());
    }
}
