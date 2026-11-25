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
//! Everything else in the file (output settings, printer settings, variants) is not read.
//! Like a board, the bytes are handed in; this never opens a file.

use board_model::{ReadError, MAX_INPUT_BYTES};

/// The most sections read before the file is treated as hostile.
const MAX_SECTIONS: usize = 100_000;

/// What a project says about its boards and parameters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    /// The boards' paths as the project gives them, relative to the project's folder, with `\`
    /// as Altium writes it.
    pub boards: Vec<String>,
    /// The project parameters, as (name, value), in the file's order. A name that appears twice
    /// (in any case) keeps its first value.
    pub parameters: Vec<(String, String)>,
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
    let mut finish = |section: &str, name: &mut Option<String>, value: &mut Option<String>| {
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
        let line = line.trim();
        if let Some(rest) = line.strip_prefix('[') {
            finish(&section, &mut name, &mut value);
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
        if is_numbered(&section, "Document") && key == "DocumentPath" && is_board(val) {
            boards.push(val.to_string());
        } else if is_numbered(&section, "Parameter") {
            match key {
                "Name" => name = Some(val.to_string()),
                "Value" => value = Some(val.to_string()),
                _ => {}
            }
        }
    }
    finish(&section, &mut name, &mut value);
    project.boards = boards;
    Ok(project)
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
    fn refuses_what_is_not_a_project() {
        assert!(read(b"\xff\xfe binary").is_err());
        assert!(read(b"[Other]\r\nx=1\r\n").is_err());
        assert!(read(b"").is_err());
    }

    #[test]
    fn numbered_sections() {
        assert!(is_numbered("Document12", "Document"));
        assert!(!is_numbered("Document", "Document"));
        assert!(!is_numbered("DocumentX", "Document"));
        assert!(!is_numbered("OutputGroup1", "Document"));
    }
}
