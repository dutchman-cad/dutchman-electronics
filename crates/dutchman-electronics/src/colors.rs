//! The colors the parts package gives its overlays and parts, in one place.
//!
//! The board file's own colors win: the solder mask and silkscreen colors of each side, the
//! surface finish of each side (which sets the color of the bare pads, and of the plating from
//! the top finish) and the color and opacity of an extruded component body. They are written
//! into the manifest as `"color": "#rrggbb"` (lowercase sRGB). Where the file says nothing, the
//! constants below are the fallback: a dark green mask, an off-white silkscreen, gold pads.
//! Components with an embedded STEP model get no color: the model carries its own.

use board_model::{Color, SurfaceFinish};

/// Solder mask: a dark green.
pub const MASK_COLOR: &str = "#1f5f3a";
/// The least opaque the solder mask overlay is (0 clear, 1 solid), and its opacity when the
/// board file gives none: 0.75.
///
/// The mask lies over all copper and the laminate sheet and is see-through, so copper shows
/// through it and laminate shows darker (a real mask over laminate is the darkest). A board file
/// may give its own opacity (0.6 for Altium's 3D view); the mask is drawn with the larger of that
/// and this. SolidCraft blends a see-through solid with what lies behind it linearly, so at 0.6
/// a pour shows 40 percent of its orange: a blue mask over copper comes out lilac-gray, and
/// bare laminate beside it reads dark. Altium's 3D view does not wash out that way, and at 0.75
/// the copper still shows as lines and pours while the mask color stays the mask color.
pub const MASK_OPACITY: f64 = 0.75;
/// The laminate sheet under the mask on each face: a dark FR4 olive, opaque. The mask over it
/// reads as the dark mask color; the board edge keeps [`BOARD_COLOR`].
pub const LAMINATE_COLOR: &str = "#2a2f1c";
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
/// (each 0 to 1) next to its color.
///
/// Everything is non-metallic on purpose: SolidCraft's modeling viewport has no environment to
/// reflect, so a metallic surface there renders flat dark gray. Gold and copper read right as a
/// colored, fairly smooth dielectric instead.
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
/// Extruded component bodies the file gives a color to: a moulded plastic.
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

/// A color as lowercase `#rrggbb`.
pub fn hex(c: &Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// What the bare pads of one side look like: color and finish.
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
            opacity: Some(1.0),
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
    fn the_default_mask_color_round_trips_through_hex() {
        assert_eq!(hex(&rgb(0x1f, 0x5f, 0x3a)), MASK_COLOR);
    }

    #[test]
    fn the_finish_process_picks_the_pad_look() {
        let cases: [(&[&str], &str, f64); 3] = [
            (
                &["ENIG", "ENEPIG", "Immersion Gold", "enig"],
                "#d4af37",
                0.45,
            ),
            (
                &[
                    "HASL",
                    "Lead-Free HASL",
                    "LF HASL",
                    "Immersion Tin",
                    "Immersion Silver",
                ],
                "#c9ccd1",
                0.4,
            ),
            (
                &[
                    "OSP",
                    "Bare",
                    "bare copper",
                    "Organic Solderability Preservative",
                ],
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
    fn every_color_is_a_lowercase_hex_triplet() {
        for c in [
            MASK_COLOR,
            LAMINATE_COLOR,
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
