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
//! Lines may be indented; a key is read without the spaces around it, and a value exactly as
//! written after the `=` (whether or not Altium keeps spaces at a value's end, keeping what the
//! file holds is right). A parameter value longer than [`MAX_PARAMETER_VALUE`] is refused, since
//! every text that refers to the parameter gets its own copy.
//!
//! Everything else in the file (output settings, printer settings, variants) is not read.
//! Like a board, the bytes are handed in; this never opens a file.

use board_model::{ReadError, MAX_INPUT_BYTES};

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
        let line = line.trim_start();
        if let Some(rest) = line.trim_end().strip_prefix('[') {
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
        let key = key.trim_end();
        if is_numbered(&section, "Document") && key == "DocumentPath" && is_board(val.trim_end()) {
            boards.push(val.trim_end().to_string());
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
    finish(&section, &mut name, &mut value);
    project.boards = boards;
    Ok(project)
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

    #[test]
    fn numbered_sections() {
        assert!(is_numbered("Document12", "Document"));
        assert!(!is_numbered("Document", "Document"));
        assert!(!is_numbered("DocumentX", "Document"));
        assert!(!is_numbered("OutputGroup1", "Document"));
    }
}
