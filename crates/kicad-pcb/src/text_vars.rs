//! KiCad's text variables: a `${NAME}` in a text shows a value when KiCad draws the board.
//!
//! What KiCad shows was recorded on our own board `k10-text-vars` (its README.txt has the table:
//! the stored string, what KiCad 10's `GetShownText` gives, for every text). The rules that give
//! all of those rows:
//!
//! - `${NAME}` is replaced by the value of NAME, wherever it stands and as often as it occurs.
//!   Names are case-sensitive (`${revision}` is not `${REVISION}`). A `$` not followed by `{` is
//!   kept (`$${REVISION}` shows `$B2`). A `${` without a closing `}` takes the rest of the text
//!   as the name (`Rev ${REVISION` shows `Rev B2`).
//! - A name that names nothing stays as typed (`${NOSUCHVAR}`).
//! - A value can itself hold variables (`BOARD_ID = ${PCB_NUMBER}-${REVISION}`); they are filled
//!   in too, at most [`MAX_DEPTH`] deep, and a text never grows past [`MAX_LEN`] bytes. A name
//!   whose value leads back to itself is a cycle. In those three cases the text stays as stored.
//!
//! Where a name is looked up, first match wins:
//! 1. in a footprint's own texts and fields: `REFERENCE`, `VALUE` and the footprint's fields by
//!    their names (`${MPN}`);
//! 2. `LAYER`: the name KiCad shows for the text's layer (`F.Silkscreen`);
//! 3. `REF:FIELD`: the field of the footprint whose reference is REF (`${J1:VALUE}`);
//! 4. the board's title block: `TITLE`, `REVISION`, `COMPANY`, `ISSUE_DATE`, `COMMENT1` to
//!    `COMMENT9`;
//! 5. `FILENAME` and `PROJECTNAME`, from the board's file name only (never a folder);
//! 6. the project's text variables ([`Variables::project`]: the `.kicad_pro`, then `--param`);
//! 7. the board's own properties (`(property "NAME" "value")` in the board file).
//!
//! The order between 4 to 7 when two of them share a name was not seen in KiCad; it is ours.

use std::collections::{BTreeMap, BTreeSet};

use board_model::{Note, Severity};

/// The most levels of variables inside variables that are filled in.
pub const MAX_DEPTH: usize = 8;

/// The longest a text may grow with its variables filled in, in bytes.
pub const MAX_LEN: usize = 4096;

/// The most unknown names given a note of their own; the rest are counted in one note.
const MAX_NAMES: usize = 20;

/// What a board's texts may refer to besides the board file itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Variables {
    /// The board's file name, without any folder (`main.kicad_pcb`), for `${FILENAME}` and
    /// `${PROJECTNAME}`. `None` leaves both as typed.
    pub file_name: Option<String>,
    /// The project's text variables (name, value), the first of a name winning.
    pub project: Vec<(String, String)>,
}

/// One footprint's fields as stored: (name, value), `Reference` and `Value` among them.
#[derive(Debug, Default)]
pub(crate) struct Fields {
    pub reference: String,
    pub fields: Vec<(String, String)>,
}

impl Fields {
    fn get(&self, name: &str) -> Option<&str> {
        let field = match name {
            "REFERENCE" => "Reference",
            "VALUE" => "Value",
            other => other,
        };
        self.fields
            .iter()
            .find(|(n, _)| n == field)
            .map(|(_, v)| v.as_str())
    }
}

/// Everything a name can be looked up in.
pub(crate) struct Scope<'a> {
    /// (name, value): `TITLE`, `REVISION`, `COMPANY`, `ISSUE_DATE`, `COMMENTn`.
    pub title_block: Vec<(String, String)>,
    /// The board file's own properties.
    pub properties: Vec<(String, String)>,
    pub footprints: &'a [Fields],
    pub variables: &'a Variables,
}

/// Why a text stays as stored.
enum Trouble {
    Cycle(String),
    Deep,
    Long,
}

/// Fills in the variables of texts, counting what it could not fill in for the notes.
pub(crate) struct Resolver<'a> {
    scope: &'a Scope<'a>,
    filled: usize,
    /// Name -> how many texts use it without it naming anything.
    unknown: BTreeMap<String, usize>,
    /// Name -> how many texts hold a cycle through it.
    cycles: BTreeMap<String, usize>,
    deep: usize,
    long: usize,
}

impl<'a> Resolver<'a> {
    pub fn new(scope: &'a Scope<'a>) -> Self {
        Resolver {
            scope,
            filled: 0,
            unknown: BTreeMap::new(),
            cycles: BTreeMap::new(),
            deep: 0,
            long: 0,
        }
    }

    /// The text `raw` as KiCad shows it: on the layer KiCad shows as `layer`, in the footprint
    /// `owner` (an index into the scope's footprints) or free (`None`).
    pub fn resolve(&mut self, raw: &str, owner: Option<usize>, layer: &str) -> String {
        if !raw.contains("${") {
            return raw.to_string();
        }
        let mut out = String::new();
        let mut unknown = BTreeSet::new();
        let mut stack = Vec::new();
        match self.expand(raw, owner, layer, 0, &mut stack, &mut unknown, &mut out) {
            Ok(()) => {
                for name in unknown {
                    *self.unknown.entry(name).or_default() += 1;
                }
                if out != raw {
                    self.filled += 1;
                }
                out
            }
            Err(Trouble::Cycle(name)) => {
                *self.cycles.entry(name).or_default() += 1;
                raw.to_string()
            }
            Err(Trouble::Deep) => {
                self.deep += 1;
                raw.to_string()
            }
            Err(Trouble::Long) => {
                self.long += 1;
                raw.to_string()
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn expand(
        &self,
        text: &str,
        owner: Option<usize>,
        layer: &str,
        depth: usize,
        stack: &mut Vec<(String, Option<usize>)>,
        unknown: &mut BTreeSet<String>,
        out: &mut String,
    ) -> Result<(), Trouble> {
        let mut rest = text;
        while let Some(i) = rest.find("${") {
            out.push_str(rest.get(..i).unwrap_or_default());
            let after = rest.get(i + 2..).unwrap_or_default();
            let (name, closed, next) = match after.find('}') {
                Some(j) => (
                    after.get(..j).unwrap_or_default(),
                    true,
                    after.get(j + 1..).unwrap_or_default(),
                ),
                None => (after, false, ""),
            };
            match self.lookup(name, owner, layer) {
                Some((value, scope_owner)) => {
                    let key = (name.to_string(), owner);
                    if stack.contains(&key) {
                        return Err(Trouble::Cycle(name.to_string()));
                    }
                    if depth >= MAX_DEPTH {
                        return Err(Trouble::Deep);
                    }
                    stack.push(key);
                    self.expand(value, scope_owner, layer, depth + 1, stack, unknown, out)?;
                    stack.pop();
                }
                None => {
                    if !name.is_empty() {
                        unknown.insert(name.to_string());
                    }
                    out.push_str("${");
                    out.push_str(name);
                    if closed {
                        out.push('}');
                    }
                }
            }
            if out.len() > MAX_LEN {
                return Err(Trouble::Long);
            }
            rest = next;
        }
        out.push_str(rest);
        if out.len() > MAX_LEN {
            return Err(Trouble::Long);
        }
        Ok(())
    }

    /// The value `name` has for a text in `owner` on `layer`, and the footprint its own variables
    /// are looked up in.
    fn lookup<'s>(
        &'s self,
        name: &str,
        owner: Option<usize>,
        layer: &'s str,
    ) -> Option<(&'s str, Option<usize>)> {
        let s = self.scope;
        if let Some(v) = owner
            .and_then(|i| s.footprints.get(i))
            .and_then(|f| f.get(name))
        {
            return Some((v, owner));
        }
        if name == "LAYER" {
            return Some((layer, owner));
        }
        if let Some((reference, field)) = name.split_once(':') {
            let i = s.footprints.iter().position(|f| f.reference == reference)?;
            return s
                .footprints
                .get(i)
                .and_then(|f| f.get(field))
                .map(|v| (v, Some(i)));
        }
        let find = |list: &'s [(String, String)]| {
            list.iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| (v.as_str(), owner))
        };
        if let Some(found) = find(&s.title_block) {
            return Some(found);
        }
        if let Some(file) = &s.variables.file_name {
            match name {
                "FILENAME" => return Some((file.as_str(), owner)),
                "PROJECTNAME" => {
                    let stem = file
                        .rsplit_once('.')
                        .map_or(file.as_str(), |(stem, _)| stem);
                    return Some((stem, owner));
                }
                _ => {}
            }
        }
        find(&s.variables.project).or_else(|| find(&s.properties))
    }

    /// The notes on what was filled in and what was not.
    pub fn notes(self) -> Vec<Note> {
        let mut notes = Vec::new();
        if self.filled > 0 {
            notes.push(Note {
                severity: Severity::Info,
                message: format!(
                    "{} text(s) use KiCad text variables and show their values, as KiCad does",
                    self.filled
                ),
            });
        }
        let warn = |message: String| Note {
            severity: Severity::Warning,
            message,
        };
        for (name, n) in self.unknown.iter().take(MAX_NAMES) {
            notes.push(warn(if name == "CURRENT_DATE" {
                format!(
                    "{n} text(s) use the text variable ${{{name}}}, which KiCad fills in with the \
                     date it draws the board on; drawn as written, since a conversion should not \
                     stamp a date onto a board"
                )
            } else {
                format!(
                    "{n} text(s) use the text variable ${{{name}}}, which neither the board nor \
                     its project defines; drawn as written"
                )
            }));
        }
        let more = self.unknown.len().saturating_sub(MAX_NAMES);
        if more > 0 {
            notes.push(warn(format!(
                "and {more} more text variable name(s) that nothing defines were drawn as written, too"
            )));
        }
        for (name, n) in &self.cycles {
            notes.push(warn(format!(
                "{n} text(s) use the text variable ${{{name}}}, whose value leads back to itself; \
                 drawn as written"
            )));
        }
        if self.deep > 0 {
            notes.push(warn(format!(
                "{} text(s) hold text variables nested more than {MAX_DEPTH} deep; drawn as written",
                self.deep
            )));
        }
        if self.long > 0 {
            notes.push(warn(format!(
                "{} text(s) would grow past {MAX_LEN} bytes with their text variables filled in; \
                 drawn as written",
                self.long
            )));
        }
        notes
    }
}

/// The text variables of a KiCad project file (`.kicad_pro`, JSON): its `text_variables` object,
/// (name, value) sorted by name (a JSON object holds each name once). A project without one has
/// none.
pub fn project_variables(bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    let v: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| format!("not a KiCad project file: {e}"))?;
    let Some(root) = v.as_object() else {
        return Err("not a KiCad project file: not a JSON object".into());
    };
    let Some(vars) = root.get("text_variables") else {
        return Ok(Vec::new());
    };
    let Some(vars) = vars.as_object() else {
        return Err("the project's text_variables is not an object".into());
    };
    Ok(vars
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope_with<'a>(project: &'a Variables, footprints: &'a [Fields]) -> Scope<'a> {
        Scope {
            title_block: vec![("REVISION".into(), "B2".into())],
            properties: Vec::new(),
            footprints,
            variables: project,
        }
    }

    #[test]
    fn a_cycle_a_deep_nest_and_a_long_text_stay_as_stored() {
        let mut project = Variables {
            file_name: None,
            project: vec![
                ("A".into(), "x${B}".into()),
                ("B".into(), "${A}".into()),
                ("SELF".into(), "${SELF}".into()),
                ("BIG".into(), "${HALF}${HALF}".into()),
                ("HALF".into(), "y".repeat(MAX_LEN / 2 + 1)),
            ],
        };
        // N0 -> N1 -> ... -> N9 = "end": ten levels.
        for i in 0..9 {
            project
                .project
                .push((format!("N{i}"), format!("${{N{}}}", i + 1)));
        }
        project.project.push(("N9".into(), "end".into()));
        project.project.push(("N3OK".into(), "${N7}".into()));
        let fps = [];
        let scope = scope_with(&project, &fps);
        let mut r = Resolver::new(&scope);
        assert_eq!(r.resolve("${A}", None, "F.Cu"), "${A}");
        assert_eq!(r.resolve("[${SELF}]", None, "F.Cu"), "[${SELF}]");
        assert_eq!(r.resolve("${N0}", None, "F.Cu"), "${N0}");
        // Eight levels are filled in: N3OK -> N7 -> N8 -> N9.
        assert_eq!(r.resolve("${N3OK}", None, "F.Cu"), "end");
        assert_eq!(r.resolve("${BIG}", None, "F.Cu"), "${BIG}");
        assert_eq!(r.resolve("${REVISION}", None, "F.Cu"), "B2");
        let notes = r.notes();
        let has = |s: &str| notes.iter().any(|n| n.message.contains(s));
        assert!(has("2 text(s) use KiCad text variables"));
        assert!(has(
            "1 text(s) use the text variable ${A}, whose value leads back"
        ));
        assert!(has(
            "1 text(s) use the text variable ${SELF}, whose value leads back"
        ));
        assert!(has("1 text(s) hold text variables nested more than 8 deep"));
        assert!(has("1 text(s) would grow past 4096 bytes"));
        assert!(notes
            .iter()
            .filter(|n| !n.message.contains("show their values"))
            .all(|n| n.severity == Severity::Warning));
    }

    #[test]
    fn unknown_names_get_one_warning_each_counting_the_texts() {
        let project = Variables::default();
        let fps = [];
        let scope = scope_with(&project, &fps);
        let mut r = Resolver::new(&scope);
        assert_eq!(r.resolve("${X} ${X} ${Y}", None, "F.Cu"), "${X} ${X} ${Y}");
        assert_eq!(r.resolve("${X}", None, "F.Cu"), "${X}");
        assert_eq!(
            r.resolve("${FILENAME} ${}", None, "F.Cu"),
            "${FILENAME} ${}"
        );
        assert_eq!(
            r.resolve("${CURRENT_DATE}", None, "F.Cu"),
            "${CURRENT_DATE}"
        );
        let notes = r.notes();
        let messages: Vec<&str> = notes.iter().map(|n| n.message.as_str()).collect();
        assert_eq!(notes.len(), 4, "{messages:?}");
        assert!(messages[0].starts_with(
            "1 text(s) use the text variable ${CURRENT_DATE}, which KiCad fills in with the date"
        ));
        assert!(
            messages[1].starts_with("1 text(s) use the text variable ${FILENAME}, which neither")
        );
        assert!(messages[2].starts_with("2 text(s) use the text variable ${X}, which neither"));
        assert!(messages[3].starts_with("1 text(s) use the text variable ${Y}"));
    }

    #[test]
    fn a_project_file_gives_its_text_variables() {
        let vars = project_variables(
            br#"{"meta": {"version": 3}, "text_variables": {"B": "2", "A": "${B}"}}"#,
        )
        .unwrap();
        assert_eq!(
            vars,
            vec![("A".into(), "${B}".into()), ("B".into(), "2".into())]
        );
        assert_eq!(project_variables(b"{}").unwrap(), vec![]);
        assert!(project_variables(b"[1]").is_err());
        assert!(project_variables(b"(kicad_pcb").is_err());
    }
}
