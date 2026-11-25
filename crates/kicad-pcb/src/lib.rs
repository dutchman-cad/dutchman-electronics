//! KiCad `.kicad_pcb` reader: reads a board file into the neutral model in the `board-model`
//! crate.
//!
//! A `.kicad_pcb` is a text file of nested s-expressions. KiCad 9 and 10 files are read; an older
//! file gets a note and no board. Everything here is derived from KiCad's published file-format
//! documentation and from boards we made ourselves (see README.md and LEGAL.md): never from
//! KiCad's source code.
//!
//! What is read: the outline from `Edge.Cuts` (lines, arcs, circles, rectangles, polygons) with
//! its cutouts; the thickness from the stack-up; the layer list in stack order; footprints with
//! placement, side, rotation and 3D model references; tracks, arcs, vias, pads and filled zones;
//! silkscreen lines, arcs, circles, polygons and texts. Anything else becomes a note.

mod geom;
mod outline;
mod padshape;
mod reader;
pub mod sexpr;

use board_model::{Detection, InputFormat, ReadError, ReadOutcome, MAX_INPUT_BYTES};

/// The KiCad `.kicad_pcb` format.
#[derive(Clone, Copy, Debug, Default)]
pub struct KiCad;

/// True when the text starts (after white space) with the `(kicad_pcb` token.
fn starts_like_a_board(head: &[u8]) -> bool {
    let text = head.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(head);
    let start = text
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(text.len());
    let rest = text.get(start..).unwrap_or(&[]);
    let Some(after) = rest.strip_prefix(b"(kicad_pcb") else {
        return false;
    };
    // The token ends at white space, a parenthesis or the end of what was given.
    after
        .first()
        .is_none_or(|b| b.is_ascii_whitespace() || *b == b'(' || *b == b')')
}

impl InputFormat for KiCad {
    fn id(&self) -> &'static str {
        "kicad_pcb"
    }

    fn name(&self) -> &'static str {
        "KiCad PCB (.kicad_pcb)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["kicad_pcb"]
    }

    fn detect(&self, head: &[u8], extension: Option<&str>) -> Detection {
        let named = extension.is_some_and(|e| e.eq_ignore_ascii_case("kicad_pcb"));
        match (starts_like_a_board(head), named) {
            (true, _) => Detection::Yes,
            (false, true) => Detection::MaybeByName,
            (false, false) => Detection::No,
        }
    }

    fn read(&self, bytes: &[u8]) -> Result<ReadOutcome, ReadError> {
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(ReadError::TooLarge { bytes: bytes.len() });
        }
        let text = String::from_utf8_lossy(bytes);
        let root = sexpr::parse(&text).map_err(ReadError::Invalid)?;
        if root.head() != Some("kicad_pcb") {
            return Err(ReadError::Invalid(
                "the file does not start with (kicad_pcb".into(),
            ));
        }
        Ok(reader::read(root))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_by_content_and_by_name() {
        let k = KiCad;
        assert_eq!(k.detect(b"(kicad_pcb (version 1)", None), Detection::Yes);
        assert_eq!(k.detect(b"\n  (kicad_pcb\n", Some("txt")), Detection::Yes);
        assert_eq!(k.detect(b"(kicad_pcbx", Some("txt")), Detection::No);
        assert_eq!(
            k.detect(b"(kicad_sch", Some("KiCad_PCB")),
            Detection::MaybeByName
        );
        assert_eq!(k.detect(b"hello", Some("pcbdoc")), Detection::No);
        assert_eq!(k.detect(b"", None), Detection::No);
    }
}
