//! The colours the parts package gives its overlays and parts, in one place.
//!
//! The board file's own colours win: the solder mask and silkscreen colours of each side, the
//! surface finish of each side (which sets the colour of the bare pads, and of the plating from
//! the top finish) and the colour and opacity of an extruded component body. They are written
//! into the manifest as `"color": "#rrggbb"` (lowercase sRGB). Where the file says nothing, the
//! constants below are the fallback: a dark green mask, an off-white silkscreen, gold pads.
//! Components with an embedded STEP model get no colour: the model carries its own.

use board_model::{Color, SurfaceFinish};

/// Solder mask: a dark green.
pub const MASK_COLOR: &str = "#1f5f3a";
/// How opaque the solder mask overlay is (0 clear, 1 solid), so the copper shows through it.
///
/// This is ours on purpose, whatever the file says: the opacity in a board file (0.6 on the
/// blue-mask test board) is Altium's 3D-view display setting, not a property of the physical
/// mask, and this value is tuned for the SolidCraft viewport.
pub const MASK_OPACITY: f64 = 0.85;
/// Copper under the mask, as it looks through it: a green a little lighter and warmer than the mask.
pub const COPPER_MASKED_COLOR: &str = "#2b6e43";
/// Bare pad copper: gold, as on an ENIG finish.
pub const COPPER_BARE_COLOR: &str = "#d4af37";
/// Pads under a tin, solder or silver finish (HASL, immersion tin or silver): a light silver.
pub const SILVER_COLOR: &str = "#c9ccd1";
/// Pads under no finish (OSP, bare copper): copper.
pub const COPPER_COLOR: &str = "#b87333";
/// Silkscreen ink: off-white.
pub const SILKSCREEN_COLOR: &str = "#f2f2f2";
/// The Board part, the FR4 substrate seen at the board edge: a pale tan.
pub const BOARD_COLOR: &str = "#c8b27a";
/// The Plating part, the copper inside plated holes: the same gold as the bare pads.
pub const PLATING_COLOR: &str = COPPER_BARE_COLOR;

/// How a surface takes the light, written into the manifest as `"metallic"` and `"roughness"`
/// (each 0 to 1) next to its colour.
///
/// Everything is non-metallic on purpose: SolidCraft's modelling viewport has no environment to
/// reflect, so a metallic surface there renders flat dark grey. Gold and copper read right as a
/// coloured, fairly smooth dielectric instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finish {
    pub metallic: f64,
    pub roughness: f64,
}

/// Solder mask, and the copper seen through it: a satin gloss.
pub const MASK_FINISH: Finish = Finish {
    metallic: 0.0,
    roughness: 0.35,
};
/// Bare pads and plating: gold, a little less glossy than the mask.
pub const GOLD_FINISH: Finish = Finish {
    metallic: 0.0,
    roughness: 0.45,
};
/// Silver pads (HASL, immersion tin or silver): a little smoother than gold.
pub const SILVER_FINISH: Finish = Finish {
    metallic: 0.0,
    roughness: 0.4,
};
/// Bare copper pads (OSP, no finish).
pub const COPPER_FINISH: Finish = Finish {
    metallic: 0.0,
    roughness: 0.5,
};
/// Extruded component bodies the file gives a colour to: a moulded plastic.
pub const BODY_FINISH: Finish = Finish {
    metallic: 0.0,
    roughness: 0.6,
};
/// Silkscreen ink: matte.
pub const SILKSCREEN_FINISH: Finish = Finish {
    metallic: 0.0,
    roughness: 0.8,
};
/// The FR4 edge: matte.
pub const BOARD_FINISH: Finish = Finish {
    metallic: 0.0,
    roughness: 0.9,
};

/// A colour as lowercase `#rrggbb`.
pub fn hex(c: &Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// Per-channel gains that turn a dark or mid mask colour into the copper seen through it. Red
/// gains most and blue least, so the copper reads lighter and warmer than the mask. They are
/// chosen so the default mask `#1f5f3a` gives exactly [`COPPER_MASKED_COLOR`], `#2b6e43`.
const MASKED_COPPER_GAIN: [f64; 3] = [1.39, 1.158, 1.155];
/// The same for a light mask, which cannot get lighter: copper under it reads a little darker
/// and yellower.
const LIGHT_MASK_GAIN: [f64; 3] = [0.95, 0.90, 0.78];
/// Masks lighter than this (relative luminance, 0 to 1) use [`LIGHT_MASK_GAIN`].
const LIGHT_MASK_LUMINANCE: f64 = 0.6;
/// The copper always differs from its mask by at least this much (0 to 255) in some channel.
pub const MIN_COPPER_DIFFERENCE: u8 = 12;
/// What is added to (dark mask) or taken from (light mask) each channel when the gains alone
/// leave the copper too close to the mask, as for a black mask or a fully saturated one.
const COPPER_NUDGE: [u8; 3] = [16, 12, 12];

/// The colour of copper under a solder mask of colour `mask`, as lowercase `#rrggbb`.
///
/// A dark or mid mask (relative luminance 0.6 or less) is multiplied per channel by
/// [`MASKED_COPPER_GAIN`] (red 1.39, green 1.158, blue 1.155); a lighter one by
/// [`LIGHT_MASK_GAIN`] (0.95, 0.90, 0.78). When a channel would pass 255, all three are scaled
/// down together instead of clipped, so the hue stays. The default mask `#1f5f3a` gives exactly
/// `#2b6e43`. If the result is still within [`MIN_COPPER_DIFFERENCE`] of the mask in every
/// channel (black, or a mask whose gain is lost to the scaling), [`COPPER_NUDGE`] is added (dark)
/// or taken away (light), so the copper never vanishes into the mask.
pub fn masked_copper(mask: &Color) -> String {
    let rgb = [mask.r, mask.g, mask.b];
    let light = luminance(mask) > LIGHT_MASK_LUMINANCE;
    let gains = if light {
        LIGHT_MASK_GAIN
    } else {
        MASKED_COPPER_GAIN
    };
    let mut v = [0.0f64; 3];
    for ((out, c), gain) in v.iter_mut().zip(rgb).zip(gains) {
        *out = f64::from(c) * gain;
    }
    let top = v.iter().copied().fold(0.0, f64::max);
    if top > 255.0 {
        let scale = 255.0 / top;
        for out in &mut v {
            *out *= scale;
        }
    }
    // Each value is within 0..=255 after the clamp, so the cast cannot truncate.
    let mut out = v.map(|x| x.round().clamp(0.0, 255.0) as u8);
    let apart = out
        .iter()
        .zip(rgb)
        .any(|(o, m)| o.abs_diff(m) >= MIN_COPPER_DIFFERENCE);
    if !apart {
        for (o, nudge) in out.iter_mut().zip(COPPER_NUDGE) {
            *o = if light {
                o.saturating_sub(nudge)
            } else {
                o.saturating_add(nudge)
            };
        }
    }
    let [r, g, b] = out;
    hex(&Color {
        r,
        g,
        b,
        opacity: 1.0,
    })
}

/// Relative luminance of a colour, 0 (black) to 1 (white), from its sRGB values as they are.
fn luminance(c: &Color) -> f64 {
    (0.2126 * f64::from(c.r) + 0.7152 * f64::from(c.g) + 0.0722 * f64::from(c.b)) / 255.0
}

/// What the bare pads of one side look like: colour and finish.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PadLook {
    pub color: &'static str,
    pub finish: Finish,
}

const GOLD_LOOK: PadLook = PadLook {
    color: COPPER_BARE_COLOR,
    finish: GOLD_FINISH,
};

/// The pad look for a surface finish process, and whether the process was one we know. ENIG,
/// ENEPIG and immersion gold are gold; HASL (also lead-free), immersion tin and immersion silver
/// are silver; OSP and bare copper are copper. Anything else, and no finish at all, is gold; only
/// a named process we do not know is reported (`false`).
pub fn pad_look(finish: Option<&SurfaceFinish>) -> (PadLook, bool) {
    let Some(f) = finish else {
        return (GOLD_LOOK, true);
    };
    let key: String = f
        .process
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let has = |words: &[&str]| words.iter().any(|w| key.contains(w));
    if has(&["enig", "enepig", "immersiongold"]) {
        (GOLD_LOOK, true)
    } else if has(&["hasl", "immersiontin", "immersionsilver"]) {
        let silver = PadLook {
            color: SILVER_COLOR,
            finish: SILVER_FINISH,
        };
        (silver, true)
    } else if has(&["organicsolderabilitypreservative"])
        || key == "osp"
        || key == "bare"
        || key == "barecopper"
    {
        let copper = PadLook {
            color: COPPER_COLOR,
            finish: COPPER_FINISH,
        };
        (copper, true)
    } else {
        (GOLD_LOOK, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color {
            r,
            g,
            b,
            opacity: 1.0,
        }
    }

    fn finish(process: &str) -> SurfaceFinish {
        SurfaceFinish {
            process: process.into(),
            material: String::new(),
            color: None,
            thickness: None,
        }
    }

    #[test]
    fn the_default_mask_gives_the_default_masked_copper() {
        assert_eq!(hex(&rgb(0x1f, 0x5f, 0x3a)), MASK_COLOR);
        assert_eq!(masked_copper(&rgb(0x1f, 0x5f, 0x3a)), COPPER_MASKED_COLOR);
    }

    #[test]
    fn masked_copper_is_lighter_and_warmer_and_capped() {
        // Blue mask 17, 85, 204 gives 23.63, 98.43, 235.62.
        assert_eq!(masked_copper(&rgb(17, 85, 204)), "#1862ec");
        assert_eq!(masked_copper(&rgb(255, 255, 255)), "#ffffff");
        assert_eq!(masked_copper(&rgb(0, 0, 0)), "#000000");
    }

    #[test]
    fn masked_copper_never_vanishes_into_its_mask() {
        // (mask, copper): the mask, and what it gives.
        let cases = [
            ((255, 255, 255), "#f2e6c7"),
            ((0, 0, 0), "#100c0c"),
            ((200, 0, 0), "#ff0000"),
            ((36, 120, 50), "#328b3a"),
            ((17, 85, 204), "#1862ec"),
            ((0x1f, 0x5f, 0x3a), "#2b6e43"),
            ((255, 0, 0), "#ff0c0c"),
        ];
        for ((r, g, b), want) in cases {
            let mask = rgb(r, g, b);
            let copper = masked_copper(&mask);
            assert_eq!(copper, want, "mask {r},{g},{b}");
            let channel = |i: usize| u8::from_str_radix(&copper[1 + 2 * i..3 + 2 * i], 16).unwrap();
            let apart = [(channel(0), r), (channel(1), g), (channel(2), b)]
                .iter()
                .any(|(c, m)| c.abs_diff(*m) >= MIN_COPPER_DIFFERENCE);
            assert!(apart, "mask {r},{g},{b} gives {copper}");
        }
        // Red keeps its hue: green and blue stay zero.
        assert_eq!(&masked_copper(&rgb(200, 0, 0))[3..], "0000");
    }

    #[test]
    fn the_finish_process_picks_the_pad_look() {
        let cases: [(&[&str], &str, f64); 3] = [
            (&["ENIG", "ENEPIG", "Immersion Gold", "enig"], "#d4af37", 0.45),
            (
                &["HASL", "Lead-Free HASL", "LF HASL", "Immersion Tin", "Immersion Silver"],
                "#c9ccd1",
                0.4,
            ),
            (
                &["OSP", "Bare", "bare copper", "Organic Solderability Preservative"],
                "#b87333",
                0.5,
            ),
        ];
        for (processes, color, roughness) in cases {
            for p in processes {
                let (look, known) = pad_look(Some(&finish(p)));
                assert_eq!(look.color, color, "{p}");
                assert_eq!(look.finish.roughness, roughness, "{p}");
                assert_eq!(look.finish.metallic, 0.0, "{p}");
                assert!(known, "{p}");
            }
        }
        let (look, known) = pad_look(Some(&finish("Hard Gold Flash")));
        assert_eq!((look.color, known), ("#d4af37", false));
        assert_eq!(pad_look(None), (GOLD_LOOK, true));
    }

    #[test]
    fn every_colour_is_a_lowercase_hex_triplet() {
        for c in [
            MASK_COLOR,
            COPPER_MASKED_COLOR,
            COPPER_BARE_COLOR,
            SILVER_COLOR,
            COPPER_COLOR,
            SILKSCREEN_COLOR,
            BOARD_COLOR,
            PLATING_COLOR,
        ] {
            assert_eq!(c.len(), 7, "{c}");
            assert!(c.starts_with('#'), "{c}");
            assert!(
                c.chars()
                    .skip(1)
                    .all(|d| d.is_ascii_digit() || ('a'..='f').contains(&d)),
                "{c}"
            );
        }
        const { assert!(MASK_OPACITY > 0.0 && MASK_OPACITY <= 1.0) };
        for f in [
            MASK_FINISH,
            GOLD_FINISH,
            SILVER_FINISH,
            COPPER_FINISH,
            BODY_FINISH,
            SILKSCREEN_FINISH,
            BOARD_FINISH,
        ] {
            assert!((0.0..=1.0).contains(&f.metallic), "{f:?}");
            assert!((0.0..=1.0).contains(&f.roughness), "{f:?}");
        }
    }
}
