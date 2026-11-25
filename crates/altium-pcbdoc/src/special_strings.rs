//! Texts that refer to a project parameter ("special strings"), such as `.PCBNumber`.
//!
//! Worked out from our own CC0 test project `param-project` (see tests/boards/param-project and
//! LEGAL.md). The file stores a text as typed; Altium shows a parameter's value in its place when
//! the board is open in its project. With the parameters `PCBNumber` = `DN-0042` and
//! `BoardRevision` = `B`, Altium 26.12.2 showed:
//!
//! - `.PCBNumber` as `DN-0042`, on overlay and copper layers and mirrored on the bottom alike;
//!   `.pcbnumber` the same (the name is matched in any case);
//! - `Rev .BoardRevision` as `Rev B`, `X.PCBNumber` as `XDN-0042`, `a.b.PCBNumber` as
//!   `a.bDN-0042`, `a/.PCBNumber` as `a/DN-0042` and `x..PCBNumber` as `x.DN-0042`;
//! - unchanged: `.NoSuchParameter`, `.PCBNumberX`, `.BoardRevision.`, `..PCBNumber`,
//!   `.x.PCBNumber`, `.PCBNumber/.BoardRevision`, `.PCBNumber rev` and `=PCBNumber`;
//! - `.Layer_Name` as `Top Overlay`: one of Altium's own special strings, not a parameter.
//!
//! One rule gives all of these: a text that starts with `.` is replaced when everything after
//! that dot names a parameter; any other text is replaced when the part after its last `.` names
//! a parameter, and keeps what comes before that dot. A text holds at most one reference.

use board_model::{Note, Severity, Text};

/// The most texts named one by one in a note; the rest are counted.
const MAX_LISTED: usize = 5;

/// The text with its reference replaced by the parameter's value, or `None` when it refers to
/// no parameter in `parameters`.
pub(crate) fn substitute(text: &str, parameters: &[(String, String)]) -> Option<String> {
    let (before, name) = match text.strip_prefix('.') {
        Some(rest) => ("", rest),
        None => text.rsplit_once('.')?,
    };
    let (_, value) = parameters
        .iter()
        .find(|(n, _)| !n.is_empty() && n.eq_ignore_ascii_case(name))?;
    Some(format!("{before}{value}"))
}

/// Replaces every reference to a parameter in the texts, and notes what was replaced and which
/// texts that start with `.` were left as they are (a parameter not given, or one of Altium's
/// own special strings, which are not read yet).
pub(crate) fn resolve(texts: &mut [Text], parameters: &[(String, String)], notes: &mut Vec<Note>) {
    let mut replaced = 0usize;
    let mut left = Vec::new();
    for text in texts.iter_mut() {
        match substitute(&text.content, parameters) {
            Some(value) => {
                text.content = value;
                replaced = replaced.saturating_add(1);
            }
            None if text.content.starts_with('.') => left.push(text.content.clone()),
            None => {}
        }
    }
    if replaced > 0 {
        notes.push(Note {
            severity: Severity::Info,
            message: format!(
                "{replaced} text(s) refer to a project parameter and show its value, as Altium \
                 does"
            ),
        });
    }
    if !left.is_empty() {
        left.sort();
        left.dedup();
        let more = left.len().saturating_sub(MAX_LISTED);
        let mut listed: Vec<String> = left
            .iter()
            .take(MAX_LISTED)
            .map(|t| format!("\"{t}\""))
            .collect();
        if more > 0 {
            listed.push(format!("and {more} more"));
        }
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "text(s) {} may be Altium special strings that refer to a project parameter or to \
                 the board; no value was given for them, so they are drawn as written",
                listed.join(", ")
            ),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use board_model::{Point, TextFont};

    fn params() -> Vec<(String, String)> {
        vec![
            ("PCBNumber".into(), "DN-0042".into()),
            ("BoardRevision".into(), "B".into()),
        ]
    }

    #[test]
    fn follows_what_altium_showed_on_param_board() {
        let p = params();
        let cases = [
            (".PCBNumber", Some("DN-0042")),
            (".BoardRevision", Some("B")),
            ("Rev .BoardRevision", Some("Rev B")),
            (".pcbnumber", Some("DN-0042")),
            ("X.PCBNumber", Some("XDN-0042")),
            ("a.b.PCBNumber", Some("a.bDN-0042")),
            ("a/.PCBNumber", Some("a/DN-0042")),
            ("x..PCBNumber", Some("x.DN-0042")),
            (".NoSuchParameter", None),
            (".PCBNumberX", None),
            (".BoardRevision.", None),
            ("..PCBNumber", None),
            (".x.PCBNumber", None),
            (".PCBNumber/.BoardRevision", None),
            (".PCBNumber rev", None),
            ("=PCBNumber", None),
            (".Layer_Name", None),
            ("plain text", None),
        ];
        for (text, want) in cases {
            assert_eq!(substitute(text, &p).as_deref(), want, "{text}");
        }
    }

    #[test]
    fn nothing_is_replaced_without_parameters_or_by_an_empty_name() {
        assert_eq!(substitute(".PCBNumber", &[]), None);
        let empty = vec![(String::new(), "x".to_string())];
        assert_eq!(substitute("ends with a dot.", &empty), None);
        assert_eq!(substitute(".", &empty), None);
    }

    fn text(content: &str) -> Text {
        Text {
            layer: 0,
            at: Point { x: 0.0, y: 0.0 },
            content: content.into(),
            height: 1.5,
            rotation_deg: 0.0,
            mirrored: false,
            font: TextFont::Stroke { width: 0.15 },
        }
    }

    #[test]
    fn resolve_replaces_and_notes_what_is_left() {
        let mut texts = [
            text(".PCBNumber"),
            text("Rev .BoardRevision"),
            text(".Layer_Name"),
            text("v1.2"),
        ];
        let mut notes = Vec::new();
        resolve(&mut texts, &params(), &mut notes);
        let contents: Vec<_> = texts.iter().map(|t| t.content.as_str()).collect();
        assert_eq!(contents, ["DN-0042", "Rev B", ".Layer_Name", "v1.2"]);
        let messages: Vec<_> = notes
            .iter()
            .map(|n| (n.severity, n.message.as_str()))
            .collect();
        assert_eq!(
            messages,
            [
                (
                    Severity::Info,
                    "2 text(s) refer to a project parameter and show its value, as Altium does"
                ),
                (
                    Severity::Warning,
                    "text(s) \".Layer_Name\" may be Altium special strings that refer to a \
                     project parameter or to the board; no value was given for them, so they \
                     are drawn as written"
                ),
            ]
        );
    }

    #[test]
    fn many_left_texts_are_counted_after_the_first_few() {
        let mut texts: Vec<Text> = (0..8).map(|i| text(&format!(".P{i}"))).collect();
        let mut notes = Vec::new();
        resolve(&mut texts, &[], &mut notes);
        let [note] = notes.as_slice() else {
            return assert_eq!(notes.len(), 1);
        };
        assert!(
            note.message.starts_with(
                "text(s) \".P0\", \".P1\", \".P2\", \".P3\", \".P4\", and 3 more may be"
            ),
            "{}",
            note.message
        );
    }
}
