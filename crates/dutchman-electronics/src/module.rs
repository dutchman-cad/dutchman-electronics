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

/// The `info --json` reply: who this module is and which formats it reads.
pub fn info() -> Value {
    let formats: Vec<Value> = crate::formats()
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
    extensions.sort_unstable();
    extensions.dedup();
    json!({
        "schema": INFO_SCHEMA,
        "ok": true,
        "name": NAME,
        "display_name": DISPLAY_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "interface": INTERFACE,
        "extensions": extensions,
        "formats": formats,
    })
}

/// The `detect <file> --json` reply for a file's first bytes and extension.
pub fn detect(head: &[u8], extension: Option<&str>) -> Value {
    let mut best: Option<(&'static str, Detection)> = None;
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
    }

    #[test]
    fn schema_major_equals_interface() {
        for schema in [INFO_SCHEMA, DETECT_SCHEMA] {
            let major = schema.rsplit('/').next().and_then(|m| m.parse::<u32>().ok());
            assert_eq!(major, Some(INTERFACE), "{schema}");
        }
    }

    #[cfg(feature = "altium")]
    #[test]
    fn info_lists_altium() {
        let v = info();
        assert_eq!(v["extensions"], json!(["pcbdoc"]));
        assert_eq!(v["formats"][0]["id"], "altium_pcbdoc");
        assert_eq!(v["formats"][0]["extension"], "pcbdoc");
        assert_eq!(v["formats"][0]["name"], "Altium PCB document (.PcbDoc)");
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

    #[test]
    fn errors_say_not_ok() {
        let e = error(INFO_SCHEMA, "boom");
        assert_eq!(e["ok"], false);
        assert_eq!(e["error"], "boom");
    }
}
