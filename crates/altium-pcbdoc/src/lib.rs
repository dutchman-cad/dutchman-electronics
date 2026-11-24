//! Altium `.PcbDoc` reader: reads a board file into the neutral model in the `board-model` crate.
//! Roadmap steps 1 and 2 are done: open the container and list its streams; read the board
//! outline, its cutouts and the board thickness (`board.rs`). Step 3 has started: tracks, track
//! arcs, pads, vias, fills and copper regions (`copper.rs`). Step 5 has started: component placements
//! (`components.rs`). Step 4 has started: texts on overlay and copper layers (`texts.rs`). Everything else in the file is reported as a "not read yet"
//! note.
//!
//! A `.PcbDoc` is an OLE / Compound File Binary container: a small file system inside one file,
//! with folders ("storages") and files ("streams"). The board's data lives in streams such as
//! `Board6/Data` and `Tracks6/Data`. The `cfb` crate (MIT) opens the container; what is inside
//! each stream is for you to find out from your own test boards (see CONTRIBUTING.md).

use std::io::{Cursor, Read};

mod board;
mod components;
mod copper;
pub mod privacy;
mod texts;

use board_model::{
    Board, Detection, InputFormat, Note, ReadError, ReadOutcome, Severity, MAX_INPUT_BYTES,
};

/// Streams holding board objects that this version does not read yet. Each one that has data
/// gets a Dropped note, so nothing is lost silently.
const NOT_READ_YET: &[(&str, &str)] = &[
    ("/ComponentBodies6/Data", "component 3D bodies"),
    ("/Dimensions6/Data", "dimensions"),
    (
        "/ShapeBasedComponentBodies6/Data",
        "shape-based component bodies",
    ),
    ("/ShapeBasedRegions6/Data", "shape-based regions"),
];

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
                path: container_path(entry.path()),
                len: entry.len(),
            });
        }
    }
    streams.sort();
    Ok(streams)
}

/// Writes a path inside the container with `/` between the parts on every operating system.
/// (`Path::to_string_lossy` would give `\` on Windows, so `/Board6/Data` would not be found.)
pub(crate) fn container_path(path: &std::path::Path) -> String {
    let mut out = String::new();
    for part in path.components() {
        if let std::path::Component::Normal(name) = part {
            out.push('/');
            out.push_str(&name.to_string_lossy());
        }
    }
    if out.is_empty() {
        out.push('/');
    }
    out
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
        let mut container = cfb::CompoundFile::open(Cursor::new(bytes))
            .map_err(|e| ReadError::Invalid(e.to_string()))?;
        let board6 = read_stream(&mut container, "/Board6/Data")?
            .ok_or_else(|| ReadError::Invalid("no /Board6/Data stream".into()))?;
        let regions6 = read_stream(&mut container, "/Regions6/Data")?;
        let tracks6 = read_stream(&mut container, "/Tracks6/Data")?;
        let pads6 = read_stream(&mut container, "/Pads6/Data")?;
        let components6 = read_stream(&mut container, "/Components6/Data")?;
        let texts6 = read_stream(&mut container, "/Texts6/Data")?;
        let wide_strings6 = read_stream(&mut container, "/WideStrings6/Data")?;
        let arcs6 = read_stream(&mut container, "/Arcs6/Data")?;
        let vias6 = read_stream(&mut container, "/Vias6/Data")?;
        let fills6 = read_stream(&mut container, "/Fills6/Data")?;

        let mut notes = Vec::new();
        let record = board::board_record(&board6).map_err(ReadError::Invalid)?;
        let shape =
            board::read(&record, regions6.as_deref(), &mut notes).map_err(ReadError::Invalid)?;
        let components = match &components6 {
            Some(data) => components::read(
                data,
                texts6.as_deref(),
                wide_strings6.as_deref(),
                &mut notes,
            )
            .map_err(ReadError::Invalid)?,
            None => components::Components::default(),
        };
        let mut layers = copper::CopperLayers::new(&record);
        let mut copper = Vec::new();
        if let Some(data) = &tracks6 {
            copper.extend(copper::tracks(data, &layers, &mut notes).map_err(ReadError::Invalid)?);
        }
        if let Some(data) = &arcs6 {
            copper.extend(copper::arcs(data, &layers, &mut notes).map_err(ReadError::Invalid)?);
        }
        if let Some(data) = &pads6 {
            copper.extend(
                copper::pads(data, &layers, &components, &mut notes).map_err(ReadError::Invalid)?,
            );
        }
        if let Some(data) = &vias6 {
            copper.extend(copper::vias(data, &layers, &mut notes).map_err(ReadError::Invalid)?);
        }
        if let Some(data) = &fills6 {
            copper.extend(copper::fills(data, &layers, &mut notes).map_err(ReadError::Invalid)?);
        }
        copper.extend(copper::regions(&shape.other_regions, &layers, &mut notes));
        if streams
            .iter()
            .any(|s| s.path == "/Polygons6/Data" && s.len > 0)
        {
            notes.push(Note {
                severity: Severity::Info,
                message: "polygon pour settings (/Polygons6/Data) are not read; the copper each \
                          pour holds is read as regions"
                    .into(),
            });
        }
        let wide = match &wide_strings6 {
            Some(data) => components::wide_strings(data).map_err(ReadError::Invalid)?,
            None => Default::default(),
        };
        let texts = match &texts6 {
            Some(data) => texts::read(data, &wide, &components, &mut layers, &mut notes)
                .map_err(ReadError::Invalid)?,
            None => Vec::new(),
        };
        for (path, what) in NOT_READ_YET {
            if let Some(s) = streams.iter().find(|s| s.path == *path && s.len > 0) {
                notes.push(Note {
                    severity: Severity::Dropped,
                    message: format!("{what} ({} bytes in {path}) are not read yet", s.len),
                });
            }
        }
        Ok(ReadOutcome {
            board: Board {
                outline: shape.outline,
                thickness: shape.thickness,
                layers: layers.layers,
                copper,
                texts,
                components: components.placements,
                ..Board::default()
            },
            notes,
        })
    }
}

/// Reads one stream, or `None` when the file has no such stream. Bounded by the file size.
fn read_stream(
    container: &mut cfb::CompoundFile<Cursor<&[u8]>>,
    path: &str,
) -> Result<Option<Vec<u8>>, ReadError> {
    if !container.is_stream(path) {
        return Ok(None);
    }
    let mut data = Vec::new();
    container
        .open_stream(path)
        .and_then(|s| s.take(MAX_INPUT_BYTES as u64).read_to_end(&mut data))
        .map_err(|e| ReadError::Invalid(format!("{path}: {e}")))?;
    Ok(Some(data))
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

    /// Paths built with the platform's own separator (`\` on Windows) still come out with `/`.
    #[test]
    fn container_paths_use_slashes_on_every_platform() {
        let stream = std::path::PathBuf::from("/").join("Board6").join("Data");
        assert_eq!(container_path(&stream), "/Board6/Data");
        let top = std::path::PathBuf::from("/").join("FileHeader");
        assert_eq!(container_path(&top), "/FileHeader");
        assert_eq!(container_path(std::path::Path::new("/")), "/");
    }
}
