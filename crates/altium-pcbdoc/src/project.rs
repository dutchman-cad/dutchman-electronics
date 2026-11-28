//! Altium PCB projects (`.PrjPcb`): which boards a project holds and its parameters.
//!
//! Worked out from our own CC0 test project `param-project` (tests/boards/param-project, made in
//! Altium 26.12.2; see LEGAL.md). The file is text: UTF-8 with a byte order mark and CRLF line
//! ends, in sections like an INI file. The parts read here:
//!
//! - `[Document1]`, `[Document2]`, ...: one per document in the project, in the order of the
//!   Projects panel, each with `DocumentPath=` the file's path relative to the project's folder
//!   (`param-board.PcbDoc`). Only documents whose path ends in `.PcbDoc` are boards.
//! - `[Parameter1]`, `[Parameter2]`, ...: one per project parameter, with `Name=` and `Value=`
//!   (the project's parameters; `param-project` holds them in the other order from how they
//!   were entered).
//!
//! - `[ProjectVariant1]`, `[ProjectVariant2]`, ...: one per assembly variant, worked out from
//!   our own CC0 test project `variants-project` (tests/boards/variants-project/README.txt).
//!   `Description=` is the variant's name. `VariationCount=` lines `Variation1=`, ... each name
//!   one part, as `|`-separated `key=value` fields: `Designator=R2|UniqueId=\JQYPWMZJ|Kind=1|
//!   AlternatePart=`. `Kind=1` is Not Fitted; `Kind=0` is Fitted, with a changed parameter.
//!   A changed parameter is `ParamVariation1=ParameterName=Comment|VariantValue=22k` with the
//!   part in `ParamDesignator1=R1`; for it Altium also writes the new value into the
//!   variation's `AlternatePart`. Only the Comment is read; a fitted part whose `AlternatePart`
//!   is not just its new comment (an alternate part from a library) gets a note, since the part
//!   is not swapped.
//!
//! Lines may be indented; a key is read without the spaces around it, and a value exactly as
//! written after the `=` (whether or not Altium keeps spaces at a value's end, keeping what the
//! file holds is right). A parameter value longer than [`MAX_PARAMETER_VALUE`] is refused, since
//! every text that refers to the parameter gets its own copy; a variant's new comment too.
//!
//! Everything else in the file (output settings, printer settings) is not read.
//! Like a board, the bytes are handed in; this never opens a file.

use board_model::{Board, Note, ReadError, Severity, Variant, VariantValue, MAX_INPUT_BYTES};

/// The most sections read before the file is treated as hostile.
const MAX_SECTIONS: usize = 100_000;

/// The longest parameter value read, in bytes. Altium's own are a few words; every text that
/// refers to a parameter gets its own copy of the value, so a huge one would multiply.
pub const MAX_PARAMETER_VALUE: usize = 64 * 1024;

/// What a project says about its boards and parameters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    /// The boards' paths as the project gives them, relative to the project's folder, with `\`
    /// as Altium writes it.
    pub boards: Vec<String>,
    /// The project parameters, as (name, value), in the file's order. A name that appears twice
    /// (in any case) keeps its first value.
    pub parameters: Vec<(String, String)>,
    /// The assembly variants, in the file's order.
    pub variants: Vec<Variant>,
    /// What the variants hold that is not read (alternate parts, parameters other than the
    /// Comment, kinds of variation not known).
    pub notes: Vec<Note>,
}

/// Puts a project's variants on a board read with that project, and gives the project's notes
/// on them, plus a warning for each designator a variant names that the board does not have.
pub fn add_variants(board: &mut Board, project: &Project) -> Vec<Note> {
    let mut notes = project.notes.clone();
    for variant in &project.variants {
        let named = variant
            .not_fitted
            .iter()
            .chain(variant.values.iter().map(|v| &v.designator));
        let mut missing: Vec<&str> = Vec::new();
        for designator in named {
            let on_board = board.components.iter().any(|c| &c.designator == designator);
            if !on_board && !missing.contains(&designator.as_str()) {
                missing.push(designator);
            }
        }
        if !missing.is_empty() {
            notes.push(Note {
                severity: Severity::Warning,
                message: format!(
                    "variant \"{}\" names {} that the board does not have",
                    variant.name,
                    missing.join(", ")
                ),
            });
        }
    }
    board.variants = project.variants.clone();
    notes
}

/// Reads a `.PrjPcb` file.
pub fn read(bytes: &[u8]) -> Result<Project, ReadError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(ReadError::TooLarge { bytes: bytes.len() });
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ReadError::Invalid("the project file is not UTF-8 text".into()))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if !text.trim_start().starts_with("[Design]") {
        return Err(ReadError::Invalid(
            "not an Altium project: it does not start with a [Design] section".into(),
        ));
    }
    let mut project = Project::default();
    let mut section = String::new();
    let mut sections = 0usize;
    let mut name: Option<String> = None;
    let mut value: Option<String> = None;
    let mut keys: Vec<(String, String)> = Vec::new();
    let mut finish = |section: &str,
                      name: &mut Option<String>,
                      value: &mut Option<String>,
                      keys: &mut Vec<(String, String)>| {
        if is_numbered(section, "ProjectVariant") {
            let (variant, notes) = variant(keys);
            project.variants.push(variant);
            project.notes.extend(notes);
        }
        keys.clear();
        if is_numbered(section, "Parameter") {
            if let Some(n) = name.take().filter(|n| !n.is_empty()) {
                let v = value.take().unwrap_or_default();
                if !project
                    .parameters
                    .iter()
                    .any(|(seen, _)| seen.eq_ignore_ascii_case(&n))
                {
                    project.parameters.push((n, v));
                }
            }
        }
        *name = None;
        *value = None;
    };
    let mut boards = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.trim_end().strip_prefix('[') {
            finish(&section, &mut name, &mut value, &mut keys);
            sections = sections.saturating_add(1);
            if sections > MAX_SECTIONS {
                return Err(ReadError::Invalid(format!(
                    "more than {MAX_SECTIONS} sections"
                )));
            }
            section = rest.strip_suffix(']').unwrap_or(rest).to_string();
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim_end();
        if is_numbered(&section, "Document") && key == "DocumentPath" && is_board(val.trim_end()) {
            boards.push(val.trim_end().to_string());
        } else if is_numbered(&section, "ProjectVariant") {
            if val.len() > MAX_PARAMETER_VALUE {
                return Err(ReadError::Invalid(format!(
                    "a line in [{section}] is {} bytes long; the most read is \
                     {MAX_PARAMETER_VALUE}",
                    val.len()
                )));
            }
            keys.push((
                key.to_string(),
                val.trim_end_matches(['\r', '\n']).to_string(),
            ));
        } else if is_numbered(&section, "Parameter") {
            match key {
                "Name" => name = Some(val.trim().to_string()),
                "Value" if val.len() > MAX_PARAMETER_VALUE => {
                    return Err(ReadError::Invalid(format!(
                        "the value in [{section}] is {} bytes long; the most read is \
                         {MAX_PARAMETER_VALUE}",
                        val.len()
                    )));
                }
                "Value" => value = Some(val.to_string()),
                _ => {}
            }
        }
    }
    finish(&section, &mut name, &mut value, &mut keys);
    project.boards = boards;
    Ok(project)
}

/// One `[ProjectVariantN]` section's variant, from its `key=value` lines, and notes on what of it
/// is not read.
fn variant(keys: &[(String, String)]) -> (Variant, Vec<Note>) {
    let get = |key: &str| keys.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
    let mut v = Variant {
        name: get("Description").unwrap_or_default().to_string(),
        ..Variant::default()
    };
    let mut notes = Vec::new();
    let note = |severity, message: String| Note { severity, message };
    // The changed parameters first: a fitted part's AlternatePart is checked against them.
    let mut params: Vec<(String, String, String)> = Vec::new();
    for (key, line) in keys {
        let Some(n) = key.strip_prefix("ParamVariation") else {
            continue;
        };
        if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let fields = fields(line);
        let designator = get(&format!("ParamDesignator{n}")).unwrap_or_default();
        let parameter = fields.get("ParameterName").copied().unwrap_or_default();
        let new_value = fields.get("VariantValue").copied().unwrap_or_default();
        params.push((
            designator.to_string(),
            parameter.to_string(),
            new_value.to_string(),
        ));
    }
    for (designator, parameter, new_value) in &params {
        if parameter.eq_ignore_ascii_case("Comment") {
            v.values.push(VariantValue {
                designator: designator.clone(),
                comment: new_value.clone(),
            });
        } else {
            notes.push(note(
                Severity::Info,
                format!(
                    "variant \"{}\" sets the parameter {parameter} of {designator} to \
                     {new_value}; only a part's Comment is read",
                    v.name
                ),
            ));
        }
    }
    for (key, line) in keys {
        let Some(n) = key.strip_prefix("Variation") else {
            continue;
        };
        if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let fields = fields(line);
        let designator = fields.get("Designator").copied().unwrap_or_default();
        let alternate = fields.get("AlternatePart").copied().unwrap_or_default();
        match fields.get("Kind").map(|k| k.trim()) {
            Some("1") => v.not_fitted.push(designator.to_string()),
            Some("0") => {
                let only_value = params.iter().any(|(d, p, value)| {
                    d == designator && p.eq_ignore_ascii_case("Comment") && value == alternate
                });
                if !alternate.is_empty() && !only_value {
                    notes.push(note(
                        Severity::Warning,
                        format!(
                            "variant \"{}\" fits the alternate part {alternate} for \
                             {designator}; alternate part not applied",
                            v.name
                        ),
                    ));
                }
            }
            kind => notes.push(note(
                Severity::Warning,
                format!(
                    "variant \"{}\" has a variation of {designator} of kind {}, not read",
                    v.name,
                    kind.unwrap_or("?")
                ),
            )),
        }
    }
    (v, notes)
}

/// The `key=value` fields of a `|`-separated line, each value as written.
fn fields(line: &str) -> std::collections::BTreeMap<&str, &str> {
    line.split('|')
        .filter_map(|f| f.split_once('='))
        .map(|(k, v)| (k.trim(), v))
        .collect()
}

/// What is wrong with a link from a project to one of its documents, or `None` for one that
/// stays inside the project's folder: an absolute path (`C:\...`, `\\server\...`, `/...`) or
/// one that climbs out with `..`. The test project's links are plain file names.
pub fn link_problem(link: &str) -> Option<&'static str> {
    let bytes = link.as_bytes();
    let drive = bytes.get(1) == Some(&b':') && bytes.first().is_some_and(u8::is_ascii_alphabetic);
    if drive || link.starts_with(['\\', '/']) {
        return Some("is absolute");
    }
    if link.split(['\\', '/']).any(|part| part == "..") {
        return Some("leaves the project's folder");
    }
    None
}

/// True for `Document7` when `word` is "Document": the word and then only digits.
fn is_numbered(section: &str, word: &str) -> bool {
    section
        .strip_prefix(word)
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// True when a document path names a board (`.PcbDoc`, in any case).
fn is_board(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("pcbdoc"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = "\u{feff}[Design]\r\nVersion=1.0\r\n\r\n[Document1]\r\n\
        DocumentPath=Main.SchDoc\r\n\r\n[Document2]\r\nDocumentPath=boards\\Main.PcbDoc\r\n\r\n\
        [Parameter1]\r\nName=Rev\r\nValue=C\r\n\r\n[Parameter2]\r\nName=rev\r\nValue=D\r\n\r\n\
        [Parameter3]\r\nName=Empty\r\nValue=\r\n\r\n[Configuration1]\r\nName=Sources\r\n";

    #[test]
    fn reads_boards_and_parameters() -> Result<(), ReadError> {
        let p = read(SMALL.as_bytes())?;
        assert_eq!(p.boards, ["boards\\Main.PcbDoc"]);
        // "rev" repeats "Rev" in another case and is left out; [Configuration1]'s Name is not a
        // parameter.
        assert_eq!(
            p.parameters,
            [
                ("Rev".to_string(), "C".to_string()),
                ("Empty".to_string(), String::new())
            ]
        );
        Ok(())
    }

    #[test]
    fn reads_indented_lines_and_keeps_a_value_as_written() -> Result<(), ReadError> {
        let text = "[Design]\r\n  [Document1]  \r\n\tDocumentPath=a.PcbDoc \r\n\
            [Parameter1]\r\n  Name = Rev\r\n  Value= B \r\n";
        let p = read(text.as_bytes())?;
        assert_eq!(p.boards, ["a.PcbDoc"]);
        assert_eq!(p.parameters, [("Rev".to_string(), " B ".to_string())]);
        Ok(())
    }

    #[test]
    fn refuses_a_parameter_value_over_the_cap() -> Result<(), ReadError> {
        let at_cap = "x".repeat(MAX_PARAMETER_VALUE);
        let text = format!("[Design]\r\n[Parameter1]\r\nName=Big\r\nValue={at_cap}\r\n");
        let read_back = read(text.as_bytes())?;
        assert_eq!(
            read_back.parameters.first().map(|(_, v)| v.len()),
            Some(MAX_PARAMETER_VALUE)
        );
        let over = format!("[Design]\r\n[Parameter1]\r\nName=Big\r\nValue={at_cap}y\r\n");
        assert!(matches!(read(over.as_bytes()), Err(ReadError::Invalid(_))));
        Ok(())
    }

    #[test]
    fn refuses_what_is_not_a_project() {
        assert!(read(b"\xff\xfe binary").is_err());
        assert!(read(b"[Other]\r\nx=1\r\n").is_err());
        assert!(read(b"").is_err());
    }

    #[test]
    fn links_must_stay_in_the_project_folder() {
        assert_eq!(link_problem("param-board.PcbDoc"), None);
        assert_eq!(link_problem(r"boards\main.PcbDoc"), None);
        assert_eq!(link_problem(r"C:\Users\x\b.PcbDoc"), Some("is absolute"));
        assert_eq!(link_problem(r"\\server\b.PcbDoc"), Some("is absolute"));
        assert_eq!(link_problem("/home/x/b.PcbDoc"), Some("is absolute"));
        assert_eq!(
            link_problem(r"..\other\b.PcbDoc"),
            Some("leaves the project's folder")
        );
        assert_eq!(
            link_problem("a/../../b.PcbDoc"),
            Some("leaves the project's folder")
        );
    }

    const VARIANTS: &str = "\u{feff}[Design]\r\n[Document1]\r\nDocumentPath=b.PcbDoc\r\n\r\n\
        [ProjectVariant1]\r\nDescription=Swap\r\nVariationCount=3\r\n\
        Variation1=Designator=U1|UniqueId=\\AAAA|Kind=0|AlternatePart=LM358B\r\n\
        Variation2=Designator=C1|UniqueId=\\BBBB|Kind=1|AlternatePart=\r\n\
        Variation3=Designator=Q1|UniqueId=\\CCCC|Kind=7|AlternatePart=\r\n\
        ParamVariation1=ParameterName=Tolerance|VariantValue=1%\r\n\
        ParamDesignator1=R5\r\nParamVariationCount=1\r\n";

    #[test]
    fn a_variant_s_alternate_part_other_parameters_and_unknown_kinds_get_notes(
    ) -> Result<(), ReadError> {
        let p = read(VARIANTS.as_bytes())?;
        assert_eq!(
            p.variants,
            [Variant {
                name: "Swap".into(),
                not_fitted: vec!["C1".into()],
                fitted: Vec::new(),
                values: Vec::new(),
            }]
        );
        let messages: Vec<&str> = p.notes.iter().map(|n| n.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "variant \"Swap\" sets the parameter Tolerance of R5 to 1%; only a part's \
                 Comment is read",
                "variant \"Swap\" fits the alternate part LM358B for U1; alternate part not \
                 applied",
                "variant \"Swap\" has a variation of Q1 of kind 7, not read",
            ]
        );
        Ok(())
    }

    #[test]
    fn variants_name_parts_the_board_may_not_have() -> Result<(), ReadError> {
        let p = read(VARIANTS.as_bytes())?;
        let mut board = Board::default();
        let notes = add_variants(&mut board, &p);
        assert_eq!(board.variants, p.variants);
        assert_eq!(
            notes.last().map(|n| n.message.as_str()),
            Some("variant \"Swap\" names C1 that the board does not have")
        );
        Ok(())
    }

    #[test]
    fn numbered_sections() {
        assert!(is_numbered("Document12", "Document"));
        assert!(!is_numbered("Document", "Document"));
        assert!(!is_numbered("DocumentX", "Document"));
        assert!(!is_numbered("OutputGroup1", "Document"));
    }
}
