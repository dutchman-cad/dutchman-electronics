//! Solder mask and silkscreen colours.
//!
//! Worked out from our own CC0 test boards (see LEGAL.md):
//!
//! - `/Board6/Data` has a key `3DCONFIGURATION` whose value is a second list of settings,
//!   separated by backticks: `` `KEY=VALUE`KEY=VALUE… ``. These are the colours Altium's own 3D
//!   view draws the board in, saved with the board (View Configuration in 3D).
//! - `CFG3D.TOPSOLDERMASKCOLOR`, `CFG3D.BOTSOLDERMASKCOLOR`, `CFG3D.TOPSILKSCREENCOLOR` and
//!   `CFG3D.BOTSILKSCREENCOLOR` are each a number with red in the lowest byte, then green, then
//!   blue, and each has an `…OPACITY` key from 0 to 1.
//! - On `one-resistor` (Altium's defaults) both masks are 3307556 (red 36, green 120, blue 50)
//!   at opacity 0.6, and both silkscreens 16448250 (250, 250, 250) at opacity 1.
//! - On `one-blue-mask-resistor` both masks are 13391121 (17, 85, 204) and nothing else
//!   changed; on `one-yellow-silkscreen-resistor` both silkscreens are 65535 (255, 255, 0) and
//!   nothing else changed.
//!
//! Surface finish:
//!
//! - `V9_STACKCUSTOMDATA` in `/Board6/Data` is base64 of zlib-compressed XML, the layer stack as
//!   Altium's Layer Stack Manager keeps it: a `<Stack>` with its `<Layer>`s listed top to bottom,
//!   each with a `TypeId`, a `Name` and `<Property Name="…">value</Property>`s.
//! - A surface finish is a layer of its own there (Insert layer above, Surface Finish), with
//!   `TypeId` `b0827674-798c-4cf8-807c-8e6c2a11c145`. With the stack kept symmetric Altium adds one
//!   above the top copper and one below the bottom copper (copper is `TypeId`
//!   `f4eccd87-2cfb-4f37-be50-4f3a272b4d01`). Its properties are `Process` (an ID, a colon and the
//!   name), `Material`, `Material.Color` (`#AARRGGBB`) and `Thickness` (such as `0.004mm`).
//! - `one-enig-resistor`: ENIG, `Nickel, Gold`, `#FFFFC400`, 0.004 mm on both sides.
//!   `one-hasl-resistor`: HASL, `PbSn`, `#FFF2F2F2`, 0.02 mm on both sides. Every board before
//!   them has no finish layer.
//! - The finish layers are not among the `V9_STACK_LAYERn_…` keys, so the board's thickness
//!   (from those keys, see `board.rs`) leaves them out: 1.600 mm where Altium shows 1.608 (ENIG)
//!   or 1.640 (HASL).

use board_model::{Appearance, Color, Note, Severity, SurfaceFinish};

use crate::board::Properties;

/// The `TypeId` of a surface finish layer in the stack XML.
const FINISH_TYPE: &str = "b0827674-798c-4cf8-807c-8e6c2a11c145";
/// The `TypeId` of a copper (signal) layer in the stack XML.
const COPPER_TYPE: &str = "f4eccd87-2cfb-4f37-be50-4f3a272b4d01";
/// The most bytes the stack XML may unpack to before it is treated as hostile.
const MAX_STACK_XML: usize = 16 << 20;

/// Reads the solder mask and silkscreen colours and the surface finishes from the `/Board6/Data`
/// record. What is missing stays `None`; what cannot be read is `None` too, with a note.
pub(crate) fn read(board: &Properties, notes: &mut Vec<Note>) -> Appearance {
    let mut appearance = colors(board, notes);
    if let Some(data) = board.get("V9_STACKCUSTOMDATA") {
        match stack_xml(data) {
            Some(xml) => {
                let (top, bottom) = finishes(&xml);
                appearance.top_finish = top;
                appearance.bottom_finish = bottom;
            }
            None => notes.push(Note {
                severity: Severity::Warning,
                message: "the layer stack (V9_STACKCUSTOMDATA) could not be unpacked; no surface \
                          finish was read"
                    .into(),
            }),
        }
    }
    appearance
}

/// The stack XML: `data` decoded from base64 and unpacked, or `None`.
fn stack_xml(data: &str) -> Option<String> {
    let packed = base64(data.trim())?;
    let xml =
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&packed, MAX_STACK_XML).ok()?;
    String::from_utf8(xml).ok()
}

/// Standard base64 (`+`, `/`, `=` padding), or `None` for any other character.
fn base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The top and bottom surface finish of the stack XML: a finish layer before the first copper
/// layer is the top one, one after the last copper layer the bottom one.
fn finishes(xml: &str) -> (Option<SurfaceFinish>, Option<SurfaceFinish>) {
    let layers: Vec<(&str, &str)> = xml
        .split("<Layer ")
        .skip(1)
        .filter_map(|rest| {
            let type_id = attribute(rest, "TypeId")?;
            let end = rest.find("</Layer>").or_else(|| rest.find("/>"))?;
            Some((type_id, rest.get(..end)?))
        })
        .collect();
    let first_copper = layers.iter().position(|(t, _)| *t == COPPER_TYPE);
    let last_copper = layers.iter().rposition(|(t, _)| *t == COPPER_TYPE);
    let (mut top, mut bottom) = (None, None);
    for (i, (type_id, body)) in layers.iter().enumerate() {
        if *type_id != FINISH_TYPE {
            continue;
        }
        if first_copper.is_some_and(|c| i < c) {
            top = finish(body);
        } else if last_copper.is_some_and(|c| i > c) {
            bottom = finish(body);
        }
    }
    (top, bottom)
}

/// One finish layer's properties.
fn finish(body: &str) -> Option<SurfaceFinish> {
    let process = property(body, "Process")?;
    let process = process
        .rsplit(':')
        .next()
        .unwrap_or(&process)
        .trim()
        .to_string();
    Some(SurfaceFinish {
        process,
        material: property(body, "Material").unwrap_or_default(),
        color: property(body, "Material.Color").and_then(|c| argb(&c)),
        thickness: property(body, "Thickness").and_then(|t| length_mm(&t)),
    })
}

/// The value of `name="…"` in an XML start tag.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let start = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let rest = tag.get(start..)?;
    rest.get(..rest.find('"')?)
}

/// The text of `<Property Name="name" …>text</Property>`, with XML's five entities decoded.
fn property(body: &str, name: &str) -> Option<String> {
    let at = body.find(&format!("<Property Name=\"{name}\""))?;
    let rest = body.get(at..)?;
    let open = rest.find('>')? + 1;
    let close = rest.find("</Property>")?;
    let text = rest.get(open..close)?;
    Some(
        text.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&amp;", "&"),
    )
}

/// `#AARRGGBB` as a colour.
fn argb(text: &str) -> Option<Color> {
    let hex = text.trim().strip_prefix('#')?;
    if hex.len() != 8 {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    let byte = |shift: u32| ((v >> shift) & 0xff) as u8;
    Some(Color {
        r: byte(16),
        g: byte(8),
        b: byte(0),
        opacity: f64::from(byte(24)) / 255.0,
    })
}

/// A length such as `0.004mm` or `0.8mil` in millimetres.
fn length_mm(text: &str) -> Option<f64> {
    let text = text.trim();
    let (number, scale) = match text.strip_suffix("mm") {
        Some(n) => (n, 1.0),
        None => (text.strip_suffix("mil")?, 0.0254),
    };
    let mm = number.trim().parse::<f64>().ok()? * scale;
    (mm.is_finite() && (0.0..=100.0).contains(&mm)).then_some(mm)
}

/// The solder mask and silkscreen colours of `3DCONFIGURATION`.
fn colors(board: &Properties, notes: &mut Vec<Note>) -> Appearance {
    let Some(config) = board.get("3DCONFIGURATION") else {
        return Appearance::default();
    };
    let settings: Vec<(&str, &str)> = config
        .split('`')
        .filter_map(|pair| pair.split_once('='))
        .collect();
    let get = |key: &str| {
        settings
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.trim())
    };
    let mut unreadable = 0usize;
    let mut color = |name: &str| -> Option<Color> {
        let value = get(&format!("CFG3D.{name}COLOR"))?;
        let opacity = match get(&format!("CFG3D.{name}COLOROPACITY")) {
            None => Some(1.0),
            Some(v) => v.parse::<f64>().ok().filter(|o| (0.0..=1.0).contains(o)),
        };
        match (
            value.parse::<u32>().ok().filter(|&c| c <= 0xff_ffff),
            opacity,
        ) {
            (Some(c), Some(opacity)) => Some(Color {
                r: (c & 0xff) as u8,
                g: ((c >> 8) & 0xff) as u8,
                b: ((c >> 16) & 0xff) as u8,
                opacity,
            }),
            _ => {
                unreadable += 1;
                None
            }
        }
    };
    let appearance = Appearance {
        top_solder_mask: color("TOPSOLDERMASK"),
        bottom_solder_mask: color("BOTSOLDERMASK"),
        top_silkscreen: color("TOPSILKSCREEN"),
        bottom_silkscreen: color("BOTSILKSCREEN"),
        ..Appearance::default()
    };
    if unreadable > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{unreadable} solder mask or silkscreen colour(s) in 3DCONFIGURATION could not be \
                 read; left out"
            ),
        });
    }
    appearance
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(config: &str) -> Properties {
        Properties::parse(format!("|3DCONFIGURATION={config}|").as_bytes())
    }

    #[test]
    fn colours_are_red_green_blue_from_the_low_byte_up() {
        let mut notes = Vec::new();
        let a = read(
            &props(
                "`RECORD=Board`CFG3D.TOPSOLDERMASKCOLOR=13391121\
                 `CFG3D.TOPSOLDERMASKCOLOROPACITY=0.600000`CFG3D.TOPSILKSCREENCOLOR=65535",
            ),
            &mut notes,
        );
        let mask = a.top_solder_mask.unwrap();
        assert_eq!((mask.r, mask.g, mask.b, mask.opacity), (17, 85, 204, 0.6));
        let silk = a.top_silkscreen.unwrap();
        assert_eq!((silk.r, silk.g, silk.b, silk.opacity), (255, 255, 0, 1.0));
        assert_eq!((a.bottom_solder_mask, a.bottom_silkscreen), (None, None));
        assert!(notes.is_empty());
    }

    #[test]
    fn unreadable_colours_are_left_out_with_a_note() {
        let mut notes = Vec::new();
        let a = read(
            &props(
                "`CFG3D.TOPSOLDERMASKCOLOR=green`CFG3D.BOTSOLDERMASKCOLOR=16777216\
                 `CFG3D.TOPSILKSCREENCOLOR=1`CFG3D.TOPSILKSCREENCOLOROPACITY=2",
            ),
            &mut notes,
        );
        assert_eq!(a, Appearance::default());
        assert_eq!(notes.len(), 1);
        assert!(notes[0].message.starts_with("3 solder mask"));
        // No 3DCONFIGURATION at all is no colour and no note.
        let mut notes = Vec::new();
        let none = read(&Properties::parse(b"|KIND0=0|"), &mut notes);
        assert_eq!((none, notes.len()), (Appearance::default(), 0));
    }

    #[test]
    fn base64_decodes_and_refuses_other_characters() {
        assert_eq!(base64("TWFu").unwrap(), b"Man");
        assert_eq!(base64("TWE=").unwrap(), b"Ma");
        assert_eq!(base64("").unwrap(), b"");
        assert!(base64("TW*u").is_none());
    }

    #[test]
    fn finish_layers_above_and_below_the_copper() {
        let layer = |type_id: &str, process: &str| {
            format!(
                "<Layer Id=\"x\" TypeId=\"{type_id}\" Name=\"n\"><Properties>\
                 <Property Name=\"Process\" Type=\"EntityReference\">id:{process}</Property>\
                 <Property Name=\"Material\" Type=\"String\">Tin &amp; Lead</Property>\
                 <Property Name=\"Material.Color\" Type=\"C\">#80FF0010</Property>\
                 <Property Name=\"Thickness\" Type=\"LengthValue\">0.8mil</Property>\
                 </Properties></Layer>"
            )
        };
        let xml = [
            layer(FINISH_TYPE, "ENIG"),
            layer(COPPER_TYPE, "-"),
            layer(FINISH_TYPE, "between"),
            layer(COPPER_TYPE, "-"),
            layer(FINISH_TYPE, "HASL"),
        ]
        .concat();
        let (top, bottom) = finishes(&xml);
        let (top, bottom) = (top.unwrap(), bottom.unwrap());
        assert_eq!(
            (top.process.as_str(), bottom.process.as_str()),
            ("ENIG", "HASL")
        );
        assert_eq!(top.material, "Tin & Lead");
        let c = top.color.unwrap();
        assert_eq!((c.r, c.g, c.b), (255, 0, 16));
        assert!((c.opacity - 128.0 / 255.0).abs() < 1e-12);
        assert!((top.thickness.unwrap() - 0.02032).abs() < 1e-12);
        // No copper layer: a finish cannot be placed on a side.
        assert_eq!(finishes(&layer(FINISH_TYPE, "ENIG")), (None, None));
    }

    #[test]
    fn a_stack_that_cannot_be_unpacked_is_a_note() {
        let mut notes = Vec::new();
        let a = read(
            &Properties::parse(b"|V9_STACKCUSTOMDATA=eNrt*|"),
            &mut notes,
        );
        assert_eq!((a.top_finish, a.bottom_finish), (None, None));
        assert_eq!(notes.len(), 1);
    }
}
