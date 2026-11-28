//! `read <file> --out <dir> --json`: a board file into a placed-parts package
//! (`dutchman.parts/1`, dutchman-cad/dutchman#1005) that Dutchman writes into SolidCraft.
//!
//! The package is a folder of STEP files plus a small manifest printed on stdout:
//! - `models`: each geometry file once, `{id, file}` (file names relative to the folder),
//! - `parts`: placed instances, `{name, model, transform, notes}`; `name` is the stable
//!   identity (the board itself is `Board`, each component its designator),
//! - `overlays`: copper, silkscreen and solder mask as flat areas on the board's faces (see
//!   `overlay`), each with a color,
//! - a `Plating` part when the board has plated holes: the copper lining them, one tube each, all
//!   in one STEP file,
//! - `notes`: everything read but not in the package, as `Info`, `Degraded` or `Dropped`.
//!
//! The file may also be an Altium project (`.PrjPcb`, with the `altium` feature): [`open`] then
//! reads the board it holds. The reader never opens a file itself: this module opens the project
//! and its board, and fills in the board's texts that refer to the project's parameters or to
//! Altium's special strings (`altium_pcbdoc::special_strings`).

use std::io::Read;
use std::path::{Path, PathBuf};

use board_model::{
    Board, CopperItem, EmbeddedModel, ModelRef, ModelSource, Note, Placement, Severity, Side,
    MAX_INPUT_BYTES,
};
use serde_json::{json, Value};

use crate::colors::{hex, pad_look, BOARD_COLOR, BOARD_FINISH, BODY_FINISH};
pub use crate::standard::Bodies;
use crate::standard::{Recognizer, Standard};

/// Schema of the package manifest.
pub const PARTS_SCHEMA: &str = "dutchman.parts/1";
/// The longest parameter value taken, in bytes, from `--param` and from a project: every text
/// that refers to a parameter gets its own copy of the value.
#[cfg(feature = "altium")]
pub use altium_pcbdoc::project::MAX_PARAMETER_VALUE;
/// The longest parameter value taken, in bytes, from `--param`.
#[cfg(not(feature = "altium"))]
pub const MAX_PARAMETER_VALUE: usize = 64 * 1024;
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
/// How thick the copper lining a plated hole is, in mm (a common 25 micrometers). Board files do
/// not give it, so it is assumed and a note says so.
pub const PLATING_THICKNESS: f64 = 0.025;
/// The folder (`group`) a loader puts the components of each side in, and the overlays in. The
/// `Board` and `Plating` parts have none and stay at the top level.
pub const GROUP_TOP: &str = "Components top";
pub const GROUP_BOTTOM: &str = "Components bottom";
pub const GROUP_LAYERS: &str = "Layers";
/// The folder of the 3D bodies that belong to no part.
pub const GROUP_FREE: &str = "Free bodies";
/// The most bodies one part gets; more are left out, with a note.
pub const MAX_BODIES_PER_PART: usize = 256;
/// The most free bodies the package holds; more are left out, with a note.
pub const MAX_FREE_BODIES: usize = 10_000;
/// The most bodies (of parts and free) the package holds in all.
pub const MAX_BODIES: usize = 100_000;

/// A package before it is written: the manifest and its files.
#[derive(Clone, Debug, PartialEq)]
pub struct Package {
    pub manifest: Value,
    /// (file name relative to the package folder, contents)
    pub files: Vec<(String, Vec<u8>)>,
}

impl Package {
    /// Adds notes (manifest notes: `{severity, message}`) to the manifest's.
    pub fn add_notes(&mut self, more: Vec<Value>) {
        if let Some(notes) = self.manifest.get_mut("notes").and_then(Value::as_array_mut) {
            notes.extend(more);
        }
    }
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

/// A board file to read, with the parameters its texts may refer to.
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    /// The board file itself (inside the project's folder when it came from a project).
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    /// (name, value), each name once in any case.
    pub parameters: Vec<(String, String)>,
    /// Notes on how the input was opened (manifest notes), to add to the package's.
    pub notes: Vec<Value>,
}

/// What `read` was asked to open besides the file itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Open<'a> {
    /// For a project: which of its boards (file name, with or without `.PcbDoc`, any case).
    pub board: Option<&'a str>,
    /// For a board: the project whose parameters its texts use, without opening its links.
    pub project: Option<&'a Path>,
    /// Parameters given by hand; a name given here wins over the project's.
    pub parameters: &'a [(String, String)],
}

/// Opens what `read` was given. A board file is read as it is, with the parameters of
/// `with.project` when given. An Altium project (`.PrjPcb`) gives its board, the only one or
/// the one `with.board` names, from inside the project's folder: a link that is absolute or
/// leaves that folder is refused, and such a board can be read by passing it as the file with
/// the project as `with.project`.
pub fn open(path: &Path, with: &Open<'_>) -> Result<Input, String> {
    let bytes = read_input(path)?;
    let mut input = if is_project(path) {
        if with.project.is_some() {
            return Err("--project goes with a board file, not with a project".into());
        }
        project_board(path, &bytes, with.board)?
    } else if let Some(name) = with.board {
        return Err(format!(
            "--board {name} chooses a board of a project, but {} is not a project",
            path.display()
        ));
    } else {
        let (parameters, notes) = match with.project {
            Some(project) if is_kicad_project(project) => {
                (kicad_project_variables(project)?, Vec::new())
            }
            Some(project) => (project_parameters(project)?, Vec::new()),
            None if is_kicad_board(path) => variables_next_to(path),
            None => parameters_next_to(path),
        };
        Input {
            path: path.to_path_buf(),
            bytes,
            parameters,
            notes,
        }
    };
    // KiCad's text variable names are case-sensitive; Altium's parameter names are not.
    let kicad = is_kicad_board(&input.path);
    for (name, value) in with.parameters {
        let known = input.parameters.iter_mut().find(|(n, _)| {
            if kicad {
                n == name
            } else {
                n.eq_ignore_ascii_case(name)
            }
        });
        match known {
            Some((_, v)) => v.clone_from(value),
            None => input.parameters.push((name.clone(), value.clone())),
        }
    }
    Ok(input)
}

/// The parameters of a project file, for a board passed on its own.
#[cfg(feature = "altium")]
fn project_parameters(project: &Path) -> Result<Vec<(String, String)>, String> {
    let bytes = read_input(project)?;
    altium_pcbdoc::project::read(&bytes)
        .map(|p| p.parameters)
        .map_err(|e| format!("{}: {e}", project.display()))
}

#[cfg(not(feature = "altium"))]
fn project_parameters(project: &Path) -> Result<Vec<(String, String)>, String> {
    Err(format!(
        "{}: this build does not read Altium projects",
        project.display()
    ))
}

/// The most `.PrjPcb` files looked at in a board's folder.
#[cfg(feature = "altium")]
const MAX_PROJECTS_NEXT_TO_BOARD: usize = 64;

/// The parameters of the project that sits in the board's own folder and links the board, for a
/// board passed without `--project`, and the notes on that. The folder is looked at once, not
/// below: at most [`MAX_PROJECTS_NEXT_TO_BOARD`] `.PrjPcb` files, each at most `MAX_INPUT_BYTES`
/// long and read like any project. One project that links the board gives its parameters; several
/// give none (a warning names them); none gives none, silently. A project that is a symbolic
/// link leading out of the folder is not read.
#[cfg(feature = "altium")]
fn parameters_next_to(board: &Path) -> (Vec<(String, String)>, Vec<Value>) {
    let is_board = board
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pcbdoc"));
    let (Some(file), true) = (board.file_name().and_then(|f| f.to_str()), is_board) else {
        return (Vec::new(), Vec::new());
    };
    let folder = match board.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let Ok(real_folder) = folder.canonicalize() else {
        return (Vec::new(), Vec::new());
    };
    let Ok(entries) = std::fs::read_dir(folder) else {
        return (Vec::new(), Vec::new());
    };
    let mut found: Vec<PathBuf> = Vec::new();
    let mut too_many = false;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_prj = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("prjpcb"));
        if !is_prj {
            continue;
        }
        if found.len() >= MAX_PROJECTS_NEXT_TO_BOARD {
            too_many = true;
            break;
        }
        found.push(path);
    }
    found.sort();
    let warn = |message: String| json!({ "severity": "Degraded", "message": message });
    let mut notes = Vec::new();
    if too_many {
        notes.push(warn(format!(
            "more than {MAX_PROJECTS_NEXT_TO_BOARD} project files (.PrjPcb) next to the board; \
             only {MAX_PROJECTS_NEXT_TO_BOARD} were looked at. Use --project to name the project"
        )));
    }
    let mut linking: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for path in &found {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let skipped = |why: &str| {
            warn(format!(
                "the project {name} next to the board was not used: {why}"
            ))
        };
        let inside = path
            .canonicalize()
            .is_ok_and(|real| real.starts_with(&real_folder));
        if !inside {
            notes.push(skipped(
                "it leads out of the board's folder through a symbolic link",
            ));
            continue;
        }
        let too_big = std::fs::metadata(path)
            .map(|m| m.len() > u64::try_from(MAX_INPUT_BYTES).unwrap_or(u64::MAX))
            .unwrap_or(false);
        if too_big {
            notes.push(skipped(&format!(
                "it is larger than {MAX_INPUT_BYTES} bytes"
            )));
            continue;
        }
        let project = match read_input(path)
            .and_then(|bytes| altium_pcbdoc::project::read(&bytes).map_err(|e| e.to_string()))
        {
            Ok(project) => project,
            Err(e) => {
                notes.push(skipped(&e));
                continue;
            }
        };
        if project.boards.iter().any(|link| links_to(link, file)) {
            linking.push((name, project.parameters));
        }
    }
    match linking.len() {
        0 => (Vec::new(), notes),
        1 => {
            let (name, parameters) = linking.remove(0);
            notes.push(json!({
                "severity": "Info",
                "message": format!("parameters taken from {name} next to the board"),
            }));
            (parameters, notes)
        }
        n => {
            let names: Vec<String> = linking.into_iter().map(|(name, _)| name).collect();
            notes.push(warn(format!(
                "{n} projects next to the board link it ({}); none of their parameters are \
             used. Use --project to name the one to take them from",
                names.join(", ")
            )));
            (Vec::new(), notes)
        }
    }
}

#[cfg(not(feature = "altium"))]
fn parameters_next_to(_: &Path) -> (Vec<(String, String)>, Vec<Value>) {
    (Vec::new(), Vec::new())
}

/// True when a project's link (`\` or `/`, any case, a leading `.\` allowed) names the file
/// `file` in the project's own folder.
#[cfg(feature = "altium")]
fn links_to(link: &str, file: &str) -> bool {
    if altium_pcbdoc::project::link_problem(link).is_some() {
        return false;
    }
    let mut parts = link
        .split(['\\', '/'])
        .filter(|p| !p.is_empty() && *p != ".");
    matches!((parts.next(), parts.next()), (Some(only), None) if only.eq_ignore_ascii_case(file))
}

/// True for a KiCad board file (`.kicad_pcb`, in any case), when this build reads KiCad.
fn is_kicad_board(path: &Path) -> bool {
    cfg!(feature = "kicad") && has_extension(path, "kicad_pcb")
}

/// True for a KiCad project file (`.kicad_pro`, in any case), when this build reads KiCad.
fn is_kicad_project(path: &Path) -> bool {
    cfg!(feature = "kicad") && has_extension(path, "kicad_pro")
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(extension))
}

/// The text variables of a KiCad project file given with `--project`.
#[cfg(feature = "kicad")]
fn kicad_project_variables(project: &Path) -> Result<Vec<(String, String)>, String> {
    let bytes = read_input(project)?;
    kicad_pcb::project_variables(&bytes).map_err(|e| format!("{}: {e}", project.display()))
}

#[cfg(not(feature = "kicad"))]
fn kicad_project_variables(project: &Path) -> Result<Vec<(String, String)>, String> {
    Err(format!(
        "{}: this build does not read KiCad projects",
        project.display()
    ))
}

/// The text variables of the KiCad project beside a board passed without `--project`: the
/// `.kicad_pro` of the board's own name in the board's folder, the one KiCad opens with the board.
/// None there gives none, silently; one that cannot be used gives a warning naming it (by its file
/// name only), as does one that is a symbolic link leading out of the folder.
#[cfg(feature = "kicad")]
fn variables_next_to(board: &Path) -> (Vec<(String, String)>, Vec<Value>) {
    let project = board.with_extension("kicad_pro");
    let Some(name) = project
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
    else {
        return (Vec::new(), Vec::new());
    };
    if !project.is_file() {
        return (Vec::new(), Vec::new());
    }
    let skipped = |why: &str| {
        let message = format!("the project {name} next to the board was not used: {why}");
        (
            Vec::new(),
            vec![json!({ "severity": "Degraded", "message": message })],
        )
    };
    let folder = match board.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let inside = match (folder.canonicalize(), project.canonicalize()) {
        (Ok(f), Ok(p)) => p.starts_with(f),
        _ => false,
    };
    if !inside {
        return skipped("it leads out of the board's folder through a symbolic link");
    }
    let too_big = std::fs::metadata(&project)
        .map(|m| m.len() > u64::try_from(MAX_INPUT_BYTES).unwrap_or(u64::MAX))
        .unwrap_or(false);
    if too_big {
        return skipped(&format!("it is larger than {MAX_INPUT_BYTES} bytes"));
    }
    let Ok(bytes) = std::fs::read(&project) else {
        return skipped("it cannot be read");
    };
    match kicad_pcb::project_variables(&bytes) {
        Ok(variables) => (
            variables,
            vec![json!({
                "severity": "Info",
                "message": format!("text variables taken from {name} next to the board"),
            })],
        ),
        Err(e) => skipped(&e),
    }
}

#[cfg(not(feature = "kicad"))]
fn variables_next_to(_: &Path) -> (Vec<(String, String)>, Vec<Value>) {
    (Vec::new(), Vec::new())
}

/// Reads a board file. A KiCad board gets its file name (no folder) and `parameters`, its
/// project's text variables, for the texts that use them.
fn read_board(
    format: &dyn board_model::InputFormat,
    path: &Path,
    bytes: &[u8],
    parameters: &[(String, String)],
) -> Result<board_model::ReadOutcome, String> {
    #[cfg(feature = "kicad")]
    if format.id() == board_model::InputFormat::id(&kicad_pcb::KiCad) {
        let variables = kicad_pcb::Variables {
            file_name: path
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string),
            project: parameters.to_vec(),
        };
        return kicad_pcb::read_with(bytes, &variables).map_err(|e| e.to_string());
    }
    let _ = (path, parameters);
    format.read(bytes).map_err(|e| e.to_string())
}

/// True for an Altium project file (`.PrjPcb`, in any case), when this build reads Altium.
fn is_project(path: &Path) -> bool {
    cfg!(feature = "altium")
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("prjpcb"))
}

/// The board a project holds, read from the project's folder, with the project's parameters.
#[cfg(feature = "altium")]
fn project_board(path: &Path, bytes: &[u8], choice: Option<&str>) -> Result<Input, String> {
    let project =
        altium_pcbdoc::project::read(bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let listed = project.boards.join(", ");
    let index = match (choice, project.boards.len()) {
        (_, 0) => return Err(format!("{} holds no board (.PcbDoc)", path.display())),
        (None, 1) => 0,
        (None, n) => {
            return Err(format!(
                "{} holds {n} boards ({listed}); choose one with --board NAME",
                path.display()
            ))
        }
        (Some(wanted), _) => {
            let matches: Vec<usize> = project
                .boards
                .iter()
                .enumerate()
                .filter(|(_, link)| names_board(link, wanted))
                .map(|(i, _)| i)
                .collect();
            match matches.as_slice() {
                [one] => *one,
                [] => {
                    return Err(format!(
                        "{} holds no board named {wanted} (it holds {listed})",
                        path.display()
                    ))
                }
                _ => {
                    return Err(format!(
                        "{wanted} is ambiguous: {} holds {} boards of that name ({listed}); \
                         give the board's folder too, as the project lists it",
                        path.display(),
                        matches.len()
                    ))
                }
            }
        }
    };
    let link = project.boards.get(index).ok_or("no such board")?;
    if let Some(why) = altium_pcbdoc::project::link_problem(link) {
        return Err(format!(
            "the project's link to {link} {why}, so it is not followed; to read that board, pass \
             it as the file and add --project {}",
            path.display()
        ));
    }
    let relative = link.replace('\\', "/");
    let folder = path.parent().unwrap_or(Path::new("."));
    let board_path = folder.join(&relative);
    let cannot = |e: &dyn std::fmt::Display| {
        format!(
            "the project's board {relative} cannot be read ({e}); it must sit where the project \
             says, relative to the project's folder"
        )
    };
    // A link inside the folder can still lead out of it through a symbolic link (or a
    // junction); the board is read only when its real place is inside the project's folder.
    let real_folder = folder.canonicalize().map_err(|e| cannot(&e))?;
    let real_board = board_path.canonicalize().map_err(|e| cannot(&e))?;
    if !real_board.starts_with(&real_folder) {
        return Err(format!(
            "the project's link to {link} leads out of the project's folder through a symbolic \
             link, so it is not followed; to read that board, pass it as the file and add \
             --project {}",
            path.display()
        ));
    }
    let bytes = read_input(&real_board).map_err(|e| cannot(&e))?;
    Ok(Input {
        path: board_path,
        bytes,
        parameters: project.parameters,
        notes: Vec::new(),
    })
}

/// True when `wanted` names the board a project links to as `link`: the whole link (with `\`
/// or `/`), the file name, or the file name without `.PcbDoc`, in any case.
#[cfg(feature = "altium")]
fn names_board(link: &str, wanted: &str) -> bool {
    let same = |a: &str, b: &str| {
        a.replace('\\', "/")
            .eq_ignore_ascii_case(&b.replace('\\', "/"))
    };
    let file = link.rsplit(['\\', '/']).next().unwrap_or(link);
    same(link, wanted)
        || same(file, wanted)
        || file
            .rsplit_once('.')
            .is_some_and(|(stem, _)| stem.eq_ignore_ascii_case(wanted))
}

#[cfg(not(feature = "altium"))]
fn project_board(path: &Path, _: &[u8], _: Option<&str>) -> Result<Input, String> {
    Err(format!(
        "{}: this build does not read Altium projects",
        path.display()
    ))
}

/// Builds the package for a board file's bytes. `path` gives the extension and, when the file
/// names no board, the board's name. An Altium board's texts that refer to one of `parameters`
/// (name, value) or to Altium's special strings get their value. The board's folder and that
/// folder's `Models` folder, then `model_folders`, are where linked model files are looked for
/// (see [`crate::linked`]).
pub fn build(
    path: &Path,
    bytes: &[u8],
    parameters: &[(String, String)],
    model_folders: &[PathBuf],
    copper: crate::overlay::Copper,
) -> Result<Package, String> {
    build_with_bodies(
        path,
        bytes,
        parameters,
        model_folders,
        copper,
        Bodies::default(),
    )
}

/// The overlay kinds a loader says it can draw beyond the ones every loader knows (`--accept-overlay`).
/// Without one, the package holds only `copper`, `silkscreen` and `mask`, and the mask is opaque.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Accept {
    /// The loader draws `laminate` overlays and see-through masks.
    pub laminate: bool,
}

impl Accept {
    /// The overlay kinds that can be negotiated.
    pub const KINDS: &'static [&'static str] = &["laminate"];

    /// Adds the kind named `kind`, or says which kinds there are.
    pub fn accept(&mut self, kind: &str) -> Result<(), String> {
        match kind {
            "laminate" => self.laminate = true,
            other => {
                return Err(format!(
                    "--accept-overlay {other}: not a kind we can negotiate; accepted: {}",
                    Self::KINDS.join(", ")
                ))
            }
        }
        Ok(())
    }
}

/// [`build`], with the choice of which bodies the parts get (`--bodies`, see [`Bodies`]).
pub fn build_with_bodies(
    path: &Path,
    bytes: &[u8],
    parameters: &[(String, String)],
    model_folders: &[PathBuf],
    copper: crate::overlay::Copper,
    bodies: Bodies,
) -> Result<Package, String> {
    build_accepting(
        path,
        bytes,
        parameters,
        model_folders,
        copper,
        bodies,
        Accept::default(),
    )
}

/// [`build_with_bodies`], with the overlay kinds the loader accepts (`--accept-overlay`). Without
/// `laminate` there is no laminate overlay and the mask's `opacity` is 1.0, so an older loader
/// gets a correct board: copper hidden under an opaque mask.
pub fn build_accepting(
    path: &Path,
    bytes: &[u8],
    parameters: &[(String, String)],
    model_folders: &[PathBuf],
    copper: crate::overlay::Copper,
    bodies: Bodies,
    accept: Accept,
) -> Result<Package, String> {
    let extension = path.extension().and_then(|e| e.to_str());
    let head = bytes.get(..bytes.len().min(4096)).unwrap_or_default();
    let format = crate::detect(head, extension)
        .ok_or_else(|| "not a board file this module reads".to_string())?;
    let mut outcome = read_board(format.as_ref(), path, bytes, parameters)?;
    #[cfg(feature = "altium")]
    if format.id() == board_model::InputFormat::id(&altium_pcbdoc::Altium) {
        let notes = altium_pcbdoc::special_strings::resolve_texts(&mut outcome.board, parameters);
        outcome.notes.extend(notes);
    }
    #[cfg(not(feature = "altium"))]
    let _ = parameters;

    let linked_notes = crate::linked::resolve(
        &mut outcome,
        format.as_ref(),
        &crate::linked::folders(path, model_folders),
    );
    outcome.notes.extend(linked_notes);
    let name = if outcome.board.name.trim().is_empty() {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("board")
            .to_string()
    } else {
        outcome.board.name.clone()
    };
    build_outcome(outcome, format.as_ref(), &name, copper, bodies, accept)
}

/// Builds a package from an already read board, including its notes, models and fonts.
/// Linked models and text parameters must be resolved before calling this function.
pub fn build_outcome(
    outcome: board_model::ReadOutcome,
    format: &dyn board_model::InputFormat,
    name: &str,
    copper: crate::overlay::Copper,
    bodies: Bodies,
    accept: Accept,
) -> Result<Package, String> {
    let board = outcome.board;
    // When there is no outline to make a board from, the reader's own words say why (an older
    // KiCad file, an outline that does not close), so the first Dropped note comes with the
    // error. Any other failure keeps its own message.
    let why = if board.outline.outer.segments.is_empty() {
        outcome
            .notes
            .iter()
            .find(|n| n.severity == Severity::Dropped)
            .map(|n| n.message.clone())
    } else {
        None
    };
    // A reader that refused the whole file (a KiCad board older than 9) returns no layers and one
    // note; that note alone is the error, in plain words, rather than "no outline" before it.
    let refused = board.layers.is_empty() && outcome.notes.len() == 1;
    let step = board_step::write(&board, name).map_err(|e| match &why {
        Some(why) if refused => why.clone(),
        Some(why) => format!("{e}: {why}"),
        None => e.to_string(),
    })?;

    let overlay_thickness = step.thickness;
    let board_ref = &board;
    let fonts = &outcome.fonts;
    std::thread::scope(|scope| {
        let render = || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::overlay::overlays(board_ref, fonts, overlay_thickness, BOARD_PART, copper)
            })).unwrap_or_else(|_| (Vec::new(), vec![json!({
                "severity": "Dropped",
                "message": "the laminate, copper, finish, silkscreen and solder mask overlays could not be drawn",
            })]))
        };
        // We start only after STEP succeeds. Plating and components read the same board
        // independently; failed worker creation uses the serial path.
        let worker = std::thread::Builder::new().spawn_scoped(scope, render).ok();
        let mut notes: Vec<Value> = outcome.notes.iter().map(note).collect();
        let board_notes: Vec<Value> = step.notes.iter().map(note).collect();
        // What was left out of the board solid is said in the package's own notes too, not only on
        // the Board part.
        notes.extend(board_notes.iter().cloned());
        let mut models = vec![json!({ "id": BOARD_MODEL, "file": BOARD_FILE })];
        let mut parts = vec![json!({
            "name": BOARD_PART,
            "model": BOARD_MODEL,
            "transform": IDENTITY,
            "color": BOARD_COLOR,
            "metallic": BOARD_FINISH.metallic,
            "roughness": BOARD_FINISH.roughness,
            "notes": board_notes,
        })];
        let plated = plating(&board, &step);
        let mut files = vec![(BOARD_FILE.to_string(), step.text.into_bytes())];
        let recognizer = Recognizer::new(&board);
        let placed = components_with(
            &board.components,
            &outcome.models,
            step.thickness,
            &board.free_models,
            Some((&recognizer, bodies)),
        );
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
        notes.extend(plated.notes);
        // The plating takes the top finish; a process we do not know is named once, in a note.
        let (plating_look, _) = pad_look(board.appearance.top_finish.as_ref());
        notes.extend(finish_notes(&board));
        if let Some(text) = plated.text {
            models.push(json!({ "id": PLATING_MODEL, "file": PLATING_FILE }));
            parts.push(json!({
                "name": PLATING_PART,
                "model": PLATING_MODEL,
                "transform": IDENTITY,
                "color": plating_look.color,
                "metallic": plating_look.finish.metallic,
                "roughness": plating_look.finish.roughness,
                "notes": [],
            }));
            files.push((PLATING_FILE.to_string(), text.into_bytes()));
        }
        let (mut overlays, overlay_notes) = match worker {
            Some(worker) => worker.join().unwrap_or_else(|_| render()),
            None => render(),
        };
        if !accept.laminate {
            negotiate_down(&mut overlays);
        }
        notes.extend(overlay_notes);
        let mut manifest = json!({
            "schema": PARTS_SCHEMA,
            "ok": true,
            "name": name,
            "units": "mm",
            "source": { "format": format.id(), "format_name": format.name() },
        });
        if let Some(map) = manifest.as_object_mut() {
            // These arrays already contain JSON values, including every overlay coordinate.
            // Moving them avoids a second complete serialization and allocation pass.
            map.insert("models".into(), Value::Array(models));
            map.insert("parts".into(), Value::Array(parts));
            map.insert("overlays".into(), Value::Array(overlays));
            map.insert("notes".into(), Value::Array(notes));
        }
        Ok(Package { manifest, files })
    })
}

/// Makes the overlays what a loader that knows only `copper`, `silkscreen` and `mask` can draw:
/// no `laminate` overlay, and an opaque mask. With no laminate sheet the mask and the silkscreen
/// over bare laminate lie on the board face, so they drop by the sheet's thickness; those over
/// copper stay on the copper. A flat overlay (no `thickness`) is left where it is.
fn negotiate_down(list: &mut Vec<Value>) {
    list.retain(|o| o["kind"] != "laminate");
    for o in list.iter_mut() {
        let over_laminate = matches!(o["kind"].as_str(), Some("mask" | "silkscreen"))
            && !o["name"]
                .as_str()
                .is_some_and(|n| n.ends_with(" over copper"))
            && o.get("thickness").is_some();
        let sign = match o["side"].as_str() {
            Some("top") => 1.0,
            Some("bottom") => -1.0,
            _ => continue,
        };
        let z = o["z"].as_f64();
        let Some(map) = o.as_object_mut() else {
            continue;
        };
        if let (true, Some(z)) = (over_laminate, z) {
            let z = z - sign * crate::overlay::LAMINATE_THICKNESS;
            map.insert("z".into(), json!((z * 1e6).round() / 1e6));
        }
        if map.get("kind").and_then(Value::as_str) == Some("mask") {
            map.insert("opacity".into(), json!(1.0));
        }
    }
}

/// An Info note for each side whose surface finish process we do not know, whose pads are drawn
/// gold. A side whose process is empty says nothing to name, so has no note. The plating (which
/// takes the top finish) is mentioned only for the top side.
fn finish_notes(board: &Board) -> Vec<Value> {
    let mut notes = Vec::new();
    for (side, finish) in [
        ("top", &board.appearance.top_finish),
        ("bottom", &board.appearance.bottom_finish),
    ] {
        let Some(f) = finish else { continue };
        let process = f.process.trim();
        if process.is_empty() || pad_look(Some(f)).1 {
            continue;
        }
        let plating = if side == "top" {
            " and the plating"
        } else {
            ""
        };
        notes.push(json!({
            "severity": "Info",
            "message": format!(
                "the {side} surface finish process \"{process}\" is not one we know, so the \
                 {side} pads{plating} are drawn gold"
            ),
        }));
    }
    notes
}

/// The plating's STEP file (when there is any) and the notes on it.
struct Plating {
    text: Option<String>,
    notes: Vec<Value>,
}

/// The copper lining the plated holes: one tube for each, from the board's bottom (z 0) to its top
/// (z = `thickness`), in the Board's frame; for overlapping holes cut as one opening, one tube
/// following the union's wall. Plated holes are every via, and every pad whose drill the board
/// model marks `plated`, but only those the board solid was actually cut with (`step`, from
/// `board_step::write`): a tube in a hole that was not cut would float in the air or run through
/// the substrate. A tube's outer radius is the drill radius, its inner one that less
/// [`PLATING_THICKNESS`]. No text when no hole is plated, or when the tubes cannot be written;
/// a hole not cut, too small to line, or past the cap, or a failure, says so in a note.
fn plating(board: &Board, step: &board_step::BoardStep) -> Plating {
    let thickness = step.thickness;
    let wanted = board
        .copper
        .iter()
        .filter(|item| match item {
            CopperItem::Via { .. } => true,
            CopperItem::Pad { drill, plated, .. }
            | CopperItem::PolygonPad { drill, plated, .. } => drill.is_some() && *plated,
            _ => false,
        })
        .count();
    let not_cut = wanted.saturating_sub(step.plated_holes_cut());
    let mut list = Vec::new();
    let mut unions = Vec::new();
    let mut left_out = 0usize;
    for hole in step.holes.iter().filter(|h| h.plated) {
        let inner_radius = hole.radius - PLATING_THICKNESS;
        if inner_radius <= board_step::TOLERANCE || list.len() >= board_step::MAX_TUBES {
            left_out = left_out.saturating_add(hole.also.saturating_add(1));
            continue;
        }
        list.push(board_step::Tube {
            at: hole.at,
            outer_radius: hole.radius,
            inner_radius,
            span: hole.span,
        });
    }
    for merged in step.merged.iter().filter(|m| m.plated) {
        let lined = merged
            .circles
            .iter()
            .all(|(_, radius)| radius - PLATING_THICKNESS > board_step::TOLERANCE);
        if !lined || list.len().saturating_add(unions.len()) >= board_step::MAX_TUBES {
            left_out = left_out.saturating_add(merged.circles.len().saturating_add(merged.also));
            continue;
        }
        let tube = board_step::UnionTube {
            circles: merged.circles.clone(),
            wall: PLATING_THICKNESS,
            span: merged.span,
        };
        // A failed union must not prevent the other openings from receiving copper.
        if board_step::tubes_and_unions(
            &[],
            std::slice::from_ref(&tube),
            0.0,
            thickness,
            PLATING_PART,
        )
        .is_err()
        {
            left_out = left_out.saturating_add(merged.circles.len().saturating_add(merged.also));
            continue;
        }
        unions.push(tube);
    }
    let mut notes = Vec::new();
    if not_cut > 0 {
        notes.push(json!({
            "severity": "Dropped",
            "message": format!(
                "{not_cut} plated drilled hole(s) were not cut into the board, so have no plating"
            ),
        }));
    }
    if left_out > 0 {
        notes.push(json!({
            "severity": "Dropped",
            "message": format!(
                "{left_out} plated drilled hole(s) are too small to line with {PLATING_THICKNESS} mm of \
                 plating, have an outline that cannot be lined, or are past the first {} tubes, and have no plating",
                board_step::MAX_TUBES
            ),
        }));
    }
    if list.is_empty() && unions.is_empty() {
        return Plating { text: None, notes };
    }
    match board_step::tubes_and_unions(&list, &unions, 0.0, thickness, PLATING_PART) {
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

/// `base`, or `base (2)`, `base (3)` ... when taken; the name is then taken.
fn unique_name(used: &mut std::collections::BTreeSet<String>, base: &str) -> String {
    let mut name = base.to_string();
    let mut n = 2usize;
    while used.contains(&name) {
        name = format!("{base} ({n})");
        n = n.saturating_add(1);
    }
    used.insert(name.clone());
    name
}

/// What the bodies of a package share: equal bodies share one model file.
#[derive(Default)]
struct Models {
    /// Embedded model id to model id in the package.
    unpacked: std::collections::BTreeMap<String, String>,
    /// Extruded body (outline and heights) to model id.
    extruded: std::collections::BTreeMap<String, String>,
    /// Sphere radius (as text, exact) to model id.
    spheres: std::collections::BTreeMap<String, String>,
}

/// Why a body could not be placed.
enum Lost {
    Missing,
    External(String),
    Failed(String),
}

/// The model id (in the package) for one body, writing its file the first time.
fn body_model(
    ctx: &mut Models,
    model: &ModelRef,
    embedded: &[EmbeddedModel],
    placed: &mut Placed,
) -> Result<String, Lost> {
    match &model.source {
        ModelSource::Embedded { id } => {
            if let Some(model_id) = ctx.unpacked.get(id.as_str()) {
                return Ok(model_id.clone());
            }
            match embedded.iter().find(|m| &m.id == id) {
                Some(m) if m.step.starts_with(b"ISO-10303-21") => {
                    let model_id = format!("model-{}", placed.models.len().saturating_add(1));
                    let file = format!("{model_id}.step");
                    placed.models.push((model_id.clone(), file, m.step.clone()));
                    ctx.unpacked.insert(m.id.clone(), model_id.clone());
                    Ok(model_id)
                }
                Some(m) => Err(Lost::Failed(format!("{} is not a STEP file", m.name))),
                None => Err(Lost::Missing),
            }
        }
        ModelSource::External { file, .. } => Err(Lost::External(file.clone())),
        ModelSource::Extruded {
            outline,
            standoff,
            height,
        } => {
            let key = format!("{outline:?} {standoff} {height}");
            if let Some(id) = ctx.extruded.get(&key) {
                return Ok(id.clone());
            }
            let id = format!("model-{}", placed.models.len().saturating_add(1));
            let step = board_step::prism(outline, *standoff, *height, &id)
                .map_err(|why| Lost::Failed(why.to_string()))?;
            placed
                .models
                .push((id.clone(), format!("{id}.step"), step.text.into_bytes()));
            ctx.extruded.insert(key, id.clone());
            Ok(id)
        }
        ModelSource::Sphere { center, z, radius } => {
            if !(center.x.is_finite() && center.y.is_finite() && z.is_finite()) {
                return Err(Lost::Failed(format!(
                    "a sphere centered at ({}, {}, {z}) is not at a place",
                    center.x, center.y
                )));
            }
            // One model per radius, at the origin: the part's transform puts it in place.
            let key = format!("{radius:?}");
            if let Some(id) = ctx.spheres.get(&key) {
                return Ok(id.clone());
            }
            let id = format!("model-{}", placed.models.len().saturating_add(1));
            let step =
                board_step::sphere(*radius, &id).map_err(|why| Lost::Failed(why.to_string()))?;
            placed
                .models
                .push((id.clone(), format!("{id}.step"), step.text.into_bytes()));
            ctx.spheres.insert(key, id.clone());
            Ok(id)
        }
    }
}

/// The manifest part of one body: `frame` is the part's frame (or the board's, for a free
/// body) and the body's own offset and rotation go on top of it.
fn body_part(
    model: &ModelRef,
    name: &str,
    id: &str,
    frame: &Transform,
    group: &str,
    notes: &mut Vec<Value>,
) -> Result<Value, Lost> {
    let [rx, ry, rz] = model.rotation_deg;
    let [dx, dy, dz] = model.offset;
    let mut on_part = compose(
        &translation(dx, dy, dz),
        &compose(&turn(2, rz), &compose(&turn(1, ry), &turn(0, rx))),
    );
    // A sphere's file is centered on the origin; its place is part of the body.
    if let ModelSource::Sphere { center, z, .. } = &model.source {
        on_part = compose(&on_part, &translation(center.x, center.y, *z));
    }
    let transform = compose(frame, &on_part);
    if !transform.iter().flatten().all(|number| number.is_finite()) {
        return Err(Lost::Failed("the body's placement is not finite".into()));
    }
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
    let mut part = json!({
        "name": name,
        "model": id,
        "transform": transform,
        "group": group,
        "notes": part_notes,
    });
    // An extruded body or sphere the file colors; an embedded STEP model keeps its own colors.
    if let Some(color) = &model.color {
        if let Some(o) = color
            .opacity
            .filter(|o| !o.is_finite() || !(0.0..=1.0).contains(o))
        {
            notes.push(json!({
                "severity": "Info",
                "message": format!("{name}'s 3D body opacity {o} is not between 0 and 1, so it is drawn opaque"),
            }));
        } else if color.opacity == Some(0.0) {
            notes.push(json!({
                "severity": "Info",
                "message": format!("{name}'s 3D body is fully transparent, as the board file says"),
            }));
        }
    }
    if let (Some(color), Some(map)) = (&model.color, part.as_object_mut()) {
        map.insert("color".into(), json!(hex(color)));
        map.insert(
            "opacity".into(),
            json!(color
                .opacity
                .filter(|o| o.is_finite() && (0.0..=1.0).contains(o))
                .unwrap_or(1.0)),
        );
        map.insert("metallic".into(), json!(BODY_FINISH.metallic));
        map.insert("roughness".into(), json!(BODY_FINISH.roughness));
    }
    Ok(part)
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
#[cfg(test)]
fn components(list: &[Placement], embedded: &[EmbeddedModel], thickness: f64) -> Placed {
    components_with(list, embedded, thickness, &[], None)
}

/// [`components`], and with `standard` (what recognizes a part's package, and which bodies are
/// wanted) a part with no model at all gets the standard body of its package; with
/// [`Bodies::Standard`] so does one whose model is a plain extruded body.
fn components_with(
    list: &[Placement],
    embedded: &[EmbeddedModel],
    thickness: f64,
    free: &[ModelRef],
    standard: Option<(&Recognizer<'_>, Bodies)>,
) -> Placed {
    components_with_limit(list, embedded, thickness, free, standard, MAX_BODIES)
}

/// The package body limit is supplied separately so small tests can reach it.
fn components_with_limit(
    list: &[Placement],
    embedded: &[EmbeddedModel],
    thickness: f64,
    free: &[ModelRef],
    standard: Option<(&Recognizer<'_>, Bodies)>,
    max_bodies: usize,
) -> Placed {
    let mut placed = Placed::default();
    // Equal standard bodies share their two model files.
    let mut standard_models: std::collections::BTreeMap<String, String> = Default::default();
    // Each package, with the parts that got it.
    let mut recognized: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    // The same, for parts whose plain extruded body the standard one replaced.
    let mut replaced: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let mut used =
        std::collections::BTreeSet::from([BOARD_PART.to_string(), PLATING_PART.to_string()]);
    // Equal extruded bodies share one model file; so do parts naming the same embedded model.
    let mut ctx = Models::default();
    let (mut no_model, mut missing, mut external, mut failed) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut bodies_placed = 0usize;
    let mut over_part = Vec::new();
    let mut total_over = 0usize;
    for (index, p) in list.iter().enumerate() {
        let base = match p.designator.trim() {
            "" => format!("Part {}", index.saturating_add(1)),
            d => d.to_string(),
        };
        let name = unique_name(&mut used, &base);
        // A part has no usable model at all when it names none, or only links to files that
        // were not found; a plain extruded body is replaced when `--bodies standard` asks.
        let all_external = p
            .models
            .iter()
            .all(|m| matches!(m.source, ModelSource::External { .. }));
        let extruded_body = !p.models.is_empty()
            && p.models
                .iter()
                .all(|m| matches!(m.source, ModelSource::Extruded { .. }));
        let wants_standard = match standard {
            None => false,
            Some((_, bodies)) => all_external || (extruded_body && bodies == Bodies::Standard),
        };
        if let (true, Some((recognizer, _))) = (wants_standard, standard) {
            if let Some(found) = recognizer.recognize(p) {
                match standard_parts(
                    &name,
                    (p, thickness),
                    (&found, extruded_body),
                    &mut standard_models,
                    &mut placed,
                    &mut used,
                ) {
                    Ok(()) => {
                        let list = if extruded_body {
                            &mut replaced
                        } else {
                            &mut recognized
                        };
                        list.entry(found.package.clone()).or_default().push(name);
                        continue;
                    }
                    // The file's own extruded body is better than none.
                    Err(why) if extruded_body => placed.notes.push(json!({
                        "severity": "Degraded",
                        "message": format!(
                            "{name}: the standard {} body could not be written ({why}); the \
                             board's extruded body is used",
                            found.package
                        ),
                    })),
                    Err(why) => {
                        failed.push(format!("{name}: {why}"));
                        continue;
                    }
                }
            }
        }
        if p.models.is_empty() {
            no_model.push(name);
            continue;
        }
        let group = match p.side {
            Side::Top => GROUP_TOP,
            Side::Bottom => GROUP_BOTTOM,
        };
        let frame = part_frame(p, thickness);
        if p.models.len() > MAX_BODIES_PER_PART {
            over_part.push(format!("{name} ({})", p.models.len()));
        }
        for (k, model) in p.models.iter().take(MAX_BODIES_PER_PART).enumerate() {
            if bodies_placed >= max_bodies {
                total_over = total_over
                    .saturating_add(p.models.len().min(MAX_BODIES_PER_PART).saturating_sub(k));
                if k == 0 {
                    no_model.push(format!(
                        "{name} (main body left out by the package body limit)"
                    ));
                }
                break;
            }
            bodies_placed = bodies_placed.saturating_add(1);
            // The first body is the part itself; the others are "<designator> body 2", ...
            let body_name = if k == 0 {
                name.clone()
            } else {
                unique_name(&mut used, &format!("{name} body {}", k.saturating_add(1)))
            };
            let outcome = body_model(&mut ctx, model, embedded, &mut placed)
                .and_then(|id| body_part(model, &body_name, &id, &frame, group, &mut placed.notes));
            match outcome {
                Ok(part) => placed.parts.push(part),
                Err(Lost::Missing) => missing.push(body_name),
                Err(Lost::External(file)) => external.push(format!("{body_name} ({file})")),
                Err(Lost::Failed(why)) => failed.push(format!("{body_name}: {why}")),
            }
        }
    }
    if !over_part.is_empty() {
        placed.notes.push(json!({
            "severity": "Dropped",
            "message": format!(
                "{} component(s) have more than {MAX_BODIES_PER_PART} 3D bodies; the rest of \
                 each were left out: {}",
                over_part.len(),
                named(&over_part)
            ),
        }));
    }
    // Bodies that belong to no part sit in board coordinates, z up from the board's top face.
    let free_frame = translation(0.0, 0.0, thickness);
    let mut free_over = 0usize;
    for (k, model) in free.iter().enumerate() {
        if k >= MAX_FREE_BODIES || bodies_placed >= max_bodies {
            free_over = free.len().saturating_sub(MAX_FREE_BODIES);
            total_over =
                total_over.saturating_add(free.len().min(MAX_FREE_BODIES).saturating_sub(k));
            break;
        }
        bodies_placed = bodies_placed.saturating_add(1);
        let body_name = unique_name(&mut used, &format!("Free body {}", k.saturating_add(1)));
        let outcome = body_model(&mut ctx, model, embedded, &mut placed).and_then(|id| {
            body_part(
                model,
                &body_name,
                &id,
                &free_frame,
                GROUP_FREE,
                &mut placed.notes,
            )
        });
        match outcome {
            Ok(part) => placed.parts.push(part),
            Err(Lost::Missing) => missing.push(body_name),
            Err(Lost::External(file)) => external.push(format!("{body_name} ({file})")),
            Err(Lost::Failed(why)) => failed.push(format!("{body_name}: {why}")),
        }
    }
    if free_over > 0 {
        placed.notes.push(json!({
            "severity": "Dropped",
            "message": format!(
                "{free_over} free 3D bod(ies) were left out: the package holds at most \
                 {MAX_FREE_BODIES} free bodies and {max_bodies} bodies in all"
            ),
        }));
    }
    if total_over > 0 {
        placed.notes.push(json!({
            "severity": "Dropped",
            "message": format!(
                "{total_over} bodies left out: the package holds at most {max_bodies} bodies"
            ),
        }));
    }
    for (package, parts) in &recognized {
        placed.notes.push(json!({
            "severity": "Info",
            "message": format!(
                "{} component(s) have no usable 3D model; each got a standard {package} body \
                 (recognized from its pads): {}",
                parts.len(),
                named(parts)
            ),
        }));
    }
    for (package, parts) in &replaced {
        placed.notes.push(json!({
            "severity": "Info",
            "message": format!(
                "{} component(s) had a plain extruded body, replaced by a standard {package} \
                 body (--bodies standard): {}",
                parts.len(),
                named(parts)
            ),
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
            "component(s) link to a 3D model file that was not found or not used (see the \
             notes on linked model files) and are not in the package",
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

/// Adds the two models of a standard body (once for equal ones) and its two parts, the body
/// named `name` and the leads "<name> leads" (made unique), in the component's group.
fn standard_parts(
    name: &str,
    (p, thickness): (&Placement, f64),
    (found, replaces_extruded): (&Standard, bool),
    models: &mut std::collections::BTreeMap<String, String>,
    placed: &mut Placed,
    used: &mut std::collections::BTreeSet<String>,
) -> Result<(), String> {
    // Write both models before adding either, so a failure leaves nothing behind.
    let mut ids: Vec<String> = Vec::new();
    let mut fresh: Vec<(String, String, Vec<u8>)> = Vec::new();
    for (solids, what) in [(&found.body, "body"), (&found.leads, "leads")] {
        let key = format!("{solids:?}");
        let cached = models.get(&key).cloned().or_else(|| {
            fresh
                .iter()
                .find(|(k, ..)| *k == key)
                .map(|(_, id, _)| id.clone())
        });
        let id = match cached {
            Some(id) => id,
            None => {
                let id = format!(
                    "model-{}",
                    placed
                        .models
                        .len()
                        .saturating_add(fresh.len())
                        .saturating_add(1)
                );
                let step = board_step::solids(solids, &id, &format!("Standard {what}"))
                    .map_err(|e| e.to_string())?;
                fresh.push((key, id.clone(), step.text.into_bytes()));
                id
            }
        };
        ids.push(id);
    }
    for (key, id, bytes) in fresh {
        placed
            .models
            .push((id.clone(), format!("{id}.step"), bytes));
        models.insert(key, id);
    }
    let (Some(body_id), Some(leads_id)) = (ids.first(), ids.get(1)) else {
        return Err("no model ids".into());
    };
    let mut leads_name = format!("{name} leads");
    let mut n = 2usize;
    while used.contains(&leads_name) {
        leads_name = format!("{name} leads ({n})");
        n = n.saturating_add(1);
    }
    used.insert(leads_name.clone());
    let group = match p.side {
        Side::Top => GROUP_TOP,
        Side::Bottom => GROUP_BOTTOM,
    };
    let transform = part_frame(p, thickness);
    let origin = if replaces_extruded {
        "replacing the board's plain extruded body (--bodies standard)"
    } else {
        "no usable 3D model in the file"
    };
    for (part, model, look, what) in [
        (name, body_id, found.body_look, "body"),
        (leads_name.as_str(), leads_id, found.leads_look, "leads"),
    ] {
        let mut notes = vec![json!({
            "severity": "Info",
            "message": format!(
                "standard {what} of a {}, recognized from the pads ({origin})",
                found.package
            ),
        })];
        for extra in &found.notes {
            notes.push(json!({
                "severity": "Info",
                "message": format!("{}: {extra}", found.package),
            }));
        }
        placed.parts.push(json!({
            "name": part,
            "model": model,
            "transform": transform,
            "group": group,
            "color": look.color,
            "opacity": 1.0,
            "metallic": BODY_FINISH.metallic,
            "roughness": look.roughness,
            "notes": notes,
        }));
    }
    Ok(())
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
            models: vec![ModelRef {
                name: String::new(),
                source: ModelSource::Extruded {
                    outline: rect(-0.8, -0.4, 0.8, 0.4),
                    standoff: 0.0,
                    height: 0.45,
                },
                offset: [0.0; 3],
                rotation_deg: [0.0; 3],
                color: None,
            }],
            comment: String::new(),
            fitted: true,
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
        // the off-center box (x 0 to 1.6 in the model) spans x 23.8 to 25.4, projecting down.
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
        if let Some(m) = r1.models.first_mut() {
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
        none.models.clear();
        let mut embedded = part("U1", 0.0, 0.0, 0.0, Side::Top);
        let mut external = part("U2", 0.0, 0.0, 0.0, Side::Top);
        let mut flat = part("U3", 0.0, 0.0, 0.0, Side::Top);
        if let (Some(e), Some(x), Some(f)) = (
            embedded.models.first_mut(),
            external.models.first_mut(),
            flat.models.first_mut(),
        ) {
            e.source = ModelSource::Embedded { id: "{M}".into() };
            x.source = ModelSource::External {
                file: "soic8.step".into(),
                checksum: None,
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
        assert!(messages[2].ends_with(
            "link to a 3D model file that was not found or not used (see the notes on linked \
             model files) and are not in the package: U2 (soic8.step)"
        ));
        assert!(messages[3].contains("U3: cannot make the board solid"));
    }

    #[test]
    fn an_embedded_model_is_written_once_for_every_part_that_uses_it() {
        let mut r1 = part("R1", 10.0, 0.0, 0.0, Side::Top);
        let mut r2 = part("R2", 20.0, 0.0, 0.0, Side::Top);
        let mut r3 = part("R3", 30.0, 0.0, 0.0, Side::Top);
        for (p, id) in [(&mut r1, "A"), (&mut r2, "A"), (&mut r3, "B")] {
            if let Some(m) = p.models.first_mut() {
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
        build_laminate(Path::new(&path), &bytes, crate::overlay::Copper::Outer)
    }

    /// The package of a loader that accepts every overlay kind.
    #[cfg(feature = "altium")]
    fn build_laminate(path: &Path, bytes: &[u8], copper: crate::overlay::Copper) -> Package {
        build_accepting(
            path,
            bytes,
            &[],
            &[],
            copper,
            Bodies::default(),
            Accept { laminate: true },
        )
        .unwrap()
    }

    #[cfg(feature = "altium")]
    #[test]
    fn without_accept_overlay_there_is_no_laminate_and_the_mask_is_opaque() {
        let path = format!(
            "{}/../altium-pcbdoc/tests/boards/one-blue-mask-resistor.PcbDoc",
            env!("CARGO_MANIFEST_DIR")
        );
        let bytes = std::fs::read(&path).unwrap();
        let kinds = |p: &Package| -> Vec<String> {
            p.manifest["overlays"]
                .as_array()
                .unwrap()
                .iter()
                .map(|o| o["kind"].as_str().unwrap().to_string())
                .collect()
        };
        let old = build(
            Path::new(&path),
            &bytes,
            &[],
            &[],
            crate::overlay::Copper::Outer,
        )
        .unwrap();
        assert!(!kinds(&old).contains(&"laminate".to_string()));
        assert!(kinds(&old).contains(&"mask".to_string()));
        for o in old.manifest["overlays"].as_array().unwrap() {
            if o["kind"] == "mask" {
                assert_eq!(o["opacity"], 1.0, "{o}");
            }
        }
        let new = build_laminate(Path::new(&path), &bytes, crate::overlay::Copper::Outer);
        assert_eq!(kinds(&new).iter().filter(|k| *k == "laminate").count(), 2);
        for o in new.manifest["overlays"].as_array().unwrap() {
            if o["kind"] == "mask" {
                let opacity = o["opacity"].as_f64().unwrap();
                assert!((0.75..1.0).contains(&opacity), "{o}");
            }
        }
        // With no laminate sheet the coat and ink over bare laminate lie on the board face (5 um
        // lower); those over copper stay on the copper. Same areas, same thickness.
        for (o, n) in old.manifest["overlays"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| matches!(o["kind"].as_str(), Some("mask" | "silkscreen")))
            .filter_map(|o| {
                let n = new.manifest["overlays"]
                    .as_array()?
                    .iter()
                    .find(|n| n["name"] == o["name"])?;
                Some((o, n))
            })
        {
            let drop = if o["name"].as_str().unwrap().ends_with(" over copper") {
                0.0
            } else if o["side"] == "top" {
                0.005
            } else {
                -0.005
            };
            let (zo, zn) = (o["z"].as_f64().unwrap(), n["z"].as_f64().unwrap());
            assert!((zn - zo - drop).abs() < 1e-9, "{o} {n}");
            assert_eq!(o["areas"], n["areas"]);
            assert_eq!(o["thickness"], n["thickness"]);
        }
        // Everything else is the same.
        let others = |p: &Package| -> Vec<String> {
            kinds(p).into_iter().filter(|k| k != "laminate").collect()
        };
        assert_eq!(others(&old), others(&new));
    }

    #[test]
    fn accept_takes_laminate_and_refuses_the_rest_listing_what_is_accepted() {
        let mut a = Accept::default();
        assert!(!a.laminate);
        a.accept("laminate").unwrap();
        a.accept("laminate").unwrap();
        assert!(a.laminate);
        let err = a.accept("glass").unwrap_err();
        assert!(err.contains("glass") && err.contains("laminate"), "{err}");
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

    /// all-features after thinning (#126): every overlay is still a valid multipolygon (rings that
    /// do not cross, holes inside, areas that do not touch), and the silkscreen, which had 15116
    /// points before thinning, keeps fewer than 8000.
    #[cfg(feature = "altium")]
    #[test]
    fn thinned_overlays_stay_valid_and_the_silkscreen_loses_most_points() {
        use geo::Validation;
        let package = package_of("all-features");
        let ring = |r: &Value| {
            geo::LineString::from(
                r.as_array()
                    .unwrap()
                    .iter()
                    .map(|q| (q[0].as_f64().unwrap(), q[1].as_f64().unwrap()))
                    .collect::<Vec<_>>(),
            )
        };
        let mut silk_points = 0;
        for o in package.manifest["overlays"].as_array().unwrap() {
            let polygons: Vec<geo::Polygon> = o["areas"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| {
                    geo::Polygon::new(
                        ring(&a["outer"]),
                        a["holes"].as_array().unwrap().iter().map(ring).collect(),
                    )
                })
                .collect();
            let shape = geo::MultiPolygon::new(polygons);
            assert!(
                shape.is_valid(),
                "{}: {:?}",
                o["name"],
                shape.validation_errors()
            );
            if o["kind"] == "silkscreen" {
                silk_points += shape
                    .iter()
                    .map(|p| {
                        p.interiors().iter().map(|h| h.0.len() - 1).sum::<usize>()
                            + p.exterior().0.len()
                            - 1
                    })
                    .sum::<usize>();
            }
        }
        assert!(silk_points > 0 && silk_points < 8000, "{silk_points}");
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
            let package = build_laminate(&path, &bytes, crate::overlay::Copper::Outer);
            // The board's thickness T: the top face of its STEP solid.
            let (_, step) = package
                .files
                .iter()
                .find(|(name, _)| name == BOARD_FILE)
                .unwrap();
            let t = point_heights(&String::from_utf8(step.clone()).unwrap())
                .into_iter()
                .fold(f64::MIN, f64::max);
            for p in package.manifest["parts"].as_array().unwrap() {
                // The board and plating stay at the top level; each component is in its side's
                // folder, and a body of no part in the free bodies' folder.
                let want = match p["name"].as_str().unwrap() {
                    BOARD_PART | PLATING_PART => None,
                    _ => Some([GROUP_TOP, GROUP_BOTTOM, GROUP_FREE]),
                };
                match want {
                    None => assert!(p.get("group").is_none(), "{}: {p}", path.display()),
                    Some(groups) => assert!(
                        groups.iter().any(|g| p["group"] == *g),
                        "{}: {p}",
                        path.display()
                    ),
                }
            }
            for o in package.manifest["overlays"].as_array().unwrap() {
                assert_eq!(o["part"], "Board");
                assert_eq!(o["group"], GROUP_LAYERS);
                assert!(overlay_area(o) > 0.0, "{}: {o}", path.display());
                let z = o["z"].as_f64().unwrap();
                // Laminate and copper start at the face, the finish on top of the copper, the mask
                // and silkscreen as below, and the bottom
                // side mirrors the top below z = 0.
                let name = o["name"].as_str().unwrap();
                let (offset, thickness) = match o["kind"].as_str().unwrap() {
                    "laminate" => (0.0, 0.005),
                    "copper" if name.starts_with("Finish") => (0.035, 0.004),
                    "copper" => (0.0, 0.035),
                    // The mask coat lies on the laminate's top or the copper's top; the
                    // silkscreen on top of the coat under it.
                    "mask" if name.ends_with(" over copper") => (0.005, 0.040),
                    "mask" => (0.005, 0.010),
                    _ if name.ends_with(" over copper") => (0.045, 0.015),
                    _ => (0.015, 0.015),
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
                    // The file's own opacity, at least 0.75, or 0.75 where it gives none.
                    let opacity = o["opacity"].as_f64().unwrap();
                    assert!(opacity > 0.0 && opacity <= 1.0, "{}: {o}", path.display());
                } else {
                    assert!(o.get("opacity").is_none(), "{}: {o}", path.display());
                }
            }
            // Every test board has a usable outline, so each side has a mask over laminate (a
            // board with no bare laminate left at all could lack it, none of ours does).
            let masks = package.manifest["overlays"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["kind"] == "mask" && !o["name"].as_str().unwrap().ends_with("copper"))
                .count();
            assert_eq!(masks, 2, "{}", path.display());
            let laminates = package.manifest["overlays"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["kind"] == "laminate")
                .count();
            assert_eq!(laminates, 2, "{}", path.display());
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
    fn the_solder_mask_is_the_board_less_openings_and_drills() {
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
        // one-resistor: two bare SMD pads of 0.8 x 1.0 on the top, each opened by the board's
        // mask expansion rule of 0.1 mm a side: (0.8 + 0.2) * (1.0 + 0.2) = 1.2 mm2. The top mask
        // is 1500 - 2 * 1.2 = 1497.6; the bottom has no pads, so it stays 1500.
        assert!((mask("one-resistor", "top") - 1497.6).abs() < 1e-3);
        assert!((mask("one-resistor", "bottom") - 1500.0).abs() < 1e-3);
        // one-via: the file says the via is open on both sides (Altium's older boards do not tent
        // them), so each face has an opening of the via's 1.0 mm diameter grown by the rule's
        // 0.1 mm: radius 0.6, pi * 0.36 = 1.13097, so the mask is 1500 - 1.13097 = 1498.86903
        // (less a little for the polygon, so the tolerance is 0.02).
        let open_via = 1500.0 - std::f64::consts::PI * 0.6 * 0.6;
        assert!((mask("one-via", "top") - open_via).abs() < 0.02);
        assert!((mask("one-via", "bottom") - open_via).abs() < 0.02);
        // one-plated-pad: a round 2.0 mm pad on all layers with a 1.0 mm hole. Its opening is the
        // pad grown by the board's mask expansion rule of 0.1 mm to a radius of 1.1, on both
        // faces: 1500 - pi * 1.1^2 = 1496.19867, less a little for the 32-sided polygon
        // (16 * 1.1^2 * sin(11.25 deg) = 3.77694 mm2 against 3.80133), so the tolerance is 0.04.
        let open = 1500.0 - std::f64::consts::PI * 1.1 * 1.1;
        assert!((mask("one-plated-pad", "top") - open).abs() < 0.04);
        assert!((mask("one-plated-pad", "bottom") - open).abs() < 0.04);
    }

    #[cfg(feature = "altium")]
    #[test]
    fn the_mask_opens_each_pad_by_the_boards_expansion_rule() {
        // The mask-expansion boards are one-resistor (two bare SMD pads of 0.8 x 1.0 on the top)
        // with the rule at 0, 0.05, 0.15 and 0.2 mm: each pad's opening is (0.8 + 2e) * (1.0 + 2e)
        // on the 1500 mm2 outline. Altium stores the rule in mil, so allow 1e-3.
        for (board, e) in [
            ("mask-expansion-0", 0.0),
            ("mask-expansion-0-05", 0.05),
            ("mask-expansion-0-15", 0.15),
            ("mask-expansion-0-2", 0.2),
        ] {
            let package = package_of(board);
            // The mask is the part over bare laminate and the part over copper, which never
            // overlap.
            let top = overlay_area(overlay_named(&package.manifest, board, "Solder mask top"))
                + package.manifest["overlays"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|o| o["name"] == "Solder mask top over copper")
                    .map(overlay_area)
                    .sum::<f64>();
            let want = 1500.0 - 2.0 * (0.8 + 2.0 * e) * (1.0 + 2.0 * e);
            assert!((top - want).abs() < 1e-3, "{board}: {top}, expected {want}");
        }
    }

    #[cfg(feature = "altium")]
    #[test]
    fn the_mask_cuts_the_boards_cutout_and_hole() {
        // outline-with-hole: a 10 x 10 cutout and a 3.0 mm unplated hole. The hole's pad is bare
        // copper (a ring of no width), so the hole is open all the same, and the opening grows by
        // the board's mask expansion rule of 0.1 mm to a radius of 1.6: 1500 - 100 - pi * 1.6^2
        // = 1391.95752. The opening is a 32-sided polygon, a little under the circle (16 *
        // 1.6^2 * sin(11.25 deg) = 7.99089 mm2 against 8.04248), so the tolerance is 0.08.
        let package = package_of("outline-with-hole");
        let top = overlay_named(&package.manifest, "outline-with-hole", "Solder mask top");
        let area = overlay_area(top);
        let want = 1400.0 - std::f64::consts::PI * 1.6 * 1.6;
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
    fn overlays_and_the_board_part_carry_their_colors() {
        let package = package_of("one-plated-pad");
        let m = &package.manifest;
        let color = |name: &str| overlay_named(m, "one-plated-pad", name)["color"].clone();
        for side in ["top", "bottom"] {
            assert_eq!(color(&format!("Solder mask {side}")), "#247832");
            assert_eq!(color(&format!("Finish {side}")), "#d4af37");
        }
        assert_eq!(part_named(m, "Board")["color"], "#c8b27a");
        assert_eq!(part_named(m, "Board")["metallic"], 0.0);
        assert_eq!(part_named(m, "Board")["roughness"], 0.9);
        assert_eq!(
            overlay_named(m, "one-plated-pad", "Finish top")["roughness"],
            0.45
        );
        let mask = overlay_named(m, "one-plated-pad", "Solder mask top");
        let opacity = mask["opacity"].as_f64().unwrap();
        assert!(opacity > 0.0 && opacity <= 1.0, "{opacity}");
        assert_eq!(
            (&mask["metallic"], &mask["roughness"]),
            (&json!(0.0), &json!(0.35))
        );
        assert_eq!(mask["kind"], "mask");
        // The board is 1.6 mm thick (the file stores 1.600002 after its unit conversion).
        // A coat on the laminate's top: 1.6 + 0.005.
        assert!((mask["z"].as_f64().unwrap() - 1.605).abs() < 1e-4);
        let bottom = overlay_named(m, "one-plated-pad", "Solder mask bottom");
        assert_eq!(bottom["z"].as_f64().unwrap(), -0.005);
        // Copper is copper-colored whatever the mask, over a dark laminate sheet.
        let track = package_of("one-track");
        assert_eq!(
            overlay_named(&track.manifest, "one-track", "Copper top")["color"],
            "#b87333"
        );
        let laminate = overlay_named(&track.manifest, "one-track", "Laminate top");
        assert_eq!(laminate["kind"], "laminate");
        assert_eq!(laminate["color"], "#2a2f1c");
        assert!(laminate.get("opacity").is_none());
        let silk = package_of("one-overlay-line");
        assert_eq!(
            overlay_named(&silk.manifest, "one-overlay-line", "Silkscreen top")["color"],
            "#fafafa"
        );
        // An extruded body has the file's color (Altium's default gray, 128, 128, 128, opaque).
        let resistor = package_of("one-resistor");
        let r1 = part_named(&resistor.manifest, "R1");
        assert_eq!(
            (&r1["color"], &r1["opacity"]),
            (&json!("#808080"), &json!(1.0))
        );
    }

    #[cfg(feature = "altium")]
    #[test]
    fn the_files_mask_and_silkscreen_colors_win() {
        // README.txt, sixteenth batch: mask 17, 85, 204 and silkscreen 255, 255, 0 on both sides.
        let blue = package_of("one-blue-mask-resistor");
        for side in ["top", "bottom"] {
            let mask = overlay_named(&blue.manifest, "blue", &format!("Solder mask {side}"));
            assert_eq!(mask["color"], "#1155cc");
            // Altium's mask opacity for this board (README.txt) is 0.6, raised to the 0.75 floor.
            assert_eq!(mask["opacity"], 0.75);
        }
        // Those two boards have no track and no silkscreen line, so the copper under the mask
        // and the silkscreen are drawn from the same boards with one-track's copper and
        // one-overlay-line's line put on them.
        let read = |name: &str| {
            let path = format!(
                "{}/../altium-pcbdoc/tests/boards/{name}.PcbDoc",
                env!("CARGO_MANIFEST_DIR")
            );
            let bytes = std::fs::read(path).unwrap();
            let format = crate::detect(&bytes, Some("PcbDoc")).unwrap();
            format.read(&bytes).unwrap().board
        };
        let track = read("one-track");
        let line = read("one-overlay-line");
        assert!(!track.copper.is_empty() && !line.graphics.is_empty());
        let mut blue_board = read("one-blue-mask-resistor");
        blue_board.copper = track.copper.clone();
        let (overlays, _) = crate::overlay::overlays(
            &blue_board,
            &[],
            1.6,
            BOARD_PART,
            crate::overlay::Copper::Outer,
        );
        let find = |list: &[Value], name: &str| {
            list.iter()
                .find(|o| o["name"] == name)
                .unwrap_or_else(|| panic!("no {name}"))
                .clone()
        };
        // Copper keeps its own color under any mask.
        assert_eq!(find(&overlays, "Copper top")["color"], "#b87333");
        let mut yellow_board = read("one-yellow-silkscreen-resistor");
        yellow_board.graphics = line.graphics.clone();
        let (overlays, _) = crate::overlay::overlays(
            &yellow_board,
            &[],
            1.6,
            BOARD_PART,
            crate::overlay::Copper::Outer,
        );
        assert_eq!(find(&overlays, "Silkscreen top")["color"], "#ffff00");
        // Silkscreen colors stay at the file's other values on the blue board's side.
        blue_board.graphics = line.graphics;
        let (overlays, _) = crate::overlay::overlays(
            &blue_board,
            &[],
            1.6,
            BOARD_PART,
            crate::overlay::Copper::Outer,
        );
        assert_eq!(find(&overlays, "Silkscreen top")["color"], "#fafafa");
    }

    #[cfg(feature = "altium")]
    #[test]
    fn the_finish_process_colors_the_bare_pads() {
        // README.txt, seventeenth batch: ENIG on the one board, HASL on the other. The top side
        // of one-resistor holds its two SMD pads; the bottom has none to draw.
        let enig = package_of("one-enig-resistor");
        let hasl = package_of("one-hasl-resistor");
        let gold = overlay_named(&enig.manifest, "enig", "Finish top");
        assert_eq!(gold["color"], "#d4af37");
        assert_eq!(gold["roughness"], 0.45);
        assert_eq!(gold["metallic"], 0.0);
        let silver = overlay_named(&hasl.manifest, "hasl", "Finish top");
        assert_eq!(silver["color"], "#c9ccd1");
        assert_eq!(silver["roughness"], 0.4);
        assert_eq!(silver["metallic"], 0.0);
        let notes = hasl.manifest["notes"].as_array().unwrap();
        assert!(notes.iter().all(|n| !n["message"]
            .as_str()
            .unwrap_or_default()
            .contains("surface finish")));
    }

    fn finish(process: &str) -> Option<board_model::SurfaceFinish> {
        Some(board_model::SurfaceFinish {
            process: process.into(),
            material: String::new(),
            color: None,
            thickness: None,
        })
    }

    #[test]
    fn an_unknown_finish_is_noted_per_side_and_an_empty_one_is_not() {
        let mut board = Board::default();
        board.appearance.top_finish = finish("Hard Gold");
        board.appearance.bottom_finish = finish("  ");
        let notes = finish_notes(&board);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert_eq!(notes[0]["severity"], "Info");
        assert_eq!(
            notes[0]["message"],
            "the top surface finish process \"Hard Gold\" is not one we know, so the top pads \
             and the plating are drawn gold"
        );
        // An unknown bottom finish does not touch the plating, which takes the top finish.
        board.appearance.top_finish = finish("ENIG");
        board.appearance.bottom_finish = finish(" Flash Gold ");
        let notes = finish_notes(&board);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert_eq!(
            notes[0]["message"],
            "the bottom surface finish process \"Flash Gold\" is not one we know, so the bottom \
             pads are drawn gold"
        );
        board.appearance.bottom_finish = finish("HASL");
        assert!(finish_notes(&board).is_empty());
    }

    #[test]
    fn a_fully_transparent_body_keeps_its_opacity_and_is_noted() {
        let mut list = [
            part("R1", 10.0, 10.0, 0.0, Side::Top),
            part("R2", 20.0, 10.0, 0.0, Side::Top),
        ];
        for (p, opacity) in list.iter_mut().zip([0.0, 0.5]) {
            if let Some(m) = p.models.first_mut() {
                m.color = Some(board_model::Color {
                    r: 1,
                    g: 2,
                    b: 3,
                    opacity: Some(opacity),
                });
            }
        }
        let placed = components(&list, &[], 1.6);
        assert_eq!(placed.parts[0]["opacity"], 0.0);
        assert_eq!(placed.parts[1]["opacity"], 0.5);
        let messages: Vec<&str> = placed
            .notes
            .iter()
            .filter_map(|n| n["message"].as_str())
            .collect();
        assert_eq!(
            messages,
            ["R1's 3D body is fully transparent, as the board file says"]
        );
        assert_eq!(placed.notes[0]["severity"], "Info");
    }

    #[cfg(feature = "altium")]
    #[test]
    fn a_bottom_only_finish_colors_only_the_bottom_pads() {
        // one-bottom-resistor has its pads on the bottom; give that side HASL and the top nothing.
        let path = format!(
            "{}/../altium-pcbdoc/tests/boards/one-bottom-resistor.PcbDoc",
            env!("CARGO_MANIFEST_DIR")
        );
        let bytes = std::fs::read(path).unwrap();
        let mut board = crate::detect(&bytes, Some("PcbDoc"))
            .unwrap()
            .read(&bytes)
            .unwrap()
            .board;
        board.appearance.top_finish = None;
        board.appearance.bottom_finish = finish("HASL");
        let (overlays, _) =
            crate::overlay::overlays(&board, &[], 1.6, BOARD_PART, crate::overlay::Copper::Outer);
        let pads = overlays
            .iter()
            .find(|o| o["name"] == "Finish bottom")
            .unwrap();
        assert_eq!(pads["color"], "#c9ccd1");
        assert_eq!(pads["roughness"], 0.4);
        assert!(overlays.iter().all(|o| o["name"] != "Finish top"));
        // The same board with no finish keeps the gold default.
        board.appearance.bottom_finish = None;
        let (overlays, _) =
            crate::overlay::overlays(&board, &[], 1.6, BOARD_PART, crate::overlay::Copper::Outer);
        let pads = overlays
            .iter()
            .find(|o| o["name"] == "Finish bottom")
            .unwrap();
        assert_eq!(pads["color"], "#d4af37");
    }

    #[cfg(feature = "altium")]
    #[test]
    fn a_colored_extruded_body_gives_its_part_color_and_opacity() {
        // README.txt: R1's body is 32, 128, 224 at opacity 0.5.
        let package = package_of("one-colored-body-resistor");
        let r1 = part_named(&package.manifest, "R1");
        assert_eq!(r1["color"], "#2080e0");
        assert_eq!(r1["opacity"], 0.5);
        assert_eq!(r1["metallic"], 0.0);
        assert_eq!(r1["roughness"], 0.6);
        // An embedded STEP model keeps its own colors.
        let step = package_of("one-step-resistor");
        assert!(part_named(&step.manifest, "R1").get("color").is_none());
        assert!(part_named(&step.manifest, "R1").get("opacity").is_none());
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
        // The file says the via is open: the overlays draw its ring with the drill open, pi * (0.5^2
        // - 0.25^2) = 0.58905 mm2 with one hole, and the board keeps its drill: a 0.25 mm cylinder
        // in board.step.
        let top = overlay_named(m, "one-via", "Copper top");
        assert_eq!(top["areas"][0]["holes"].as_array().unwrap().len(), 1);
        assert!((overlay_area(top) - 0.589).abs() < 0.01);
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

    #[cfg(feature = "altium")]
    #[test]
    fn a_through_hole_header_gets_its_step_model_and_four_plated_holes() {
        // one-step-header: J1 at (25, 15) holds header-1x4.step (x -1.27 to 1.27, y -5.08 to
        // 5.08, z -3 to 8.5: the pins reach through the 1.6 mm board and 1.4 mm below it), and
        // its four pads have plated 1.0 mm holes.
        let package = package_of("one-step-header");
        let m = &package.manifest;
        let j1 = part_named(m, "J1");
        assert!(j1.get("color").is_none());
        let (_, step) = package
            .files
            .iter()
            .find(|(name, _)| {
                m["models"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|x| x["id"] == j1["model"] && x["file"] == name.as_str())
            })
            .unwrap();
        let step = String::from_utf8(step.clone()).unwrap();
        assert_eq!(step.matches("MANIFOLD_SOLID_BREP(").count(), 5);
        let (lo, hi) = placed_box(&j1["transform"], [-1.27, -5.08, -3.0], [1.27, 5.08, 8.5]);
        assert!(
            near(lo, [23.73, 9.92, -1.4]) && near(hi, [26.27, 20.08, 10.1]),
            "{lo:?} {hi:?}"
        );
        let (_, plating) = package
            .files
            .iter()
            .find(|(name, _)| name == PLATING_FILE)
            .unwrap();
        let radii = cylinder_radii(&String::from_utf8(plating.clone()).unwrap());
        // Each tube is written as two halves, outside and inside, as on one-plated-pad.
        assert_eq!(radii.len(), 16, "{radii:?}");
        assert!(
            radii
                .iter()
                .all(|r| (r - 0.5).abs() < 1e-3 || (r - 0.475).abs() < 1e-3),
            "{radii:?}"
        );
    }

    #[cfg(feature = "altium")]
    #[test]
    fn inner_copper_layers_are_drawn_only_when_asked_for() {
        // four-layer: a track on Layer 1 from (10, 10) to (20, 10) and one on Layer 2 from
        // (10, 20) to (20, 20), both 0.25 wide, and a via at (30, 15), 0.6 across with a 0.3 hole.
        let path = format!(
            "{}/../altium-pcbdoc/tests/boards/four-layer.PcbDoc",
            env!("CARGO_MANIFEST_DIR")
        );
        let bytes = std::fs::read(&path).unwrap();
        let read = |copper| build(Path::new(&path), &bytes, &[], &[], copper).unwrap();
        let messages = |m: &Value| -> Vec<String> {
            m["notes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n["message"].as_str().unwrap().to_string())
                .collect()
        };
        let inner_overlays = |m: &Value| -> Vec<Value> {
            m["overlays"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["side"] == "inner")
                .cloned()
                .collect()
        };
        // By default the inner layers are left out, as before, with the note.
        let outer = read(crate::overlay::Copper::Outer);
        assert!(inner_overlays(&outer.manifest).is_empty());
        assert!(messages(&outer.manifest)
            .iter()
            .any(|m| m.contains("on inner layers are left out")));
        // Asked for, each inner layer is an overlay at its place in the stack.
        let all = read(crate::overlay::Copper::All);
        let m = &all.manifest;
        assert!(!messages(m)
            .iter()
            .any(|m| m.contains("on inner layers are left out")));
        let inner = inner_overlays(m);
        let names: Vec<&str> = inner.iter().map(|o| o["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["Copper Layer 1", "Copper Layer 2"]);
        let t = 1.6 + 2.0 * 0.035 + 2.0 * 2.8 * 0.0254;
        let mil = 2.8 * 0.0254;
        for (o, depth, track_y) in [
            (&inner[0], 0.010 + 0.035 + mil, 10.0),
            (&inner[1], 0.010 + 0.035 + mil + 0.035 + 1.510, 20.0),
        ] {
            assert_eq!(o["kind"], "copper");
            assert_eq!(o["color"], crate::colors::COPPER_COLOR);
            let z = o["z"].as_f64().unwrap();
            assert!((z - (t - depth - 0.035)).abs() < 0.001, "{o}");
            assert!((o["thickness"].as_f64().unwrap() - 0.035).abs() < 0.001);
            // The track and the via's ring, the via's hole cut out of the ring.
            let areas = o["areas"].as_array().unwrap();
            assert_eq!(areas.len(), 2, "{o}");
            let ys = |a: &Value| -> (f64, f64) {
                let pts = a["outer"].as_array().unwrap();
                let y: Vec<f64> = pts.iter().map(|p| p[1].as_f64().unwrap()).collect();
                (
                    y.iter().copied().fold(f64::MAX, f64::min),
                    y.iter().copied().fold(f64::MIN, f64::max),
                )
            };
            assert!(areas.iter().any(|a| {
                let (lo, hi) = ys(a);
                (lo - (track_y - 0.125)).abs() < 0.001 && (hi - (track_y + 0.125)).abs() < 0.001
            }));
            assert!(areas
                .iter()
                .any(|a| a["holes"].as_array().is_some_and(|h| h.len() == 1)));
        }
        // The outer copper is the same either way.
        let others = |m: &Value| -> Vec<Value> {
            m["overlays"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["side"] != "inner")
                .cloned()
                .collect()
        };
        assert_eq!(others(&outer.manifest), others(m));
    }

    #[test]
    fn planes_are_drawn_with_their_clearances_and_reliefs_but_no_via_pads() {
        // four-layer-planes: Layer 1 (GND) is one region, Layer 2 is split at x = 25 into a VCC
        // half and a half without a net. Vias at (10, 10) GND, (10, 20) VCC and (35, 20) without
        // a net, and a GND pad at (40, 10).
        let path = format!(
            "{}/../altium-pcbdoc/tests/boards/four-layer-planes.PcbDoc",
            env!("CARGO_MANIFEST_DIR")
        );
        let bytes = std::fs::read(&path).unwrap();
        let read = |copper| build(Path::new(&path), &bytes, &[], &[], copper).unwrap();
        let inner = |m: &Value| -> Vec<Value> {
            m["overlays"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["side"] == "inner")
                .cloned()
                .collect()
        };
        assert!(inner(&read(crate::overlay::Copper::Outer).manifest).is_empty());
        let all = read(crate::overlay::Copper::All);
        let planes = inner(&all.manifest);
        let names: Vec<&str> = planes.iter().map(|o| o["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["Copper Layer 1", "Copper Layer 2"]);
        let holes = |o: &Value| -> Vec<usize> {
            let mut h: Vec<usize> = o["areas"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a["holes"].as_array().unwrap().len())
                .collect();
            h.sort_unstable();
            h
        };
        // Layer 1 is one piece of copper: two reliefs of four pieces of air each, two clearances,
        // and the holes of the GND via and pad inside their reliefs. No via keeps a ring of its
        // own inside its clearance.
        assert_eq!(holes(&planes[0]), [12], "{}", planes[0]);
        // Layer 2: the VCC half has the VCC via's relief, its hole and the GND via's clearance;
        // the other half only the clearances of the via and the pad on it.
        assert_eq!(holes(&planes[1]), [2, 6], "{}", planes[1]);
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

    #[test]
    fn components_are_grouped_by_side() {
        let placed = components(
            &[
                part("R1", 1.0, 1.0, 0.0, Side::Top),
                part("R2", 2.0, 1.0, 0.0, Side::Bottom),
            ],
            &[],
            1.6,
        );
        let groups: Vec<(&str, &str)> = placed
            .parts
            .iter()
            .map(|p| (p["name"].as_str().unwrap(), p["group"].as_str().unwrap()))
            .collect();
        assert_eq!(
            groups,
            vec![("R1", "Components top"), ("R2", "Components bottom")]
        );
    }

    fn cut(x: f64, radius: f64, plated: bool) -> board_step::CutHole {
        board_step::CutHole {
            at: Point { x, y: 1.0 },
            radius,
            plated,
            span: None,
            also: 0,
        }
    }

    /// A board solid that was cut with exactly these holes, 1.6 mm thick.
    fn solid_with(holes: Vec<board_step::CutHole>) -> board_step::BoardStep {
        board_step::BoardStep {
            text: String::new(),
            notes: Vec::new(),
            stats: board_step::Stats::default(),
            thickness: 1.6,
            holes,
            merged: Vec::new(),
        }
    }

    #[cfg(feature = "kicad")]
    #[test]
    fn board_info_and_dropped_notes_reach_the_manifest() {
        let bytes = br#"(kicad_pcb (version 20241229) (generator "pcbnew")
          (general (thickness 1.6))
          (layers (0 "F.Cu" signal) (2 "B.Cu" signal) (25 "Edge.Cuts" user))
          (footprint "test" (layer "F.Cu") (at 0 0)
            (pad "1" thru_hole circle (at 10 5) (size 2 2) (drill 1) (layers "*.Cu" "*.Mask"))
            (pad "2" thru_hole circle (at 10.8 5) (size 2 2) (drill 1) (layers "*.Cu" "*.Mask"))
            (pad "3" thru_hole circle (at 0 0) (size 2 2) (drill 1) (layers "*.Cu" "*.Mask")))
          (gr_rect (start 0 0) (end 20 10) (stroke (width 0.1) (type default)) (fill no) (layer "Edge.Cuts")))"#;
        let package = build(
            Path::new("notes.kicad_pcb"),
            bytes,
            &[],
            &[],
            crate::overlay::Copper::Outer,
        )
        .unwrap();
        let notes = package.manifest["notes"].as_array().unwrap();
        assert!(notes.iter().any(|n| n["severity"] == "Info"
            && n["message"]
                .as_str()
                .unwrap()
                .contains("2 overlapping drilled hole(s)")));
        assert!(notes
            .iter()
            .any(|n| n["severity"] == "Dropped"
                && n["message"].as_str().unwrap().contains("board edge")));
    }

    #[test]
    fn overlapping_plated_holes_have_one_tube() {
        use board_model::{Board, CopperItem, Outline};
        let via = |x: f64| CopperItem::Via {
            at: Point { x, y: 5.0 },
            diameter: 1.8,
            drill: 1.0,
            tented: [None, None],
            mask_expansion: [None, None],
            layers: None,
        };
        // Two 1.000 holes 0.8 apart overlap: one opening, one tube round the union, and nothing
        // said about a hole left unplated.
        let board = Board {
            thickness: Some(1.6),
            outline: Outline {
                outer: rect(0.0, 0.0, 20.0, 10.0),
                cutouts: Vec::new(),
            },
            copper: vec![via(9.6), via(10.4)],
            ..Board::default()
        };
        let step = board_step::write(&board, "t").unwrap();
        assert!(step.holes.is_empty());
        assert_eq!(step.merged.len(), 1);
        let p = plating(&board, &step);
        let text = p.text.unwrap();
        assert_eq!(text.matches("MANIFOLD_SOLID_BREP(").count(), 1);
        assert!(
            p.notes.iter().all(|n| n["severity"] == "Info"),
            "{:?}",
            p.notes
        );
    }

    #[test]
    fn small_merged_drills_and_other_holes_keep_their_plating() {
        let via = |x, drill| CopperItem::Via {
            at: Point { x, y: 5.0 },
            diameter: 1.8,
            drill,
            tented: [None, None],
            mask_expansion: [None, None],
            layers: None,
        };
        let board = Board {
            thickness: Some(1.6),
            outline: board_model::Outline {
                outer: rect(0.0, 0.0, 20.0, 10.0),
                cutouts: Vec::new(),
            },
            copper: vec![via(5.0, 0.11), via(5.01, 0.11), via(10.0, 1.0)],
            ..Board::default()
        };
        let mut step = board_step::write(&board, "small").unwrap();
        assert_eq!((step.merged.len(), step.holes.len()), (1, 1));
        let p = plating(&board, &step);
        assert_eq!(p.text.unwrap().matches("MANIFOLD_SOLID_BREP(").count(), 2);
        assert!(p.notes.iter().all(|n| n["severity"] == "Info"));

        // A union whose outline cannot be lined leaves both usable tubes intact.
        step.merged.push(board_step::MergedHole {
            circles: vec![
                (Point { x: 15.0, y: 5.0 }, 0.5),
                (
                    Point {
                        x: 15.99999,
                        y: 5.0,
                    },
                    0.5,
                ),
            ],
            plated: true,
            span: None,
            also: 0,
        });
        let p = plating(&board, &step);
        assert_eq!(p.text.unwrap().matches("MANIFOLD_SOLID_BREP(").count(), 2);
        assert!(p.notes.iter().any(|n| n["severity"] == "Dropped"
            && n["message"]
                .as_str()
                .unwrap()
                .starts_with("2 plated drilled")));
    }

    #[test]
    fn holes_too_small_to_line_are_a_note() {
        use board_model::{Board, CopperItem};
        let via = |drill| CopperItem::Via {
            at: Point { x: 1.0, y: 1.0 },
            diameter: 1.0,
            drill,
            tented: [None, None],
            mask_expansion: [None, None],
            layers: None,
        };
        // A 0.05 mm drill has radius 0.025: nothing left inside the plating.
        let small = Board {
            copper: vec![via(0.1)],
            ..Board::default()
        };
        let p = plating(&small, &solid_with(vec![cut(1.0, 0.025, true)]));
        assert!(p.text.is_none());
        assert_eq!(p.notes.len(), 1);
        assert_eq!(p.notes[0]["severity"], "Dropped");
        // A usable one beside it still gets its tube.
        let both = Board {
            copper: vec![via(0.05), via(0.3)],
            ..Board::default()
        };
        let p = plating(
            &both,
            &solid_with(vec![cut(1.0, 0.025, true), cut(3.0, 0.15, true)]),
        );
        assert!(p.text.is_some());
        assert!(p.notes.iter().any(|n| n["severity"] == "Dropped"));
    }

    #[test]
    fn unlined_contained_drills_are_all_counted() {
        let mut small = cut(1.0, 0.025, true);
        small.also = 2;
        let p = plating(&Board::default(), &solid_with(vec![small]));
        assert!(p.text.is_none());
        assert!(p.notes[0]["message"]
            .as_str()
            .unwrap()
            .starts_with("3 plated drilled"));
    }

    #[test]
    fn a_plated_hole_that_was_not_cut_gets_no_tube() {
        use board_model::{Board, CopperItem, Outline};
        let via = |x: f64, y: f64| CopperItem::Via {
            at: Point { x, y },
            diameter: 1.0,
            drill: 0.5,
            tented: [None, None],
            mask_expansion: [None, None],
            layers: None,
        };
        // A 20 x 10 board with a 4 x 4 cutout from (8, 3). Via 1 is clear; via 2 touches the
        // board edge (its center is 0.1 from x = 0, less than its 0.25 radius); via 3 is inside
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
        let p = plating(&board, &step);
        let text = p.text.unwrap();
        assert_eq!(text.matches("MANIFOLD_SOLID_BREP(").count(), 1);
        let said = p.notes.iter().any(|n| {
            n["severity"] == "Dropped"
                && n["message"]
                    == "2 plated drilled hole(s) were not cut into the board, so have no plating"
        });
        assert!(said, "{:?}", p.notes);
        // A board whose only plated hole is not cut has no plating at all.
        let edge = Board {
            copper: vec![via(0.1, 5.0)],
            ..board
        };
        let step = board_step::write(&edge, "t").unwrap();
        let p = plating(&edge, &step);
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
        // Within 1e-3, plus the 0.005 by which thinning (overlay/thin.rs) may pull an outline in.
        let near = |a: f64, b: f64| (a - b).abs() < 1e-3 + 0.005;
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
        // The Default stroke font keeps Altium's character advance, so a text's width follows
        // Altium's bounding rectangle (README.txt: one-text x 9.800 to 17.531, one-rotated-text
        // y 14.800 to 20.532, one-bottom-text x 34.468 to 40.200). Altium's rectangle reaches a
        // pen width beyond the strokes and its letters are narrower than Hershey's, so ours may
        // differ by a few tenths of a millimeter; the old proportional spacing was 0.5 off.
        let wide = |a: f64, b: f64| (a - b).abs() < 0.4;
        // "TEST" at (10, 10): runs right from x 10.
        let (x0, y0, x1, y1) = silk("one-text", "Silkscreen top");
        assert!(near(y0, 9.9) && near(y1, 12.1), "{y0} {y1}");
        assert!(wide(x0, 10.0) && wide(x1, 17.531), "{x0} {x1}");
        // "ROT" at (25, 15), turned 90 degrees: runs up from y 15, capitals left of x 25.
        let (x0, y0, x1, y1) = silk("one-rotated-text", "Silkscreen top");
        assert!(near(x0, 22.9) && near(x1, 25.1), "{x0} {x1}");
        assert!(y0 > 15.0 && wide(y1, 20.532), "{y0} {y1}");
        // "BOT" at (40, 10), mirrored on the bottom: runs left from x 40.
        let (x0, y0, x1, y1) = silk("one-bottom-text", "Silkscreen bottom");
        assert!(near(y0, 9.9) && near(y1, 12.1), "{y0} {y1}");
        assert!(x1 < 40.0 && wide(x0, 34.468), "{x0} {x1}");
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
        assert!((area_of("one-resistor", "Finish top") - 1.6).abs() < 1e-3);
        // one-plated-pad: round 2, hole 1, on both faces.
        let ring = std::f64::consts::PI * 0.75;
        assert!((area_of("one-plated-pad", "Finish bottom") - ring).abs() < 0.02);
        // one-overlay-line: (10, 10) to (20, 10), 0.2 wide.
        let line = 2.0 + std::f64::consts::PI * 0.01;
        assert!((area_of("one-overlay-line", "Silkscreen top") - line).abs() < 0.01);
    }

    #[cfg(feature = "altium")]
    #[test]
    fn the_one_resistor_board_gives_r1_as_a_part() {
        let bytes = include_bytes!("../../altium-pcbdoc/tests/boards/one-resistor.PcbDoc");
        let package = build(
            Path::new("one-resistor.PcbDoc"),
            bytes,
            &[],
            &[],
            crate::overlay::Copper::Outer,
        )
        .unwrap();
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
    fn the_flipped_off_center_body_lands_where_altium_drew_it() {
        // one-bottom-extruded-resistor's README: the outline on the board is (23.8, 14.6),
        // (23.8, 15), (25.4, 15.4), (25.4, 14.6), projecting from the bottom. In R1's own frame
        // that body is (1.6, -0.4), (1.6, 0), (0, 0.4), (0, -0.4) (the same as on the top-side
        // one-offset-extruded-resistor), 0.45 high.
        let bytes =
            include_bytes!("../../altium-pcbdoc/tests/boards/one-bottom-extruded-resistor.PcbDoc");
        let package = build(
            Path::new("one-bottom-extruded-resistor.PcbDoc"),
            bytes,
            &[],
            &[],
            crate::overlay::Copper::Outer,
        )
        .unwrap();
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
        let package = build(
            Path::new("one-bottom-resistor.PcbDoc"),
            bytes,
            &[],
            &[],
            crate::overlay::Copper::Outer,
        )
        .unwrap();
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
        // pour: its center is copper only because of the track, and the gap shows on both sides.
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
        assert!(build(
            Path::new("notes.txt"),
            b"hello",
            &[],
            &[],
            crate::overlay::Copper::Outer
        )
        .is_err());
    }

    /// A rectangular SMD pad owned by a part, at a board position.
    fn smd_pad(owner: &str, x: f64, y: f64) -> CopperItem {
        CopperItem::Pad {
            bottom: None,
            layer: 0,
            at: Point { x, y },
            shape: board_model::PadShape::Rectangle,
            size_x: 0.9,
            size_y: 1.0,
            rotation_deg: 0.0,
            drill: None,
            plated: false,
            owner: Some(owner.into()),
            tented: [None, None],
            mask_expansion: [None, None],
        }
    }

    fn without_model(mut p: Placement) -> Placement {
        p.models.clear();
        p
    }

    fn names(placed: &Placed) -> Vec<&str> {
        placed
            .parts
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect()
    }

    #[test]
    fn a_part_with_no_model_gets_the_standard_body_of_its_package() {
        // R1: no model, 0603 pads. R2: a plain extruded body, also 0603 pads. C1: no model, on
        // the bottom, turned 90 degrees (its pads at (10, 5 -/+ 0.8) seen from the top).
        // U1: no model and pads that are no package.
        let board = Board {
            components: vec![
                without_model(part("R1", 0.0, 0.0, 0.0, Side::Top)),
                part("R2", 5.0, 0.0, 0.0, Side::Top),
                without_model(part("C1", 10.0, 5.0, 90.0, Side::Bottom)),
                without_model(part("U1", 20.0, 0.0, 0.0, Side::Top)),
            ],
            copper: vec![
                smd_pad("R1", -0.8, 0.0),
                smd_pad("R1", 0.8, 0.0),
                smd_pad("R2", 4.2, 0.0),
                smd_pad("R2", 5.8, 0.0),
                smd_pad("C1", 10.0, 4.2),
                smd_pad("C1", 10.0, 5.8),
                smd_pad("U1", 20.0, 0.0),
            ],
            ..Board::default()
        };
        let recognizer = Recognizer::new(&board);
        let all = |bodies| {
            components_with(
                &board.components,
                &[],
                1.6,
                &[],
                Some((&recognizer, bodies)),
            )
        };

        // `file`: R1 and C1 get a body; R2 keeps its extruded one; U1 is dropped as before.
        let placed = all(Bodies::File);
        assert_eq!(names(&placed), ["R1", "R1 leads", "R2", "C1", "C1 leads"]);
        let dropped: Vec<&Value> = placed
            .notes
            .iter()
            .filter(|n| n["severity"] == "Dropped")
            .collect();
        assert_eq!(dropped.len(), 1);
        assert!(dropped[0]["message"].as_str().unwrap().contains("U1"));
        let info: Vec<&str> = placed
            .notes
            .iter()
            .filter(|n| n["severity"] == "Info")
            .map(|n| n["message"].as_str().unwrap())
            .collect();
        assert_eq!(info.len(), 2, "{info:?}");
        assert!(info
            .iter()
            .any(|m| m.contains("0603 chip resistor") && m.contains("R1")));
        assert!(info
            .iter()
            .any(|m| m.contains("0603 chip capacitor") && m.contains("C1")));

        // The body is dark for R1 and tan for the capacitor; the leads are tin on both parts.
        let by_name = |n: &str| placed.parts.iter().find(|p| p["name"] == n).unwrap();
        assert_eq!(by_name("R1")["color"], "#1a1a1a");
        assert_eq!(by_name("C1")["color"], "#b08850");
        assert_eq!(by_name("R1 leads")["color"], "#c8c8cc");
        assert_eq!(by_name("R1")["group"], GROUP_TOP);
        assert_eq!(by_name("C1 leads")["group"], GROUP_BOTTOM);
        assert_eq!(by_name("R1")["transform"], by_name("R1 leads")["transform"]);
        assert_eq!(by_name("R1")["notes"][0]["severity"], "Info");
        // The bottom part's body sits where its pads are, on the board's bottom face.
        let spot = apply(&by_name("C1")["transform"], [0.0, 0.0, 0.0]);
        assert!(close(spot, [10.0, 5.0, 0.0]), "{spot:?}");

        // `standard` also replaces R2's extruded body, with the models R1's body uses.
        let placed = all(Bodies::Standard);
        assert_eq!(
            names(&placed),
            ["R1", "R1 leads", "R2", "R2 leads", "C1", "C1 leads"]
        );
        let model =
            |n: &str| placed.parts.iter().find(|p| p["name"] == n).unwrap()["model"].clone();
        assert_eq!(model("R1"), model("R2"));
        assert_eq!(model("R1 leads"), model("R2 leads"));
        // R2's plain extruded body was replaced: said apart from the parts with no model.
        let said: Vec<&str> = placed
            .notes
            .iter()
            .filter(|n| n["severity"] == "Info")
            .map(|n| n["message"].as_str().unwrap())
            .collect();
        assert!(
            said.iter().any(|m| m.contains(
                "1 component(s) had a plain extruded body, replaced by a standard 0603 chip \
                 resistor body (--bodies standard): R2"
            )),
            "{said:?}"
        );
        let r2_note = placed.parts.iter().find(|p| p["name"] == "R2").unwrap()["notes"][0]
            ["message"]
            .as_str()
            .unwrap();
        assert!(
            r2_note.contains("replacing the board's plain extruded body"),
            "{r2_note}"
        );
        let r1_note = placed.parts.iter().find(|p| p["name"] == "R1").unwrap()["notes"][0]
            ["message"]
            .as_str()
            .unwrap();
        assert!(r1_note.contains("no usable 3D model"), "{r1_note}");
        assert_ne!(model("R1"), model("C1"), "a capacitor is thicker");
        assert_ne!(model("R1 leads"), model("C1 leads"));
        assert_eq!(
            placed.models.len(),
            4,
            "a body and its leads for each of two heights"
        );
        for (_, file, bytes) in &placed.models {
            assert!(file.ends_with(".step"));
            assert!(bytes.starts_with(b"ISO-10303-21"));
        }
    }

    #[test]
    fn a_standard_body_never_replaces_a_step_model_or_takes_a_shared_designator() {
        // A part with an embedded model keeps it in either mode; two parts named R1 cannot be
        // told apart by their pads, so neither gets a body.
        let mut embedded = part("R1", 0.0, 0.0, 0.0, Side::Top);
        embedded.models = vec![ModelRef {
            name: "m".into(),
            source: ModelSource::Embedded { id: "m".into() },
            offset: [0.0; 3],
            rotation_deg: [0.0; 3],
            color: None,
        }];
        let board = Board {
            components: vec![
                embedded,
                without_model(part("R3", 5.0, 0.0, 0.0, Side::Top)),
                without_model(part("R3", 9.0, 0.0, 0.0, Side::Top)),
            ],
            copper: vec![
                smd_pad("R1", -0.8, 0.0),
                smd_pad("R1", 0.8, 0.0),
                smd_pad("R3", 4.2, 0.0),
                smd_pad("R3", 5.8, 0.0),
            ],
            ..Board::default()
        };
        let recognizer = Recognizer::new(&board);
        let step = EmbeddedModel {
            id: "m".into(),
            name: "m.step".into(),
            step: b"ISO-10303-21;\n".to_vec(),
        };
        let placed = components_with(
            &board.components,
            &[step],
            1.6,
            &[],
            Some((&recognizer, Bodies::Standard)),
        );
        assert_eq!(names(&placed), ["R1"]);
        assert_eq!(placed.models.len(), 1);
        assert!(placed.notes.iter().any(|n| n["severity"] == "Dropped"));
    }

    fn ball(cx: f64, cy: f64, z: f64, radius: f64) -> ModelRef {
        ModelRef {
            name: String::new(),
            source: ModelSource::Sphere {
                center: Point { x: cx, y: cy },
                z,
                radius,
            },
            offset: [0.0; 3],
            rotation_deg: [0.0; 3],
            color: None,
        }
    }

    fn color(r: u8, opacity: f64) -> board_model::Color {
        board_model::Color {
            r,
            g: 0,
            b: 0,
            opacity: Some(opacity),
        }
    }

    fn dropped(placed: &Placed) -> Vec<&str> {
        placed
            .notes
            .iter()
            .filter(|n| n["severity"] == "Dropped")
            .map(|n| n["message"].as_str().unwrap())
            .collect()
    }

    #[test]
    fn every_body_of_a_part_is_its_own_part_with_its_own_color() {
        // R1 at (25, 15) on the top face (z 1.6), three bodies: the extruded one (red, opaque),
        // the same outline moved up by 0.5 (blue, half see-through), and a sphere.
        let mut r1 = part("R1", 25.0, 15.0, 0.0, Side::Top);
        let mut first = r1.models[0].clone();
        first.color = Some(color(255, 1.0));
        let mut second = first.clone();
        second.offset = [0.0, 0.0, 0.5];
        second.color = Some(color(0, 0.5));
        r1.models = vec![first, second, ball(0.5, -0.25, 0.75, 0.3)];
        let placed = components(&[r1], &[], 1.6);
        assert_eq!(names(&placed), ["R1", "R1 body 2", "R1 body 3"]);
        for p in &placed.parts {
            assert_eq!(p["group"], GROUP_TOP);
        }
        assert_eq!(placed.parts[0]["color"], "#ff0000");
        assert_eq!(placed.parts[0]["opacity"], 1.0);
        assert_eq!(placed.parts[1]["color"], "#000000");
        assert_eq!(placed.parts[1]["opacity"], 0.5);
        // Equal outlines share one model file; the sphere is a second one.
        assert_eq!(placed.parts[0]["model"], placed.parts[1]["model"]);
        assert_ne!(placed.parts[0]["model"], placed.parts[2]["model"]);
        assert_eq!(placed.models.len(), 2);
        // The second body is the first, 0.5 higher: (25, 15, 1.6 + 0.5) for its origin.
        assert!(close(
            apply(&placed.parts[1]["transform"], [0.0, 0.0, 0.0]),
            [25.0, 15.0, 2.1]
        ));
        // The sphere's center is in the part's frame: x 25 + 0.5, y 15 - 0.25, z 1.6 + 0.75.
        assert!(close(
            apply(&placed.parts[2]["transform"], [0.0, 0.0, 0.0]),
            [25.5, 14.75, 2.35]
        ));
        assert!(placed.notes.is_empty(), "{:?}", placed.notes);
    }

    #[test]
    fn unknown_body_opacity_defaults_for_siblings_and_free_bodies() {
        let mut r1 = part("R1", 0.0, 0.0, 0.0, Side::Top);
        let mut unknown = color(128, 1.0);
        unknown.opacity = None;
        r1.models[0].color = Some(unknown);
        let mut sphere = ball(0.0, 0.0, 1.0, 0.5);
        sphere.color = Some(unknown);
        r1.models.push(sphere.clone());
        let placed = components_with(&[r1], &[], 1.6, &[sphere], None);
        assert_eq!(names(&placed), ["R1", "R1 body 2", "Free body 1"]);
        for body in &placed.parts {
            assert_eq!(body["color"], "#800000");
            assert_eq!(body["opacity"], 1.0);
        }
        assert!(placed.notes.is_empty(), "{:?}", placed.notes);
    }

    #[test]
    fn a_spheres_center_turns_about_the_part_origin_before_its_offset() {
        let mut r1 = part("R1", 10.0, 20.0, 90.0, Side::Top);
        let mut sphere = ball(1.0, 2.0, 3.0, 0.5);
        sphere.rotation_deg = [90.0, 0.0, 90.0];
        sphere.offset = [4.0, 5.0, 6.0];
        r1.models = vec![sphere];
        let placed = components(&[r1], &[], 1.6);
        assert_eq!(names(&placed), ["R1"]);
        // We turn (1, 2, 3) about X by 90 degrees to (1, -3, 2), then about Z
        // to (3, 1, 2). The offset makes (7, 6, 8). The part's 90-degree Z turn
        // makes (-6, 7, 8); its board position (10, 20, 1.6) makes (4, 27, 9.6).
        assert!(close(
            apply(&placed.parts[0]["transform"], [0.0, 0.0, 0.0]),
            [4.0, 27.0, 9.6]
        ));
        assert!(placed.notes.is_empty());
    }

    #[test]
    fn an_overflowing_body_placement_is_failed_without_a_null_transform() {
        let mut r1 = part("R1", f64::MAX, 0.0, 0.0, Side::Top);
        let mut huge = ball(0.0, 0.0, 0.0, 1.0);
        huge.offset = [f64::MAX, 0.0, 0.0];
        r1.models = vec![huge, ball(0.0, 0.0, 0.0, 1.0)];
        let placed = components(&[r1], &[], 1.6);
        // We add two finite MAX offsets for the main body, which overflows.
        // Its sibling still has a finite placement and remains in the package.
        assert_eq!(names(&placed), ["R1 body 2"]);
        assert!(placed.parts[0]["transform"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|row| row.as_array().unwrap())
            .all(|number| number.as_f64().is_some_and(f64::is_finite)));
        let notes = dropped(&placed);
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("R1: the body's placement is not finite"));
    }

    #[test]
    fn a_spheres_file_holds_a_ball_of_its_radius_and_equal_ones_share_it() {
        let mut r1 = part("R1", 0.0, 0.0, 0.0, Side::Top);
        r1.models = vec![
            ball(0.0, 0.0, 0.3, 0.3),
            ball(1.0, 0.0, 0.3, 0.3),
            ball(2.0, 0.0, 0.3, 0.5),
        ];
        let placed = components(&[r1], &[], 1.6);
        assert_eq!(placed.parts.len(), 3);
        assert_eq!(placed.models.len(), 2, "radius 0.3 once, radius 0.5 once");
        let text = |i: usize| String::from_utf8(placed.models[i].2.clone()).unwrap();
        assert!(text(0).contains("SPHERICAL_SURFACE('',#") && text(0).contains(",0.3);"));
        assert!(text(1).contains(",0.5);"));
        assert_eq!(placed.parts[0]["model"], placed.parts[1]["model"]);
        assert_ne!(placed.parts[0]["model"], placed.parts[2]["model"]);
    }

    #[test]
    fn a_bottom_side_part_carries_its_extra_bodies_with_it() {
        // On the bottom face (z 0) the part's frame is flipped: a body 0.5 up in the part's
        // frame is 0.5 below the board.
        let mut r1 = part("R1", 5.0, 5.0, 0.0, Side::Bottom);
        let mut second = r1.models[0].clone();
        second.offset = [0.0, 0.0, 0.5];
        r1.models.push(second);
        let placed = components(&[r1], &[], 1.6);
        assert_eq!(names(&placed), ["R1", "R1 body 2"]);
        assert_eq!(placed.parts[1]["group"], GROUP_BOTTOM);
        assert!(close(
            apply(&placed.parts[1]["transform"], [0.0, 0.0, 0.0]),
            [5.0, 5.0, -0.5]
        ));
    }

    #[test]
    fn body_names_stay_unique() {
        // A part really named "R1 body 2" is taken first; R1's second body gets "(2)".
        let mut r1 = part("R1", 0.0, 0.0, 0.0, Side::Top);
        r1.models.push(r1.models[0].clone());
        let mut clash = part("R1 body 2", 5.0, 0.0, 0.0, Side::Top);
        clash.models.clear();
        let placed = components(&[clash, r1], &[], 1.6);
        // The clash part has no model, but its name is reserved before R1's bodies are named.
        assert_eq!(names(&placed), ["R1", "R1 body 2 (2)"]);
    }

    #[test]
    fn free_bodies_are_parts_of_their_own_in_their_own_group() {
        // Board coordinates, z up from the top face (1.6): a sphere at (5, 6) 1.0 above the top
        // face, and a raised box.
        let mut raised = part("X", 0.0, 0.0, 0.0, Side::Top).models.remove(0);
        raised.offset = [10.0, 20.0, 0.25];
        raised.rotation_deg = [0.0, 0.0, 90.0];
        let free = [ball(5.0, 6.0, 1.0, 0.4), raised];
        let placed = components_with(&[], &[], 1.6, &free, None);
        assert_eq!(names(&placed), ["Free body 1", "Free body 2"]);
        for p in &placed.parts {
            assert_eq!(p["group"], GROUP_FREE);
        }
        assert!(close(
            apply(&placed.parts[0]["transform"], [0.0, 0.0, 0.0]),
            [5.0, 6.0, 2.6]
        ));
        // The box's corner (0.8, 0.4, 0.45), turned a quarter about Z, is at (-0.4, 0.8), then
        // moved by (10, 20, 0.25) and up by the top face.
        assert!(close(
            apply(&placed.parts[1]["transform"], [0.8, 0.4, 0.45]),
            [9.6, 20.8, 2.3]
        ));
    }

    #[test]
    fn a_designator_called_free_body_does_not_clash() {
        let mut p = part("Free body 1", 0.0, 0.0, 0.0, Side::Top);
        p.models.truncate(1);
        let placed = components_with(&[p], &[], 1.6, &[ball(0.0, 0.0, 0.0, 1.0)], None);
        assert_eq!(names(&placed), ["Free body 1", "Free body 1 (2)"]);
    }

    #[test]
    fn spheres_that_are_not_balls_are_dropped_with_a_note() {
        let mut r1 = part("R1", 0.0, 0.0, 0.0, Side::Top);
        r1.models = vec![
            ball(0.0, 0.0, 0.0, f64::NAN),
            ball(0.0, 0.0, 0.0, -1.0),
            ball(0.0, 0.0, 0.0, 0.0),
            ball(f64::NAN, 0.0, 0.0, 1.0),
            ball(0.0, 0.0, f64::INFINITY, 1.0),
            ball(0.0, 0.0, 0.0, 1e12),
        ];
        let placed = components_with(&[r1], &[], 1.6, &[ball(0.0, 0.0, 0.0, -2.0)], None);
        assert!(placed.parts.is_empty());
        assert!(placed.models.is_empty());
        let notes = dropped(&placed);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].starts_with("7 component(s) have a 3D model that could not be used"));
        assert!(notes[0].contains("R1 body 6") && notes[0].contains("Free body 1"));
    }

    #[test]
    fn a_huge_number_of_bodies_is_bounded_and_noted() {
        let mut r1 = part("R1", 0.0, 0.0, 0.0, Side::Top);
        r1.models = vec![ball(0.0, 0.0, 0.0, 1.0); MAX_BODIES_PER_PART + 44];
        let free = vec![ball(0.0, 0.0, 0.0, 1.0); MAX_FREE_BODIES + 3];
        let placed = components_with(&[r1], &[], 1.6, &free, None);
        let free_parts = placed
            .parts
            .iter()
            .filter(|p| p["group"] == GROUP_FREE)
            .count();
        assert_eq!(free_parts, MAX_FREE_BODIES);
        assert_eq!(placed.parts.len(), MAX_BODIES_PER_PART + MAX_FREE_BODIES);
        // One shared file for all of them.
        assert_eq!(placed.models.len(), 1);
        let notes = dropped(&placed);
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(notes.iter().any(|m| m.contains("R1 (300)")));
        assert!(notes.iter().any(|m| m.starts_with("3 free 3D bod")));
    }

    #[test]
    fn the_total_body_limit_counts_later_parts_and_free_bodies() {
        let mut r1 = part("R1", 0.0, 0.0, 0.0, Side::Top);
        r1.models = vec![ball(0.0, 0.0, 0.0, 1.0); 3];
        let mut r2 = part("R2", 0.0, 0.0, 0.0, Side::Top);
        r2.models = vec![ball(0.0, 0.0, 0.0, 1.0); 2];
        let free = vec![ball(0.0, 0.0, 0.0, 1.0); 2];
        let placed = components_with_limit(&[r1, r2], &[], 1.6, &free, None, 2);
        assert_eq!(names(&placed), ["R1", "R1 body 2"]);
        let notes = dropped(&placed);
        // Of seven bodies, two fit: R1 loses one, R2 two, and the free bodies two.
        assert_eq!(
            notes
                .iter()
                .filter(|m| m.contains("bodies left out:"))
                .count(),
            1
        );
        assert!(notes.contains(&"5 bodies left out: the package holds at most 2 bodies"));
        // We still account for R2 when even its main body cannot fit.
        assert!(notes
            .iter()
            .any(|m| m.contains("R2 (main body left out by the package body limit)")));
    }

    #[test]
    fn standard_bodies_only_come_to_a_part_with_no_usable_model() {
        // Two parts with 0603 pads: R1 has a sphere (a usable model), R2 has none. Even with
        // `--bodies standard`, only R2 gets the standard body and leads.
        let mut r1 = part("R1", 0.0, 0.0, 0.0, Side::Top);
        r1.models = vec![ball(0.0, 0.0, 0.5, 0.5)];
        let board = Board {
            components: vec![r1, without_model(part("R2", 5.0, 0.0, 0.0, Side::Top))],
            copper: vec![
                smd_pad("R1", -0.8, 0.0),
                smd_pad("R1", 0.8, 0.0),
                smd_pad("R2", 4.2, 0.0),
                smd_pad("R2", 5.8, 0.0),
            ],
            ..Board::default()
        };
        let recognizer = Recognizer::new(&board);
        let placed = components_with(
            &board.components,
            &[],
            1.6,
            &[],
            Some((&recognizer, Bodies::Standard)),
        );
        assert_eq!(names(&placed), ["R1", "R2", "R2 leads"]);
    }

    #[test]
    fn body_opacity_fallback_is_noted_and_zero_alone_is_transparent() {
        for opacity in [
            None,
            Some(0.0),
            Some(0.5),
            Some(1.0),
            Some(-0.1),
            Some(1.1),
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(f64::NEG_INFINITY),
        ] {
            let mut model = ball(0.0, 0.0, 0.0, 1.0);
            model.color = Some(board_model::Color {
                r: 10,
                g: 20,
                b: 30,
                opacity,
            });
            let mut notes = Vec::new();
            let part = body_part(
                &model,
                "U1",
                "model-1",
                &translation(0.0, 0.0, 0.0),
                GROUP_FREE,
                &mut notes,
            )
            .unwrap_or_else(|_| panic!("a finite body placement must succeed"));
            let invalid = opacity.is_some_and(|o| !o.is_finite() || !(0.0..=1.0).contains(&o));
            if invalid {
                assert_eq!(part["opacity"], 1.0);
                assert_eq!(
                    notes,
                    vec![json!({
                        "severity": "Info",
                        "message": format!("U1's 3D body opacity {} is not between 0 and 1, so it is drawn opaque", opacity.unwrap()),
                    })]
                );
            } else if opacity == Some(0.0) {
                assert_eq!(part["opacity"], 0.0);
                assert_eq!(
                    notes,
                    vec![json!({
                        "severity": "Info",
                        "message": "U1's 3D body is fully transparent, as the board file says",
                    })]
                );
            } else {
                assert_eq!(part["opacity"], opacity.unwrap_or(1.0));
                assert!(notes.is_empty(), "{notes:?}");
            }
        }
    }
}

#[cfg(all(test, feature = "altium"))]
mod outcome_tests {
    use super::*;
    use board_model::InputFormat;

    #[test]
    fn resolved_outcome_build_matches_file_build() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../altium-pcbdoc/tests/boards/outline-with-hole.PcbDoc");
        let bytes = std::fs::read(&path).unwrap();
        let expected = build_accepting(
            &path,
            &bytes,
            &[],
            &[],
            crate::overlay::Copper::Outer,
            Bodies::default(),
            Accept { laminate: true },
        )
        .unwrap();
        let outcome = altium_pcbdoc::Altium.read(&bytes).unwrap();
        let name = expected.manifest["name"].as_str().unwrap();
        let actual = build_outcome(
            outcome,
            &altium_pcbdoc::Altium,
            name,
            crate::overlay::Copper::Outer,
            Bodies::default(),
            Accept { laminate: true },
        )
        .unwrap();
        assert_eq!(actual, expected);
    }
}
