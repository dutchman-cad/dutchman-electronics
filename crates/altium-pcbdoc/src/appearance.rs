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

use board_model::{Appearance, Color, Note, Severity};

use crate::board::Properties;

/// Reads the solder mask and silkscreen colours from the `/Board6/Data` record. A colour that is
/// missing stays `None`; one that cannot be read is `None` too, with a note.
pub(crate) fn read(board: &Properties, notes: &mut Vec<Note>) -> Appearance {
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
}
