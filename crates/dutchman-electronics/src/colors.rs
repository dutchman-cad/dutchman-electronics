//! The colours the parts package gives its overlays and parts, in one place.
//!
//! The board model carries no colours yet, so these are module defaults, written into the
//! manifest as `"color": "#rrggbb"` (lowercase sRGB). When a board file can say what its mask or
//! silkscreen looks like, the file's colour replaces the matching constant here and nowhere else.
//! Component parts get no colour: their own STEP models carry theirs.

/// Solder mask: a dark green.
pub const MASK_COLOR: &str = "#1f5f3a";
/// How opaque the solder mask overlay is (0 clear, 1 solid), so the copper shows through it.
pub const MASK_OPACITY: f64 = 0.85;
/// Copper under the mask, as it looks through it: a green a little lighter and warmer than the mask.
pub const COPPER_MASKED_COLOR: &str = "#2b6e43";
/// Bare pad copper: gold, as on an ENIG finish.
pub const COPPER_BARE_COLOR: &str = "#d4af37";
/// Silkscreen ink: off-white.
pub const SILKSCREEN_COLOR: &str = "#f2f2f2";
/// The Board part, the FR4 substrate seen at the board edge: a pale tan.
pub const BOARD_COLOR: &str = "#c8b27a";
/// The Plating part, the copper inside plated holes: the same gold as the bare pads.
pub const PLATING_COLOR: &str = COPPER_BARE_COLOR;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colour_is_a_lowercase_hex_triplet() {
        for c in [
            MASK_COLOR,
            COPPER_MASKED_COLOR,
            COPPER_BARE_COLOR,
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
    }
}
