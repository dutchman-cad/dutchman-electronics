//! Linked 3D models: a board can link a part to a model file it does not hold
//! ([`ModelSource::External`]). The board keeps only the file's bare name and, for Altium, a
//! checksum of it (see `altium_pcbdoc::model_checksum`). This looks for each linked file in the
//! folders it is given, and uses one only when its checksum is the board's: then the part's model
//! becomes an embedded one, as if the board held the file.
//!
//! The reader crates never touch the file system; this is the command's step after reading.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use board_model::{
    EmbeddedModel, InputFormat, ModelSource, Note, ReadOutcome, Severity, MAX_INPUT_BYTES,
};

/// How many names a folder may list before the rest is not looked at.
const MAX_ENTRIES: usize = 100_000;
/// How many linked files get a note of their own; the rest are counted.
const MAX_NAMED: usize = 20;
/// The prefix of the ids given to linked files, so they cannot meet an embedded model's id.
const ID_PREFIX: &str = "linked:";

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
    /// Files of that name were found, but none has the board's checksum.
    Differs,
    NotFound,
    /// The board gives no checksum, or the format computes none, so a file could not be told
    /// from another of the same name.
    Unchecked,
    /// Not a plain file name (it names a folder or climbs out of one).
    NotPlain,
    /// Reading on would pass the limit for all linked files together.
    OverBudget,
}

/// Replaces each linked model whose file is found in `folders` with the board's checksum by an
/// embedded one, and returns notes on what was used and what was not.
pub fn resolve(
    outcome: &mut ReadOutcome,
    format: &dyn InputFormat,
    folders: &[PathBuf],
) -> Vec<Note> {
    // Each linked (file, checksum) once, with how many parts use it.
    let mut linked: BTreeMap<(String, Option<u32>), usize> = BTreeMap::new();
    for p in &outcome.board.components {
        if let Some(ModelSource::External { file, checksum }) = p.model.as_ref().map(|m| &m.source)
        {
            *linked.entry((file.clone(), *checksum)).or_default() += 1;
        }
    }
    // A format that computes checksums computes one for an empty file too.
    let computes = format.model_checksum(&[]).is_some();
    let mut budget = MAX_INPUT_BYTES;
    let mut results = Vec::new();
    let mut ids: BTreeMap<(String, Option<u32>), String> = BTreeMap::new();
    for ((file, checksum), parts) in &linked {
        let lookup = if !is_plain(file) {
            Lookup::NotPlain
        } else {
            match checksum.filter(|_| computes) {
                None => Lookup::Unchecked,
                Some(want) => match find(file, want, format, folders, &mut budget) {
                    Found::File(step) => {
                        let id = format!("{ID_PREFIX}{}:{file}", ids.len().saturating_add(1));
                        outcome.models.push(EmbeddedModel {
                            id: id.clone(),
                            name: file.clone(),
                            step,
                        });
                        ids.insert((file.clone(), *checksum), id);
                        Lookup::Used
                    }
                    Found::Differs => Lookup::Differs,
                    Found::Nothing => Lookup::NotFound,
                    Found::OverBudget => Lookup::OverBudget,
                },
            }
        };
        results.push((file.clone(), *parts, lookup));
    }
    for p in &mut outcome.board.components {
        let Some(model) = p.model.as_mut() else {
            continue;
        };
        if let ModelSource::External { file, checksum } = &model.source {
            if let Some(id) = ids.get(&(file.clone(), *checksum)) {
                model.source = ModelSource::Embedded { id: id.clone() };
            }
        }
    }
    notes(&results)
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
            let Some(bytes) = read_within(&path, budget) else {
                if *budget == 0 {
                    return Found::OverBudget;
                }
                continue;
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

/// Reads a file if it fits in what is left of `budget`, and takes its size from it. A file that
/// does not fit sets `budget` to 0.
fn read_within(path: &Path, budget: &mut usize) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    let limit = u64::try_from(*budget).unwrap_or(u64::MAX);
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > *budget {
        *budget = 0;
        return None;
    }
    *budget -= bytes.len();
    Some(bytes)
}

/// A bare file name: no folder in it, and not `.` or `..`.
fn is_plain(file: &str) -> bool {
    !file.trim().is_empty() && file != "." && file != ".." && !file.contains(['/', '\\', ':', '\0'])
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
        };
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
            model: Some(ModelRef {
                name: file.into(),
                source: ModelSource::External {
                    file: file.into(),
                    checksum,
                },
                offset: [0.0; 3],
                rotation_deg: [0.0; 3],
                color: None,
            }),
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
            .model
            .as_ref()
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
        assert!(read_within(&dir.join("big.step"), &mut budget).is_none());
        assert_eq!(budget, 0);
        let mut budget = 6;
        assert_eq!(
            read_within(&dir.join("big.step"), &mut budget).as_deref(),
            Some(&b"123456"[..])
        );
        assert_eq!(budget, 0);
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
