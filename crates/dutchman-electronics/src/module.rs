//! The module interface Dutchman talks to (dutchman-cad/dutchman#1005): `info --json` and
//! `detect <file> --json`. Each reply is one JSON object with a `schema` string whose major
//! version is the interface version.

use board_model::Detection;
use serde_json::{json, Value};

/// The module's name: Dutchman runs it as `dutchman electronics ...`.
pub const NAME: &str = "electronics";
/// The name people see.
pub const DISPLAY_NAME: &str = "Dutchman Electronics";
/// The module interface version, equal to the major of every schema below.
pub const INTERFACE: u32 = 1;
/// Schema of the `info --json` reply.
pub const INFO_SCHEMA: &str = "dutchman.module-info/1";
/// Schema of the `detect <file> --json` reply.
pub const DETECT_SCHEMA: &str = "dutchman.module-detect/1";
/// The Altium project, listed beside the board formats: `read` opens the board it holds and
/// fills in the project's parameters. It is not an [`board_model::InputFormat`] of its own,
/// since a project is read together with its board file.
#[cfg(feature = "altium")]
const PROJECT: (&str, &str, &str) = ("altium_prjpcb", "Altium PCB project (.PrjPcb)", "prjpcb");

/// The `info --json` reply: who this module is and which formats it reads.
pub fn info() -> Value {
    #[allow(unused_mut)]
    let mut formats: Vec<Value> = crate::formats()
        .iter()
        .map(|f| {
            json!({
                "id": f.id(),
                "name": f.name(),
                "extension": f.extensions().first().copied().unwrap_or_default(),
                "extensions": f.extensions(),
            })
        })
        .collect();
    let mut extensions: Vec<&str> = crate::formats()
        .iter()
        .flat_map(|f| f.extensions().iter().copied())
        .collect();
    #[cfg(feature = "altium")]
    {
        let (id, name, extension) = PROJECT;
        formats.push(json!({
            "id": id,
            "name": name,
            "extension": extension,
            "extensions": [extension],
        }));
        extensions.push(extension);
    }
    extensions.sort_unstable();
    extensions.dedup();
    json!({
        "schema": INFO_SCHEMA,
        "ok": true,
        "name": NAME,
        "display_name": DISPLAY_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "repository": env!("CARGO_PKG_REPOSITORY"),
        "interface": INTERFACE,
        "extensions": extensions,
        "formats": formats,
    })
}

/// How sure a file is an Altium project: its name, and a first section `[Design]` (after any
/// white space, as the project reader accepts it).
#[cfg(feature = "altium")]
fn project(head: &[u8], extension: Option<&str>) -> Detection {
    let named = extension.is_some_and(|e| e.eq_ignore_ascii_case(PROJECT.2));
    let text = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    match (named, text.trim_ascii_start().starts_with(b"[Design]")) {
        (true, true) => Detection::Yes,
        (true, false) => Detection::MaybeByName,
        (false, _) => Detection::No,
    }
}

/// The `detect <file> --json` reply for a file's first bytes and extension.
pub fn detect(head: &[u8], extension: Option<&str>) -> Value {
    #[allow(unused_mut)]
    let mut best: Option<(&'static str, Detection)> = None;
    #[cfg(feature = "altium")]
    match project(head, extension) {
        Detection::No => {}
        found => best = Some((PROJECT.0, found)),
    }
    for format in crate::formats() {
        let detection = format.detect(head, extension);
        let better = match (best, detection) {
            (_, Detection::No) => false,
            (None, _) => true,
            (Some((_, Detection::MaybeByName)), Detection::Yes) => true,
            _ => false,
        };
        if better {
            best = Some((format.id(), detection));
        }
    }
    let (format, word) = match best {
        Some((id, Detection::Yes)) => (Value::from(id), "yes"),
        Some((id, Detection::MaybeByName)) => (Value::from(id), "maybe_by_name"),
        _ => (Value::Null, "no"),
    };
    json!({
        "schema": DETECT_SCHEMA,
        "ok": true,
        "detection": word,
        "format": format,
    })
}

/// A failure reply in the same shape, for when a command cannot run at all.
pub fn error(schema: &str, message: &str) -> Value {
    json!({ "schema": schema, "ok": false, "error": message })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_names_the_module_and_its_interface() {
        let v = info();
        assert_eq!(v["schema"], "dutchman.module-info/1");
        assert_eq!(v["ok"], true);
        assert_eq!(v["name"], "electronics");
        assert_eq!(v["display_name"], "Dutchman Electronics");
        assert_eq!(v["interface"], 1);
        assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(
            v["repository"],
            "https://github.com/dutchman-cad/dutchman-electronics"
        );
    }

    #[test]
    fn schema_major_equals_interface() {
        for schema in [INFO_SCHEMA, DETECT_SCHEMA] {
            let major = schema
                .rsplit('/')
                .next()
                .and_then(|m| m.parse::<u32>().ok());
            assert_eq!(major, Some(INTERFACE), "{schema}");
        }
    }

    #[cfg(all(feature = "altium", feature = "kicad"))]
    #[test]
    fn info_lists_altium_and_kicad() {
        let v = info();
        assert_eq!(v["extensions"], json!(["kicad_pcb", "pcbdoc", "prjpcb"]));
        assert_eq!(v["formats"][1]["id"], "kicad_pcb");
        assert_eq!(v["formats"][1]["extension"], "kicad_pcb");
        assert_eq!(v["formats"][1]["name"], "KiCad PCB (.kicad_pcb)");
        assert_eq!(v["formats"][0]["id"], "altium_pcbdoc");
        assert_eq!(v["formats"][0]["extension"], "pcbdoc");
        assert_eq!(v["formats"][0]["name"], "Altium PCB document (.PcbDoc)");
        assert_eq!(v["formats"][2]["id"], "altium_prjpcb");
        assert_eq!(v["formats"][2]["extension"], "prjpcb");
        assert_eq!(v["formats"][2]["name"], "Altium PCB project (.PrjPcb)");
    }

    #[cfg(feature = "altium")]
    #[test]
    fn detect_by_name_and_contents() {
        let ole = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        let yes = detect(&ole, Some("PcbDoc"));
        assert_eq!(yes["detection"], "yes");
        assert_eq!(yes["format"], "altium_pcbdoc");
        let maybe = detect(b"text", Some("pcbdoc"));
        assert_eq!(maybe["detection"], "maybe_by_name");
        let no = detect(b"text", Some("txt"));
        assert_eq!(no["detection"], "no");
        assert_eq!(no["format"], Value::Null);
    }

    #[cfg(feature = "kicad")]
    #[test]
    fn detect_kicad_by_contents_and_by_name() {
        let yes = detect(b"(kicad_pcb (version 20241229)", Some("kicad_pcb"));
        assert_eq!(yes["detection"], "yes");
        assert_eq!(yes["format"], "kicad_pcb");
        let by_content = detect(b"\n(kicad_pcb (version 20241229)", None);
        assert_eq!(by_content["detection"], "yes");
        let maybe = detect(b"text", Some("KiCad_PCB"));
        assert_eq!(maybe["detection"], "maybe_by_name");
        assert_eq!(maybe["format"], "kicad_pcb");
    }

    #[cfg(feature = "altium")]
    #[test]
    fn detect_a_project() {
        let yes = detect(b"\xEF\xBB\xBF[Design]\r\nVersion=1.0", Some("PrjPcb"));
        assert_eq!(yes["detection"], "yes");
        assert_eq!(yes["format"], "altium_prjpcb");
        let maybe = detect(b"something else", Some("prjpcb"));
        assert_eq!(maybe["detection"], "maybe_by_name");
        assert_eq!(detect(b"[Design]", Some("txt"))["detection"], "no");
        let indented = detect(b"\xEF\xBB\xBF\r\n  [Design]\r\n", Some("PrjPcb"));
        assert_eq!(indented["detection"], "yes");
    }

    #[test]
    fn errors_say_not_ok() {
        let e = error(INFO_SCHEMA, "boom");
        assert_eq!(e["ok"], false);
        assert_eq!(e["error"], "boom");
    }
}
