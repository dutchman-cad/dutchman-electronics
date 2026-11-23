//! Altium `.PcbDoc` reader: reads a board file into the neutral model in the `board-model` crate.
//! Step 1 of the roadmap is done: open the container and list its streams.
//!
//! A `.PcbDoc` is an OLE / Compound File Binary container: a small file system inside one file,
//! with folders ("storages") and files ("streams"). The board's data lives in streams such as
//! `Board6/Data` and `Tracks6/Data`. The `cfb` crate (MIT) opens the container; what is inside
//! each stream is for you to find out from your own test boards (see CONTRIBUTING.md).

use std::io::Cursor;

use board_model::{Detection, InputFormat, ReadError, ReadOutcome, MAX_INPUT_BYTES};

/// The first eight bytes of every OLE / Compound File Binary file.
pub const OLE_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// The most streams and folders a file may hold before it is refused as hostile.
const MAX_ENTRIES: usize = 100_000;

/// One stream inside the container.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamInfo {
    /// The path inside the container, e.g. "/Board6/Data".
    pub path: String,
    /// Its size in bytes.
    pub len: u64,
}

/// Lists every stream in an OLE container, sorted by path.
pub fn list_streams(bytes: &[u8]) -> Result<Vec<StreamInfo>, ReadError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(ReadError::TooLarge { bytes: bytes.len() });
    }
    let container = cfb::CompoundFile::open(Cursor::new(bytes))
        .map_err(|e| ReadError::Invalid(e.to_string()))?;
    let mut streams = Vec::new();
    for (seen, entry) in container.walk().enumerate() {
        if seen >= MAX_ENTRIES {
            return Err(ReadError::Invalid("too many entries".into()));
        }
        if entry.is_stream() {
            streams.push(StreamInfo {
                path: entry.path().to_string_lossy().into_owned(),
                len: entry.len(),
            });
        }
    }
    streams.sort();
    Ok(streams)
}

/// The Altium `.PcbDoc` format.
#[derive(Clone, Copy, Debug, Default)]
pub struct Altium;

impl InputFormat for Altium {
    fn id(&self) -> &'static str {
        "altium_pcbdoc"
    }

    fn name(&self) -> &'static str {
        "Altium PCB document (.PcbDoc)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["pcbdoc"]
    }

    fn detect(&self, head: &[u8], extension: Option<&str>) -> Detection {
        let named = extension.is_some_and(|e| e.eq_ignore_ascii_case("pcbdoc"));
        let ole = head.starts_with(&OLE_MAGIC);
        match (ole, named) {
            // Other programs use OLE too (old Office files), so the name decides in the end.
            (true, true) => Detection::Yes,
            (true, false) => Detection::No,
            (false, true) => Detection::MaybeByName,
            (false, false) => Detection::No,
        }
    }

    fn read(&self, bytes: &[u8]) -> Result<ReadOutcome, ReadError> {
        let streams = list_streams(bytes)?;
        // Roadmap step 1 stops here: the container opens. Steps 2 to 5 fill in the board and
        // return a `ReadOutcome` with `Note`s for anything left out.
        Err(ReadError::NotYet(format!(
            "reading the board from its {} streams",
            streams.len()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A tiny container with the same folder shape as a board: two folders, three streams.
    fn synthetic() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut cf = cfb::CompoundFile::create(Cursor::new(Vec::new()))?;
        cf.create_storage("/Board6")?;
        cf.create_storage("/Tracks6")?;
        cf.create_stream("/FileHeader")?.write_all(b"header")?;
        cf.create_stream("/Board6/Data")?
            .write_all(b"board bytes")?;
        cf.create_stream("/Tracks6/Data")?.write_all(&[0u8; 100])?;
        cf.flush()?;
        Ok(cf.into_inner().into_inner())
    }

    #[test]
    fn lists_the_streams_of_a_synthetic_container() -> Result<(), Box<dyn std::error::Error>> {
        let bytes = synthetic()?;
        let streams = list_streams(&bytes)?;
        let seen: Vec<(&str, u64)> = streams.iter().map(|s| (s.path.as_str(), s.len)).collect();
        assert_eq!(
            seen,
            vec![
                ("/Board6/Data", 11),
                ("/FileHeader", 6),
                ("/Tracks6/Data", 100)
            ]
        );
        Ok(())
    }

    #[test]
    fn detects_by_contents_and_name() -> Result<(), Box<dyn std::error::Error>> {
        let bytes = synthetic()?;
        assert_eq!(Altium.detect(&bytes, Some("PcbDoc")), Detection::Yes);
        assert_eq!(Altium.detect(&bytes, Some("doc")), Detection::No);
        assert_eq!(
            Altium.detect(b"text", Some("PcbDoc")),
            Detection::MaybeByName
        );
        Ok(())
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(list_streams(b"not an ole file at all").is_err());
        assert!(list_streams(&[]).is_err());
        assert!(Altium.read(&OLE_MAGIC).is_err());
    }
}
