//! Texts that refer to a project parameter or to one of Altium's own special strings, such as
//! `.PCBNumber` or `.Designator`.
//!
//! The file stores a text as typed; Altium shows a value in its place. Worked out from our own
//! CC0 test boards (see LEGAL.md and tests/boards/param-project/README.txt and README.txt).
//!
//! Project parameters, on `param-project` with `PCBNumber` = `DN-0042` and `BoardRevision` =
//! `B` (Altium 26.12.2):
//! - `.PCBNumber` shows `DN-0042`, on overlay and copper layers and mirrored on the bottom
//!   alike; `.pcbnumber` the same (the name is matched in any case);
//! - `Rev .BoardRevision` shows `Rev B`, `X.PCBNumber` `XDN-0042`, `a.b.PCBNumber`
//!   `a.bDN-0042`, `a/.PCBNumber` `a/DN-0042` and `x..PCBNumber` `x.DN-0042`;
//! - unchanged: `.NoSuchParameter`, `.PCBNumberX`, `.BoardRevision.`, `..PCBNumber`,
//!   `.x.PCBNumber`, `.PCBNumber/.BoardRevision`, `.PCBNumber rev` and `=PCBNumber`.
//!
//! Special strings the board itself determines, on `special-strings` (a copy of `one-resistor`,
//! whose R1 has the comment "10k"):
//! - on a text that belongs to R1: `.Designator` shows `R1`, `.designator` `R1`, `.Comment`
//!   `10k` and `R: .Designator` `R: R1`;
//! - on a free text, `.Designator` and `.Comment` stay as they are;
//! - `.Layer_Name` shows the name of the text's own layer, on a free text or one of R1's:
//!   `Top Overlay`, `Top Layer`, `Bottom Overlay`; `L: .Layer_Name` shows `L: Top Overlay`;
//! - `.Print_Date` shows the day the board is looked at. It is left as it is, with a note: a
//!   conversion should not stamp a date onto a board.
//!
//! One rule gives all of these: a text that starts with `.` is replaced when everything after
//! that dot names a value; any other text is replaced when the part after its last `.` names a
//! value, keeping what comes before that dot. A text holds at most one reference. A parameter
//! that has the name of a special string has not been tried in Altium; here the special string
//! wins.

use std::collections::BTreeMap;

use board_model::{Board, Note, Severity, Text};

/// The most names given a note of their own; the rest are counted in one note.
const MAX_NAMES: usize = 20;

/// Special strings that show a date or time when Altium draws the board.
const DATES: [&str; 1] = ["Print_Date"];

/// Replaces every reference in the board's texts to a parameter in `parameters` (name, value)
/// or to `.Designator`, `.Comment` or `.Layer_Name`, and returns the notes: how many texts were
/// replaced, and one note for each name, among the texts that start with `.`, that was left as
/// it is.
pub fn resolve_texts(board: &mut Board, parameters: &[(String, String)]) -> Vec<Note> {
    let mut replaced = 0usize;
    // Name as first written (any case folded) -> (as written, texts, first text).
    let mut left: BTreeMap<String, (String, usize, String)> = BTreeMap::new();
    let mut free_component_strings = 0usize;
    for i in 0..board.texts.len() {
        let Some(text) = board.texts.get(i) else {
            continue;
        };
        match substitute(text, board, parameters) {
            Some(value) => {
                if let Some(t) = board.texts.get_mut(i) {
                    t.content = value;
                }
                replaced = replaced.saturating_add(1);
            }
            None => {
                let Some(name) = text.content.strip_prefix('.') else {
                    continue;
                };
                if text.component.is_none()
                    && (name.eq_ignore_ascii_case("Designator")
                        || name.eq_ignore_ascii_case("Comment"))
                {
                    free_component_strings = free_component_strings.saturating_add(1);
                    continue;
                }
                let entry = left
                    .entry(name.to_ascii_lowercase())
                    .or_insert_with(|| (name.to_string(), 0, text.content.clone()));
                entry.1 = entry.1.saturating_add(1);
            }
        }
    }
    let mut notes = Vec::new();
    if replaced > 0 {
        notes.push(Note {
            severity: Severity::Info,
            message: format!(
                "{replaced} text(s) refer to a project parameter or to .Designator, .Comment or \
                 .Layer_Name and show its value, as Altium does"
            ),
        });
    }
    if free_component_strings > 0 {
        notes.push(Note {
            severity: Severity::Info,
            message: format!(
                "{free_component_strings} text(s) read .Designator or .Comment but belong to no \
                 component; they are drawn as written, as Altium does"
            ),
        });
    }
    let more = left.len().saturating_sub(MAX_NAMES);
    for (name, n, _) in left.values().take(MAX_NAMES) {
        let message = if DATES.iter().any(|d| d.eq_ignore_ascii_case(name)) {
            format!(
                "{n} text(s) read \".{name}\", which Altium fills in with a date or time when it \
                 draws the board; drawn as written, since a conversion should not stamp a date \
                 onto a board"
            )
        } else {
            format!(
                "{n} text(s) read \".{name}\", which names no parameter given and no special \
                 string read here; drawn as written"
            )
        };
        notes.push(Note {
            severity: Severity::Warning,
            message,
        });
    }
    if more > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "and {more} more name(s) after a \".\" that were drawn as written, too"
            ),
        });
    }
    notes
}

/// The text with its reference replaced by the value it names, or `None` when it names none.
fn substitute(text: &Text, board: &Board, parameters: &[(String, String)]) -> Option<String> {
    let (before, name) = match text.content.strip_prefix('.') {
        Some(rest) => ("", rest),
        None => text.content.rsplit_once('.')?,
    };
    let value = special(text, board, name).or_else(|| {
        parameters
            .iter()
            .find(|(n, _)| !n.is_empty() && n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    })?;
    Some(format!("{before}{value}"))
}

/// The value of one of Altium's special strings that the board determines, for this text.
fn special(text: &Text, board: &Board, name: &str) -> Option<String> {
    let component = || text.component.and_then(|c| board.components.get(c));
    if name.eq_ignore_ascii_case("Designator") {
        component().map(|c| c.designator.clone())
    } else if name.eq_ignore_ascii_case("Comment") {
        component().map(|c| c.comment.clone())
    } else if name.eq_ignore_ascii_case("Layer_Name") {
        board.layers.get(text.layer).map(|l| l.name.clone())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use board_model::{Layer, LayerKind, Placement, Point, Side, TextFont};

    fn params() -> Vec<(String, String)> {
        vec![
            ("PCBNumber".into(), "DN-0042".into()),
            ("BoardRevision".into(), "B".into()),
        ]
    }

    fn text(content: &str, layer: usize, component: Option<usize>) -> Text {
        Text {
            layer,
            at: Point { x: 0.0, y: 0.0 },
            content: content.into(),
            height: 1.5,
            rotation_deg: 0.0,
            mirrored: false,
            font: TextFont::Stroke { width: 0.15 },
            component,
        }
    }

    fn board(texts: Vec<Text>) -> Board {
        let layer = |name: &str, kind| Layer {
            name: name.into(),
            kind,
            side: Some(Side::Top),
        };
        Board {
            layers: vec![
                layer("Top Layer", LayerKind::Copper),
                layer("Top Overlay", LayerKind::Silkscreen),
            ],
            components: vec![Placement {
                designator: "R1".into(),
                footprint: "R0603".into(),
                at: Point { x: 25.0, y: 15.0 },
                rotation_deg: 0.0,
                side: Side::Top,
                model: None,
                comment: "10k".into(),
            }],
            texts,
            ..Board::default()
        }
    }

    fn resolved(content: &str, component: Option<usize>) -> String {
        let mut b = board(vec![text(content, 1, component)]);
        resolve_texts(&mut b, &params());
        b.texts
            .first()
            .map(|t| t.content.clone())
            .unwrap_or_default()
    }

    #[test]
    fn follows_what_altium_showed_for_parameters() {
        for (content, shown) in [
            (".PCBNumber", "DN-0042"),
            (".BoardRevision", "B"),
            ("Rev .BoardRevision", "Rev B"),
            (".pcbnumber", "DN-0042"),
            ("X.PCBNumber", "XDN-0042"),
            ("a.b.PCBNumber", "a.bDN-0042"),
            ("a/.PCBNumber", "a/DN-0042"),
            ("x..PCBNumber", "x.DN-0042"),
            (".NoSuchParameter", ".NoSuchParameter"),
            (".PCBNumberX", ".PCBNumberX"),
            (".BoardRevision.", ".BoardRevision."),
            ("..PCBNumber", "..PCBNumber"),
            (".x.PCBNumber", ".x.PCBNumber"),
            (".PCBNumber/.BoardRevision", ".PCBNumber/.BoardRevision"),
            (".PCBNumber rev", ".PCBNumber rev"),
            ("=PCBNumber", "=PCBNumber"),
            ("plain text", "plain text"),
        ] {
            assert_eq!(resolved(content, None), shown, "{content}");
        }
    }

    #[test]
    fn follows_what_altium_showed_for_special_strings() {
        for (content, component, shown) in [
            (".Designator", Some(0), "R1"),
            (".designator", Some(0), "R1"),
            (".Comment", Some(0), "10k"),
            ("R: .Designator", Some(0), "R: R1"),
            (".Layer_Name", Some(0), "Top Overlay"),
            (".Designator", None, ".Designator"),
            (".Comment", None, ".Comment"),
            (".Layer_Name", None, "Top Overlay"),
            ("L: .Layer_Name", None, "L: Top Overlay"),
            (".layer_name", None, "Top Overlay"),
            (".Print_Date", None, ".Print_Date"),
        ] {
            assert_eq!(resolved(content, component), shown, "{content}");
        }
        // The layer is the text's own.
        let mut b = board(vec![text(".Layer_Name", 0, None)]);
        resolve_texts(&mut b, &[]);
        assert_eq!(
            b.texts.first().map(|t| t.content.as_str()),
            Some("Top Layer")
        );
    }

    #[test]
    fn notes_say_what_was_replaced_and_name_each_name_left() {
        let mut b = board(vec![
            text(".PCBNumber", 1, None),
            text(".Designator", 1, Some(0)),
            text(".Designator", 1, None),
            text(".Print_Date", 1, None),
            text(".NoSuch", 1, None),
            text(".nosuch", 1, None),
            text("v1.2", 1, None),
        ]);
        let notes = resolve_texts(&mut b, &params());
        let got: Vec<_> = notes
            .iter()
            .map(|n| (n.severity, n.message.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                (
                    Severity::Info,
                    "2 text(s) refer to a project parameter or to .Designator, .Comment or \
                     .Layer_Name and show its value, as Altium does"
                ),
                (
                    Severity::Info,
                    "1 text(s) read .Designator or .Comment but belong to no component; they \
                     are drawn as written, as Altium does"
                ),
                (
                    Severity::Warning,
                    "2 text(s) read \".NoSuch\", which names no parameter given and no special \
                     string read here; drawn as written"
                ),
                (
                    Severity::Warning,
                    "1 text(s) read \".Print_Date\", which Altium fills in with a date or time \
                     when it draws the board; drawn as written, since a conversion should not \
                     stamp a date onto a board"
                ),
            ]
        );
    }

    #[test]
    fn many_names_are_counted_after_the_first_twenty() {
        let mut b = board(
            (0..25)
                .map(|i| text(&format!(".P{i:02}"), 1, None))
                .collect(),
        );
        let notes = resolve_texts(&mut b, &[]);
        assert_eq!(notes.len(), 21);
        assert_eq!(
            notes.last().map(|n| n.message.as_str()),
            Some("and 5 more name(s) after a \".\" that were drawn as written, too")
        );
    }

    #[test]
    fn nothing_is_replaced_by_an_empty_name() {
        let empty = vec![(String::new(), "x".to_string())];
        let mut b = board(vec![text("ends with a dot.", 1, None), text(".", 1, None)]);
        resolve_texts(&mut b, &empty);
        let got: Vec<_> = b.texts.iter().map(|t| t.content.as_str()).collect();
        assert_eq!(got, ["ends with a dot.", "."]);
    }
}
