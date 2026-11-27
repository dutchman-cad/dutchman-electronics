//! Linked 3D models: a board can link a part to a model file it does not hold
//! ([`ModelSource::External`]). This looks for each linked file in the folders it is given and,
//! when it finds the right one, turns the part's model into an embedded one, as if the board held
//! the file.
//!
//! - Altium keeps the file's bare name and a checksum of it (see
//!   `altium_pcbdoc::model_checksum`): a file is used only when its checksum is the board's.
//! - KiCad keeps a path, often under a path variable (`${KICAD9_3DMODEL_DIR}/x.3dshapes/y.step`),
//!   and no checksum. The file is used by its name alone, as KiCad itself finds it, with an Info
//!   note saying so. The path variable is resolved against the folders given, never a system
//!   folder; a `.wrl` (a mesh) is replaced by a STEP file of the same name next to it; and a model
//!   far larger than its part's pads is taken for the wrong file and not used.
//!
//! The reader crates never touch the file system; this is the command's step after reading.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};

use board_model::{
    Board, CopperItem, EmbeddedModel, InputFormat, ModelSource, Note, ReadOutcome, Segment,
    Severity, MAX_INPUT_BYTES,
};

/// How many names a folder may list before the rest is not looked at.
const MAX_ENTRIES: usize = 100_000;
/// How many linked files get a note of their own; the rest are counted.
const MAX_NAMED: usize = 20;
/// The prefix of the ids given to linked files, so they cannot meet an embedded model's id.
const ID_PREFIX: &str = "linked:";
/// A model found by name alone is not used when it is more than this many times the size of its
/// part's pads, in either direction.
const MAX_SIZE_RATIO: f64 = 10.0;
/// The most points read from a STEP file to measure it.
const MAX_POINTS: usize = 2_000_000;

/// The folders a board's linked models are looked for in: the board's own folder and its
/// `Models` folder (Altium's default model search path, `.\Models`), then `extra` in order.
pub fn folders(board: &Path, extra: &[PathBuf]) -> Vec<PathBuf> {
    let dir = board
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut list = vec![dir.to_path_buf(), dir.join("Models")];
    list.extend(extra.iter().cloned());
    list
}

/// What was found for one linked file.
enum Lookup {
    Used,
    /// Used by its name alone (the format keeps no checksum): the name of the file found.
    UsedByName(String),
    /// Files of that name were found, but none has the board's checksum.
    Differs,
    NotFound,
    /// The board gives no checksum while its format keeps one, so a file could not be told from
    /// another of the same name.
    Unchecked,
    /// Not a plain file name (it names a folder or climbs out of one).
    NotPlain,
    /// Reading it would pass the limit for all linked files together.
    OverBudget,
    /// Only a mesh (`.wrl`) of that name was found, and no STEP file beside it.
    MeshOnly,
}

/// A model found for a link, as given to the parts that use it.
struct Resolved {
    id: String,
    by_name: bool,
    /// The model's size in x and y, when it could be measured.
    size: Option<[f64; 2]>,
}

/// Replaces each linked model whose file is found in `folders` by an embedded one, and returns
/// notes on what was used and what was not.
pub fn resolve(
    outcome: &mut ReadOutcome,
    format: &dyn InputFormat,
    folders: &[PathBuf],
) -> Vec<Note> {
    // Each linked (file, checksum) once, with how many parts use it.
    let mut linked: BTreeMap<(String, Option<u32>), usize> = BTreeMap::new();
    let all = outcome
        .board
        .components
        .iter()
        .flat_map(|p| p.models.iter())
        .chain(outcome.board.free_models.iter());
    for m in all {
        if let ModelSource::External { file, checksum } = &m.source {
            *linked.entry((file.clone(), *checksum)).or_default() += 1;
        }
    }
    // A format that computes checksums computes one for an empty file too.
    let computes = format.model_checksum(&[]).is_some();
    let mut budget = MAX_INPUT_BYTES;
    let mut results = Vec::new();
    let mut found: BTreeMap<(String, Option<u32>), Resolved> = BTreeMap::new();
    for ((file, checksum), parts) in &linked {
        let key = (file.clone(), *checksum);
        let mut add = |name: String, step: Vec<u8>, by_name: bool| {
            let id = format!("{ID_PREFIX}{}:{name}", found.len().saturating_add(1));
            let size = step_size(&step);
            outcome.models.push(EmbeddedModel {
                id: id.clone(),
                name,
                step,
            });
            found.insert(key.clone(), Resolved { id, by_name, size });
        };
        let lookup = match (*checksum, computes) {
            (Some(_), true) if !is_plain(file) => Lookup::NotPlain,
            (Some(want), true) => match find(file, want, format, folders, &mut budget) {
                Found::File(step) => {
                    add(file.clone(), step, false);
                    Lookup::Used
                }
                Found::Differs => Lookup::Differs,
                Found::Nothing => Lookup::NotFound,
                Found::OverBudget => Lookup::OverBudget,
            },
            (None, false) => match by_name(file, folders, &mut budget) {
                Named::File(name, step) => {
                    add(name.clone(), step, true);
                    Lookup::UsedByName(name)
                }
                Named::Mesh => Lookup::MeshOnly,
                Named::Nothing => Lookup::NotFound,
                Named::NotPlain => Lookup::NotPlain,
                Named::OverBudget => Lookup::OverBudget,
            },
            _ => Lookup::Unchecked,
        };
        results.push((file.clone(), *parts, lookup));
    }

    let pads = pad_sizes(&outcome.board);
    let mut too_large: Vec<(String, String, [f64; 2], [f64; 2])> = Vec::new();
    for p in &mut outcome.board.components {
        for model in &mut p.models {
            let ModelSource::External { file, checksum } = &model.source else {
                continue;
            };
            let Some(r) = found.get(&(file.clone(), *checksum)) else {
                continue;
            };
            if r.by_name {
                if let (Some(size), Some(pad)) = (r.size, pads.get(&p.designator)) {
                    if far_larger(size, *pad) {
                        too_large.push((p.designator.clone(), file.clone(), size, *pad));
                        continue;
                    }
                }
            }
            model.source = ModelSource::Embedded { id: r.id.clone() };
        }
    }
    for model in &mut outcome.board.free_models {
        let ModelSource::External { file, checksum } = &model.source else {
            continue;
        };
        if let Some(r) = found.get(&(file.clone(), *checksum)) {
            model.source = ModelSource::Embedded { id: r.id.clone() };
        }
    }
    // A model no part uses after all is not handed on.
    let used: BTreeSet<&str> = outcome
        .board
        .components
        .iter()
        .flat_map(|p| p.models.iter())
        .chain(outcome.board.free_models.iter())
        .filter_map(|m| match &m.source {
            ModelSource::Embedded { id } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    outcome
        .models
        .retain(|m| !m.id.starts_with(ID_PREFIX) || used.contains(m.id.as_str()));
    for (_, file, _, _) in &too_large {
        if let Some((_, parts, _)) = results.iter_mut().find(|(f, _, _)| f == file) {
            *parts = parts.saturating_sub(1);
        }
    }
    let mut notes = notes(&results);
    for (designator, file, size, pad) in too_large.iter().take(MAX_NAMED) {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{designator}'s model file {file} measures {:.1} x {:.1} mm, more than \
                 {MAX_SIZE_RATIO} times its pads ({:.1} x {:.1} mm), so it is taken for the wrong \
                 file and not used",
                size[0], size[1], pad[0], pad[1]
            ),
        });
    }
    if too_large.len() > MAX_NAMED {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "and {} more part(s) whose model file is far larger than their pads, not used",
                too_large.len() - MAX_NAMED
            ),
        });
    }
    notes
}

enum Found {
    File(Vec<u8>),
    Differs,
    Nothing,
    OverBudget,
}

/// The first file named `file` (any case) in `folders` whose checksum is `want`.
fn find(
    file: &str,
    want: u32,
    format: &dyn InputFormat,
    folders: &[PathBuf],
    budget: &mut usize,
) -> Found {
    let mut differs = false;
    for folder in folders {
        for path in named(folder, file) {
            let bytes = match read_within(&path, budget) {
                Read::Bytes(bytes) => bytes,
                Read::TooBig => return Found::OverBudget,
                Read::Unreadable => continue,
            };
            if format.model_checksum(&bytes) == Some(want) {
                return Found::File(bytes);
            }
            differs = true;
        }
    }
    if differs {
        Found::Differs
    } else {
        Found::Nothing
    }
}

enum Named {
    /// The name of the file found, and its bytes.
    File(String, Vec<u8>),
    Mesh,
    Nothing,
    NotPlain,
    OverBudget,
}

/// The first file a KiCad-style link names, by name alone. A `.wrl` link takes a `.step` or
/// `.stp` file of the same name first, since a mesh cannot be used.
fn by_name(file: &str, folders: &[PathBuf], budget: &mut usize) -> Named {
    let Some((dirs, name)) = link_parts(file) else {
        return Named::NotPlain;
    };
    let mesh = stem_of_mesh(name);
    let wanted: Vec<String> = match mesh {
        Some(stem) => vec![format!("{stem}.step"), format!("{stem}.stp")],
        None => vec![name.to_string()],
    };
    for want in &wanted {
        for folder in folders {
            for path in candidates(folder, &dirs, want) {
                match read_within(&path, budget) {
                    Read::Bytes(bytes) => {
                        let found = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| want.clone());
                        return Named::File(found, bytes);
                    }
                    Read::TooBig => return Named::OverBudget,
                    Read::Unreadable => {}
                }
            }
        }
    }
    if mesh.is_some()
        && folders
            .iter()
            .any(|f| !candidates(f, &dirs, name).is_empty())
    {
        Named::Mesh
    } else {
        Named::Nothing
    }
}

/// The folders under a search folder and the file name a KiCad-style link points to. A link under
/// a known path variable (`${KIPRJMOD}`, `${KICAD9_3DMODEL_DIR}` and the like, or KiCad 5's
/// `${KISYS3DMOD}`), or a relative one, keeps its folders, looked for under each search folder; an
/// absolute link or one under another variable is looked for by its file name only, so nothing
/// is read from where it points. `None` when a part of it climbs out of a folder or is not a
/// plain name.
fn link_parts(file: &str) -> Option<(Vec<&str>, &str)> {
    let mut rest = file.trim();
    let mut keep_dirs = true;
    if let Some(after) = rest.strip_prefix("${").or_else(|| rest.strip_prefix("$(")) {
        let end = after.find(['}', ')'])?;
        keep_dirs = is_known_variable(&after[..end]);
        rest = &after[end + 1..];
    } else if rest.starts_with(['/', '\\']) || rest.get(1..2) == Some(":") {
        keep_dirs = false;
    }
    let parts: Vec<&str> = rest
        .split(['/', '\\'])
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    let (name, dirs) = parts.split_last()?;
    if !is_plain(name) {
        return None;
    }
    if !keep_dirs {
        return Some((Vec::new(), name));
    }
    dirs.iter()
        .all(|d| is_plain(d))
        .then(|| (dirs.to_vec(), *name))
}

fn is_known_variable(name: &str) -> bool {
    name == "KIPRJMOD"
        || name == "KISYS3DMOD"
        || name
            .strip_prefix("KICAD")
            .and_then(|v| v.strip_suffix("_3DMODEL_DIR"))
            .is_some_and(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
}

/// The stem of a mesh file name (`.wrl` or `.wrz`, any case), or `None`.
fn stem_of_mesh(name: &str) -> Option<&str> {
    let (stem, ext) = name.rsplit_once('.')?;
    (ext.eq_ignore_ascii_case("wrl") || ext.eq_ignore_ascii_case("wrz")).then_some(stem)
}

/// The files `name` may be under `folder`: below the link's own folders first, then in `folder`.
fn candidates(folder: &Path, dirs: &[&str], name: &str) -> Vec<PathBuf> {
    let mut list = Vec::new();
    if !dirs.is_empty() {
        let mut sub = folder.to_path_buf();
        sub.extend(dirs);
        list = named(&sub, name);
    }
    for path in named(folder, name) {
        if !list.iter().any(|f| same_file(f, &path)) {
            list.push(path);
        }
    }
    list
}

/// The files in `folder` named `file`: the exact name first, then any that differ only in case.
fn named(folder: &Path, file: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let exact = folder.join(file);
    if exact.is_file() {
        found.push(exact.clone());
    }
    let Ok(entries) = std::fs::read_dir(folder) else {
        return found;
    };
    for entry in entries.take(MAX_ENTRIES).flatten() {
        let path = entry.path();
        let same = entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.eq_ignore_ascii_case(file));
        // On a file system that ignores case, the exact path is this entry already.
        if same && path.is_file() && !found.iter().any(|f| same_file(f, &path)) {
            found.push(path);
        }
    }
    found
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// What reading one file gave.
enum Read {
    /// The file, whose size was taken from the budget.
    Bytes(Vec<u8>),
    /// The file is larger than what is left of the budget, which is unchanged.
    TooBig,
    /// The file could not be opened or read.
    Unreadable,
}

/// Reads a file if it fits in what is left of `budget`, and takes its size from it.
fn read_within(path: &Path, budget: &mut usize) -> Read {
    let Ok(file) = std::fs::File::open(path) else {
        return Read::Unreadable;
    };
    let limit = u64::try_from(*budget).unwrap_or(u64::MAX);
    let mut bytes = Vec::new();
    if file
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Read::Unreadable;
    }
    if bytes.len() > *budget {
        return Read::TooBig;
    }
    *budget -= bytes.len();
    Read::Bytes(bytes)
}

/// A bare file name: no folder in it, and not `.` or `..`.
fn is_plain(file: &str) -> bool {
    !file.trim().is_empty() && file != "." && file != ".." && !file.contains(['/', '\\', ':', '\0'])
}

/// The size in x and y of the points of a STEP file, in mm (a file in inches is scaled), or
/// `None` when it has no points.
fn step_size(step: &[u8]) -> Option<[f64; 2]> {
    let text = String::from_utf8_lossy(step);
    let scale = if text.contains("'INCH'") || text.contains(".INCH.") {
        25.4
    } else {
        1.0
    };
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for chunk in text.split("CARTESIAN_POINT").skip(1).take(MAX_POINTS) {
        // CARTESIAN_POINT('name',(x,y,z)): the coordinates are in the second parenthesis.
        let Some(open) = chunk.find('(') else {
            continue;
        };
        let Some(inner) = chunk
            .get(open + 1..)
            .and_then(|c| c.split_once('('))
            .map(|(_, r)| r)
        else {
            continue;
        };
        let Some(coords) = inner.split_once(')').map(|(c, _)| c) else {
            continue;
        };
        let mut values = coords.split(',').map(|v| v.trim().parse::<f64>());
        if let (Some(Ok(x)), Some(Ok(y))) = (values.next(), values.next()) {
            for ((l, h), v) in lo.iter_mut().zip(hi.iter_mut()).zip([x, y]) {
                *l = l.min(v * scale);
                *h = h.max(v * scale);
            }
        }
    }
    (lo[0] <= hi[0] && lo[1] <= hi[1]).then(|| [hi[0] - lo[0], hi[1] - lo[1]])
}

/// The size in x and y of each component's pads, by designator.
fn pad_sizes(board: &Board) -> BTreeMap<String, [f64; 2]> {
    let mut boxes: BTreeMap<String, ([f64; 2], [f64; 2])> = BTreeMap::new();
    let mut grow = |owner: &str, x: f64, y: f64, r: f64| {
        let b = boxes
            .entry(owner.to_string())
            .or_insert(([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]));
        b.0 = [b.0[0].min(x - r), b.0[1].min(y - r)];
        b.1 = [b.1[0].max(x + r), b.1[1].max(y + r)];
    };
    for item in &board.copper {
        match item {
            CopperItem::Pad {
                at,
                size_x,
                size_y,
                owner: Some(owner),
                ..
            } => grow(owner, at.x, at.y, size_x.max(*size_y) / 2.0),
            CopperItem::PolygonPad {
                shapes,
                owner: Some(owner),
                ..
            } => {
                for s in shapes.iter().flat_map(|c| &c.segments) {
                    match s {
                        Segment::Line { from, to } => {
                            grow(owner, from.x, from.y, 0.0);
                            grow(owner, to.x, to.y, 0.0);
                        }
                        Segment::Arc { center, radius, .. } => {
                            grow(owner, center.x, center.y, *radius);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    boxes
        .into_iter()
        .map(|(k, (lo, hi))| (k, [hi[0] - lo[0], hi[1] - lo[1]]))
        .collect()
}

/// A model far larger than its pads: its larger side against theirs, and its smaller side against
/// theirs, so a model turned a quarter turn is measured the same.
fn far_larger(model: [f64; 2], pads: [f64; 2]) -> bool {
    let sorted = |v: [f64; 2]| if v[0] >= v[1] { v } else { [v[1], v[0]] };
    let (m, p) = (sorted(model), sorted(pads));
    m[0] > MAX_SIZE_RATIO * p[0] || m[1] > MAX_SIZE_RATIO * p[1]
}

fn notes(results: &[(String, usize, Lookup)]) -> Vec<Note> {
    let mut notes = Vec::new();
    let mut more = 0usize;
    for (file, parts, lookup) in results {
        let (severity, message) = match lookup {
            Lookup::Used => (
                Severity::Info,
                format!(
                    "{parts} part(s) link to the model file {file}, which was found with the \
                     board's checksum and is used"
                ),
            ),
            Lookup::UsedByName(found) => (
                Severity::Info,
                format!(
                    "{parts} part(s) link to the model file {file}; {found} was found by its name \
                     only (the board keeps no checksum to check it by) and is used"
                ),
            ),
            Lookup::Differs => (
                Severity::Warning,
                format!(
                    "{parts} part(s) link to the model file {file}; a file of that name was found, \
                     but its checksum is not the board's, so it is not the file the board was \
                     linked to and is not used"
                ),
            ),
            Lookup::NotFound => (
                Severity::Warning,
                format!(
                    "{parts} part(s) link to the model file {file}, which is not in the board's \
                     folder, its Models folder or a folder given with --models"
                ),
            ),
            Lookup::Unchecked => (
                Severity::Warning,
                format!(
                    "{parts} part(s) link to the model file {file}, but the board gives no \
                     checksum to tell it from another file of that name, so none is used"
                ),
            ),
            Lookup::NotPlain => (
                Severity::Warning,
                format!(
                    "{parts} part(s) link to the model \"{file}\", which is not a plain file \
                     name, so it is not looked for"
                ),
            ),
            Lookup::OverBudget => (
                Severity::Warning,
                format!(
                    "{parts} part(s) link to the model file {file}, which was not read: the \
                     linked models together would pass {MAX_INPUT_BYTES} bytes"
                ),
            ),
            Lookup::MeshOnly => (
                Severity::Warning,
                format!(
                    "{parts} part(s) link to the model file {file}, a mesh, and no STEP file of \
                     that name (.step or .stp) was found beside it; a mesh cannot be used"
                ),
            ),
        };
        if *parts == 0 {
            continue;
        }
        if notes.len() < MAX_NAMED {
            notes.push(Note { severity, message });
        } else {
            more = more.saturating_add(1);
        }
    }
    if more > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!("and {more} more linked model file(s), not named one by one"),
        });
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use board_model::{Board, Detection, ModelRef, Placement, Point, ReadError, Side};

    /// A format whose checksum is the file's length.
    struct Lengths;

    impl InputFormat for Lengths {
        fn id(&self) -> &'static str {
            "lengths"
        }
        fn name(&self) -> &'static str {
            "test"
        }
        fn extensions(&self) -> &'static [&'static str] {
            &[]
        }
        fn detect(&self, _: &[u8], _: Option<&str>) -> Detection {
            Detection::No
        }
        fn read(&self, _: &[u8]) -> Result<ReadOutcome, ReadError> {
            Err(ReadError::NotYet("test".into()))
        }
        fn model_checksum(&self, file: &[u8]) -> Option<u32> {
            u32::try_from(file.len()).ok()
        }
    }

    /// A format that keeps no checksum.
    struct NoChecksum;

    impl InputFormat for NoChecksum {
        fn id(&self) -> &'static str {
            "none"
        }
        fn name(&self) -> &'static str {
            "test"
        }
        fn extensions(&self) -> &'static [&'static str] {
            &[]
        }
        fn detect(&self, _: &[u8], _: Option<&str>) -> Detection {
            Detection::No
        }
        fn read(&self, _: &[u8]) -> Result<ReadOutcome, ReadError> {
            Err(ReadError::NotYet("test".into()))
        }
    }

    fn part(designator: &str, file: &str, checksum: Option<u32>) -> Placement {
        Placement {
            designator: designator.into(),
            footprint: String::new(),
            at: Point::default(),
            rotation_deg: 0.0,
            side: Side::Top,
            models: vec![ModelRef {
                name: file.into(),
                source: ModelSource::External {
                    file: file.into(),
                    checksum,
                },
                offset: [0.0; 3],
                rotation_deg: [0.0; 3],
                color: None,
            }],
            comment: String::new(),
        }
    }

    fn outcome(parts: Vec<Placement>) -> ReadOutcome {
        ReadOutcome {
            board: Board {
                components: parts,
                ..Board::default()
            },
            notes: Vec::new(),
            models: Vec::new(),
            fonts: Vec::new(),
        }
    }

    fn temp(name: &str) -> Result<PathBuf, std::io::Error> {
        let dir = std::env::temp_dir().join(format!("de-linked-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    fn source(out: &ReadOutcome, i: usize) -> Option<&ModelSource> {
        out.board
            .components
            .get(i)?
            .models
            .first()
            .map(|m| &m.source)
    }

    #[test]
    fn a_file_with_the_boards_checksum_becomes_an_embedded_model(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = temp("used")?;
        std::fs::create_dir_all(dir.join("Models"))?;
        std::fs::write(dir.join("Models").join("box.step"), b"12345")?;
        let mut out = outcome(vec![
            part("R1", "box.step", Some(5)),
            part("R2", "box.step", Some(5)),
        ]);
        let notes = resolve(&mut out, &Lengths, &folders(&dir.join("b.PcbDoc"), &[]));
        std::fs::remove_dir_all(&dir).ok();
        let [m] = out.models.as_slice() else {
            return Err(format!("{:?}", out.models).into());
        };
        assert_eq!(
            (m.name.as_str(), m.step.as_slice()),
            ("box.step", &b"12345"[..])
        );
        let want = ModelSource::Embedded { id: m.id.clone() };
        assert_eq!(source(&out, 0), Some(&want));
        assert_eq!(source(&out, 1), Some(&want));
        assert!(m.id.starts_with(ID_PREFIX));
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].severity, Severity::Info);
        assert!(notes[0]
            .message
            .starts_with("2 part(s) link to the model file box.step"));
        Ok(())
    }

    #[test]
    fn a_file_of_the_same_name_with_another_checksum_is_not_used(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = temp("differs")?;
        let extra = temp("differs-extra")?;
        std::fs::write(dir.join("box.step"), b"1234")?;
        let mut out = outcome(vec![part("R1", "box.step", Some(5))]);
        let notes = resolve(&mut out, &Lengths, &folders(&dir.join("b.PcbDoc"), &[]));
        assert!(out.models.is_empty());
        assert!(matches!(
            source(&out, 0),
            Some(ModelSource::External { .. })
        ));
        assert!(notes[0].message.contains("its checksum is not the board's"));
        // The right file in a folder given later is found after the wrong one.
        std::fs::write(extra.join("BOX.STEP"), b"abcde")?;
        let notes = resolve(
            &mut out,
            &Lengths,
            &folders(&dir.join("b.PcbDoc"), std::slice::from_ref(&extra)),
        );
        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&extra).ok();
        assert_eq!(out.models.len(), 1, "{notes:?}");
        assert_eq!(out.models[0].step, b"abcde");
        Ok(())
    }

    #[test]
    fn missing_unchecked_and_unplain_links_are_named_and_left_alone(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = temp("missing")?;
        std::fs::write(dir.join("loose.step"), b"12345")?;
        let mut out = outcome(vec![
            part("R1", "gone.step", Some(5)),
            part("R2", "loose.step", None),
            part("R3", "../loose.step", Some(5)),
            part("R4", "sub\\loose.step", Some(5)),
        ]);
        let notes = resolve(&mut out, &Lengths, &folders(&dir.join("b.PcbDoc"), &[]));
        assert!(out.models.is_empty());
        let text: Vec<&str> = notes.iter().map(|n| n.message.as_str()).collect();
        assert_eq!(text.len(), 4, "{text:?}");
        assert!(text
            .iter()
            .any(|t| t.contains("gone.step, which is not in the board's folder")));
        assert!(text
            .iter()
            .any(|t| t.contains("loose.step, but the board gives no checksum")));
        assert_eq!(
            text.iter()
                .filter(|t| t.contains("not a plain file name"))
                .count(),
            2
        );
        // A format with no checksum uses nothing, even with a checksum on the board.
        let mut out = outcome(vec![part("R1", "loose.step", Some(5))]);
        let notes = resolve(&mut out, &NoChecksum, &folders(&dir.join("b.PcbDoc"), &[]));
        std::fs::remove_dir_all(&dir).ok();
        assert!(out.models.is_empty());
        assert!(notes[0].message.contains("gives no checksum"));
        Ok(())
    }

    #[test]
    fn notes_stop_naming_files_after_twenty() {
        let mut out = outcome(
            (0..25)
                .map(|i| part(&format!("R{i}"), &format!("m{i:02}.step"), Some(1)))
                .collect(),
        );
        let notes = resolve(&mut out, &Lengths, &[]);
        assert_eq!(notes.len(), MAX_NAMED + 1);
        assert_eq!(
            notes[MAX_NAMED].message,
            "and 5 more linked model file(s), not named one by one"
        );
    }

    #[test]
    fn a_file_over_the_budget_is_not_read() -> Result<(), Box<dyn std::error::Error>> {
        let dir = temp("budget")?;
        std::fs::write(dir.join("big.step"), b"123456")?;
        let mut budget = 5;
        assert!(matches!(
            read_within(&dir.join("big.step"), &mut budget),
            Read::TooBig
        ));
        assert_eq!(budget, 5);
        let mut budget = 6;
        assert!(
            matches!(read_within(&dir.join("big.step"), &mut budget), Read::Bytes(b) if b == b"123456")
        );
        assert_eq!(budget, 0);
        // A file that used the budget up exactly does not make a missing one look too big.
        assert!(matches!(
            read_within(&dir.join("gone.step"), &mut budget),
            Read::Unreadable
        ));
        let found = find(
            "gone.step",
            6,
            &Lengths,
            std::slice::from_ref(&dir),
            &mut budget,
        );
        assert!(matches!(found, Found::Nothing));
        let mut budget = 0;
        let found = find(
            "big.step",
            6,
            &Lengths,
            std::slice::from_ref(&dir),
            &mut budget,
        );
        std::fs::remove_dir_all(&dir).ok();
        assert!(matches!(found, Found::OverBudget));
        Ok(())
    }

    /// Whether `dir`'s file system tells names apart by case.
    fn tells_case_apart(dir: &Path) -> Result<bool, std::io::Error> {
        std::fs::write(dir.join("case-probe.txt"), b"x")?;
        let apart = !dir.join("CASE-PROBE.TXT").exists();
        std::fs::remove_file(dir.join("case-probe.txt"))?;
        Ok(apart)
    }

    #[test]
    fn names_match_in_any_case_and_one_file_is_listed_once(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = temp("case")?;
        std::fs::write(dir.join("Box.STEP"), b"12345")?;
        // Found under another case, and listed once even where the exact name opens it too.
        let found = named(&dir, "box.step");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(same_file(&found[0], &dir.join("Box.STEP")), "{found:?}");
        if tells_case_apart(&dir)? {
            // Where case tells files apart, both are found, the exact name first.
            std::fs::write(dir.join("box.step"), b"1234")?;
            let found = named(&dir, "box.step");
            assert_eq!(found.len(), 2, "{found:?}");
            assert_eq!(found[0], dir.join("box.step"));
            // The checksum picks the right one of the two.
            let mut out = outcome(vec![part("R1", "box.step", Some(5))]);
            resolve(&mut out, &Lengths, std::slice::from_ref(&dir));
            assert_eq!(out.models.len(), 1);
            assert_eq!(out.models[0].step, b"12345");
        }
        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }

    #[test]
    fn a_kicad_link_is_used_by_its_name_under_its_path_variable(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = temp("kicad")?;
        let lib = temp("kicad-lib")?;
        std::fs::create_dir_all(lib.join("Resistor_SMD.3dshapes"))?;
        std::fs::write(lib.join("Resistor_SMD.3dshapes").join("R.step"), b"res")?;
        std::fs::create_dir_all(dir.join("models"))?;
        std::fs::write(dir.join("models").join("own.step"), b"own")?;
        std::fs::write(dir.join("loose.step"), b"loose")?;
        let mut out = outcome(vec![
            part(
                "R1",
                "${KICAD9_3DMODEL_DIR}/Resistor_SMD.3dshapes/R.step",
                None,
            ),
            part("R2", "${KIPRJMOD}/models/own.step", None),
            // An absolute path and an unknown variable: looked for by the name only.
            part("R3", "/usr/share/kicad/3dmodels/loose.step", None),
            part("R4", "${MYLIB}/x/loose.step", None),
            part("R5", "${KIPRJMOD}/../secret.step", None),
        ]);
        let notes = resolve(
            &mut out,
            &NoChecksum,
            &folders(&dir.join("b.kicad_pcb"), std::slice::from_ref(&lib)),
        );
        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&lib).ok();
        let steps: Vec<&[u8]> = out.models.iter().map(|m| m.step.as_slice()).collect();
        // In the order of the links' text: ${KICAD9..., ${KIPRJMOD}/models..., ${MYLIB}..., /usr...
        assert_eq!(
            steps,
            [&b"res"[..], b"own", b"loose", b"loose"],
            "{notes:?}"
        );
        for i in 0..4 {
            assert!(
                matches!(source(&out, i), Some(ModelSource::Embedded { .. })),
                "{i}: {:?}",
                source(&out, i)
            );
        }
        assert!(matches!(
            source(&out, 4),
            Some(ModelSource::External { .. })
        ));
        let by_name = notes
            .iter()
            .filter(|n| n.severity == Severity::Info && n.message.contains("by its name only"))
            .count();
        assert_eq!(by_name, 4, "{notes:?}");
        assert!(notes.iter().any(|n| n
            .message
            .contains("secret.step\", which is not a plain file name")));
        Ok(())
    }

    #[test]
    fn a_mesh_link_takes_a_step_file_of_its_name_or_is_noted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = temp("mesh")?;
        std::fs::write(dir.join("C.wrl"), b"mesh")?;
        std::fs::write(dir.join("C.step"), b"solid")?;
        std::fs::write(dir.join("L.wrl"), b"mesh")?;
        let mut out = outcome(vec![
            part("C1", "${KISYS3DMOD}/C.wrl", None),
            part("L1", "${KISYS3DMOD}/L.wrl", None),
        ]);
        let notes = resolve(&mut out, &NoChecksum, std::slice::from_ref(&dir));
        std::fs::remove_dir_all(&dir).ok();
        let [m] = out.models.as_slice() else {
            return Err(format!("{:?}", out.models).into());
        };
        assert_eq!(
            (m.name.as_str(), m.step.as_slice()),
            ("C.step", &b"solid"[..])
        );
        assert!(matches!(
            source(&out, 1),
            Some(ModelSource::External { .. })
        ));
        assert!(
            notes.iter().any(|n| n.severity == Severity::Warning
                && n.message.contains("L.wrl, a mesh, and no STEP file")),
            "{notes:?}"
        );
        Ok(())
    }

    #[test]
    fn a_model_far_larger_than_its_pads_is_not_used() -> Result<(), Box<dyn std::error::Error>> {
        let dir = temp("large")?;
        let points = |s: f64| {
            format!(
                "ISO-10303-21;\nDATA;\n#1=CARTESIAN_POINT('',(0.,0.,0.));\n\
                 #2=CARTESIAN_POINT('',({s},{s},1.));\nENDSEC;\nEND-ISO-10303-21;\n"
            )
        };
        std::fs::write(dir.join("big.step"), points(30.0))?;
        std::fs::write(dir.join("fits.step"), points(2.0))?;
        let pad = |owner: &str, x: f64| CopperItem::Pad {
            layer: 0,
            at: Point { x, y: 0.0 },
            shape: board_model::PadShape::Rectangle,
            size_x: 1.0,
            size_y: 1.0,
            rotation_deg: 0.0,
            drill: None,
            plated: false,
            owner: Some(owner.into()),
            tented: [None, None],
            mask_expansion: [None, None],
        };
        let mut out = outcome(vec![
            part("R1", "big.step", None),
            part("R2", "fits.step", None),
        ]);
        // Each part's pads span 2 x 1 mm: big.step (30 x 30) is more than 10 times that.
        out.board.copper = vec![
            pad("R1", 0.0),
            pad("R1", 1.0),
            pad("R2", 0.0),
            pad("R2", 1.0),
        ];
        let notes = resolve(&mut out, &NoChecksum, std::slice::from_ref(&dir));
        std::fs::remove_dir_all(&dir).ok();
        assert!(matches!(
            source(&out, 0),
            Some(ModelSource::External { .. })
        ));
        assert!(matches!(
            source(&out, 1),
            Some(ModelSource::Embedded { .. })
        ));
        // Only the model a part uses is handed on.
        assert_eq!(out.models.len(), 1);
        assert_eq!(out.models[0].name, "fits.step");
        assert!(
            notes.iter().any(|n| n.severity == Severity::Warning
                && n.message
                    .starts_with("R1's model file big.step measures 30.0 x 30.0 mm")),
            "{notes:?}"
        );
        assert!(
            notes.iter().all(|n| !n.message.contains("big.step;")),
            "no part uses big.step: {notes:?}"
        );
        Ok(())
    }

    #[test]
    fn known_path_variables_keep_their_folders_and_others_do_not() {
        assert_eq!(
            link_parts("${KICAD10_3DMODEL_DIR}/a.3dshapes/b.step"),
            Some((vec!["a.3dshapes"], "b.step"))
        );
        assert_eq!(
            link_parts("$(KIPRJMOD)\\m\\b.step"),
            Some((vec!["m"], "b.step"))
        );
        assert_eq!(link_parts("${OTHER}/a/b.step"), Some((vec![], "b.step")));
        assert_eq!(link_parts("C:/lib/a/b.step"), Some((vec![], "b.step")));
        assert_eq!(link_parts("sub/b.step"), Some((vec!["sub"], "b.step")));
        assert_eq!(link_parts("${KIPRJMOD}/../b.step"), None);
        assert_eq!(link_parts("${KIPRJMOD}/"), None);
        assert!(!is_known_variable("KICAD_3DMODEL_DIR"));
        assert!(far_larger([30.0, 30.0], [2.0, 1.0]));
        assert!(!far_larger([1.0, 15.0], [2.0, 1.0]));
    }

    #[test]
    fn the_folders_are_the_boards_its_models_folder_and_the_given_ones() {
        let list = folders(Path::new("a/b.PcbDoc"), &[PathBuf::from("lib")]);
        assert_eq!(
            list,
            vec![
                PathBuf::from("a"),
                PathBuf::from("a").join("Models"),
                PathBuf::from("lib")
            ]
        );
        let list = folders(Path::new("b.PcbDoc"), &[]);
        assert_eq!(
            list,
            vec![PathBuf::from("."), PathBuf::from(".").join("Models")]
        );
    }
}
