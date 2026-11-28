//! Dutchman Electronics: the list of board formats this build can read.
//!
//! The neutral board model and the [`InputFormat`] trait live in the `board-model` crate. Each
//! format is its own crate (Altium `.PcbDoc` is `altium-pcbdoc`, KiCad `.kicad_pcb` is `kicad-pcb`), switched on by a cargo feature.
//! This crate only gathers them, so it knows no format's details.

pub use board_model;

pub mod colors;
pub mod linked;
pub mod module;
pub mod overlay;
pub mod package;
pub mod standard;
use board_model::{Detection, InputFormat};

#[cfg(feature = "altium")]
pub use altium_pcbdoc;
#[cfg(feature = "kicad")]
pub use kicad_pcb;

/// The Altium format, when this build includes it.
fn altium() -> Option<Box<dyn InputFormat>> {
    #[cfg(feature = "altium")]
    return Some(Box::new(altium_pcbdoc::Altium));
    #[cfg(not(feature = "altium"))]
    return None;
}

/// The KiCad format, when this build includes it.
fn kicad() -> Option<Box<dyn InputFormat>> {
    #[cfg(feature = "kicad")]
    return Some(Box::new(kicad_pcb::KiCad));
    #[cfg(not(feature = "kicad"))]
    return None;
}

/// Every input format compiled into this build.
pub fn formats() -> Vec<Box<dyn InputFormat>> {
    [altium(), kicad()].into_iter().flatten().collect()
}

/// Picks the format for a file: the first that says [`Detection::Yes`], else the first that
/// says [`Detection::MaybeByName`].
pub fn detect(head: &[u8], extension: Option<&str>) -> Option<Box<dyn InputFormat>> {
    let mut by_name = None;
    for format in formats() {
        match format.detect(head, extension) {
            Detection::Yes => return Some(format),
            Detection::MaybeByName if by_name.is_none() => by_name = Some(format),
            _ => {}
        }
    }
    by_name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_not_a_board() {
        assert!(detect(b"hello", Some("txt")).is_none());
    }

    #[cfg(all(feature = "altium", feature = "kicad"))]
    #[test]
    fn both_formats_are_listed_and_picked_by_name() {
        let ids: Vec<&str> = formats().iter().map(|f| f.id()).collect();
        assert_eq!(ids, vec!["altium_pcbdoc", "kicad_pcb"]);
        let picked = detect(b"not ole", Some("PcbDoc")).map(|f| f.id());
        assert_eq!(picked, Some("altium_pcbdoc"));
        let picked = detect(b"anything", Some("kicad_pcb")).map(|f| f.id());
        assert_eq!(picked, Some("kicad_pcb"));
    }

    #[cfg(feature = "kicad")]
    #[test]
    fn kicad_is_picked_by_its_content_whatever_the_name() {
        let picked = detect(b"(kicad_pcb (version 20241229)", Some("txt")).map(|f| f.id());
        assert_eq!(picked, Some("kicad_pcb"));
        let picked = detect(b"(kicad_pcb (version 20241229)", None).map(|f| f.id());
        assert_eq!(picked, Some("kicad_pcb"));
    }
}

mod variant;
