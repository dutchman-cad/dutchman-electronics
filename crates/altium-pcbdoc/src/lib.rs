//! Altium `.PcbDoc` reader: reads a board file into the neutral model in the `board-model` crate.
//! Roadmap steps 1 and 2 are done: open the container and list its streams; read the board
//! outline, its cutouts and the board thickness (`board.rs`). Step 3 has started: tracks, track
//! arcs, pads, vias, fills and copper regions (`copper.rs`). Step 5 has started: component placements
//! (`components.rs`) and their 3D models (`models.rs`). Step 4 has started: texts on overlay and copper layers (`texts.rs`) and their embedded fonts (`fonts.rs`). Everything else in the file is reported as a "not read yet"
//! note.
//!
//! A `.PcbDoc` is an OLE / Compound File Binary container: a small file system inside one file,
//! with folders ("storages") and files ("streams"). The board's data lives in streams such as
//! `Board6/Data` and `Tracks6/Data`. The `cfb` crate (MIT) opens the container; what is inside
//! each stream is for you to find out from your own test boards (see CONTRIBUTING.md).

use std::io::{Cursor, Read};

mod appearance;
mod board;
mod components;
mod copper;
mod fonts;
mod mask;
mod models;
mod planes;
pub mod privacy;
pub mod project;
mod shape_based;
pub mod special_strings;
mod texts;

/// Altium's checksum of a model file (also [`InputFormat::model_checksum`]).
pub use models::checksum as model_checksum;

use board_model::{
    Board, Detection, InputFormat, Note, ReadError, ReadOutcome, Severity, MAX_INPUT_BYTES,
};

/// Streams holding board objects that this version does not read yet. Each one that has data
/// gets a Dropped note, so nothing is lost silently.
const NOT_READ_YET: &[(&str, &str)] = &[("/Dimensions6/Data", "dimensions")];

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

    fn model_checksum(&self, file: &[u8]) -> Option<u32> {
        Some(models::checksum(file))
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
        let bodies6 = read_stream(&mut container, "/ComponentBodies6/Data")?;
        let models6 = read_stream(&mut container, "/Models/Data")?;
        let linked6 = read_stream(&mut container, "/ModelsNoEmbed/Data")?;

        let mut notes = Vec::new();
        // The shape-based streams repeat these two, with arcs where an outline has them (see
        // `shape_based.rs`). Only a region's arcs are read (a board cutout's); a body's are not.
        let mut region_arcs = Vec::new();
        for (copy, plain, data, kind) in [
            (
                "/ShapeBasedComponentBodies6/Data",
                "/ComponentBodies6/Data",
                &bodies6,
                0x0c,
            ),
            (
                "/ShapeBasedRegions6/Data",
                "/Regions6/Data",
                &regions6,
                0x0b,
            ),
        ] {
            let Some(shape) = read_stream(&mut container, copy)?.filter(|s| !s.is_empty()) else {
                continue;
            };
            match shape_based::compare(&shape, data.as_deref().unwrap_or_default(), kind) {
                None => notes.push(Note {
                    severity: Severity::Warning,
                    message: format!("{copy} does not repeat {plain}; only {plain} was read"),
                }),
                Some(records) if kind == 0x0b => region_arcs = records,
                Some(records) => {
                    let arcs = records.iter().filter(|r| r.is_some()).count();
                    if arcs > 0 {
                        notes.push(Note {
                            severity: Severity::Warning,
                            message: format!(
                                "{arcs} record(s) of {copy} have arcs, which are not read; only \
                                 the straight-edged {plain} was read"
                            ),
                        });
                    }
                }
            }
        }
        let record = board::board_record(&board6).map_err(ReadError::Invalid)?;
        let appearance = appearance::read(&record, &mut notes);
        let shape = board::read(&record, regions6.as_deref(), &region_arcs, &mut notes)
            .map_err(ReadError::Invalid)?;
        let mut components = match &components6 {
            Some(data) => components::read(
                data,
                texts6.as_deref(),
                wide_strings6.as_deref(),
                &mut notes,
            )
            .map_err(ReadError::Invalid)?,
            None => components::Components::default(),
        };
        let mut embedded = Vec::new();
        if let Some(data) = &bodies6 {
            let stored = models::attach(
                data,
                models6.as_deref(),
                linked6.as_deref(),
                &mut components,
                &mut notes,
            )
            .map_err(ReadError::Invalid)?;
            embedded = models::unpack(
                &mut components.placements,
                &stored,
                |path| read_stream(&mut container, path).map_err(|e| e.to_string()),
                &mut notes,
            );
        }
        let mut layers = copper::CopperLayers::new(&record);
        if let Some(data) = read_stream(&mut container, "/Nets6/Data")? {
            layers.nets = components::text_records(&data, "/Nets6/Data")
                .map_err(ReadError::Invalid)?
                .iter()
                .map(|r| r.get("NAME").unwrap_or_default().trim().to_string())
                .collect();
        }
        let mut copper = Vec::new();
        let mut graphics = Vec::new();
        let mut drills = Vec::new();
        if let Some(data) = &tracks6 {
            copper.extend(
                copper::tracks(data, &mut layers, &components, &mut graphics, &mut notes)
                    .map_err(ReadError::Invalid)?,
            );
        }
        if let Some(data) = &arcs6 {
            copper.extend(
                copper::arcs(data, &mut layers, &components, &mut graphics, &mut notes)
                    .map_err(ReadError::Invalid)?,
            );
        }
        if let Some(data) = &pads6 {
            copper.extend(
                copper::pads(data, &layers, &components, &mut drills, &mut notes)
                    .map_err(ReadError::Invalid)?,
            );
        }
        if let Some(data) = &vias6 {
            copper.extend(
                copper::vias(data, &layers, &mut drills, &mut notes).map_err(ReadError::Invalid)?,
            );
        }
        if let Some(data) = &fills6 {
            copper.extend(copper::fills(data, &layers, &mut notes).map_err(ReadError::Invalid)?);
        }
        copper.extend(copper::regions(&shape.other_regions, &layers, &mut notes));
        let rules6 = read_stream(&mut container, "/Rules6/Data")?;
        let solder_mask_expansion =
            mask::expansion(rules6.as_deref(), &mut notes).map_err(ReadError::Invalid)?;
        let polygons6 = read_stream(&mut container, "/Polygons6/Data")?;
        let mut pours = 0usize;
        if let Some(data) = polygons6.as_deref().filter(|d| !d.is_empty()) {
            let mut rule_notes = Vec::new();
            let rules =
                planes::rules(rules6.as_deref(), &mut rule_notes).map_err(ReadError::Invalid)?;
            let planes = planes::read(
                data,
                &layers,
                &drills,
                &shape.outline.cutouts,
                &rules,
                &mut notes,
            )
            .map_err(ReadError::Invalid)?;
            if !planes.copper.is_empty() {
                notes.extend(rule_notes);
            }
            copper.extend(planes.copper);
            pours = planes.others;
        }
        if pours > 0 {
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
            Some(data) => {
                let read = texts::read(data, &wide, &components, &mut layers, &mut notes)
                    .map_err(ReadError::Invalid)?;
                for (index, comment) in read.comments {
                    if let Some(p) = components.placements.get_mut(index) {
                        p.comment = comment;
                    }
                }
                read.texts
            }
            None => Vec::new(),
        };
        let fonts_header = read_stream(&mut container, "/EmbeddedFonts6/Header")?;
        let fonts6 = read_stream(&mut container, "/EmbeddedFonts6/Data")?;
        let fonts = fonts::read(
            fonts_header.as_deref(),
            fonts6.as_deref(),
            &texts,
            &mut notes,
        );
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
                graphics,
                texts,
                components: components.placements,
                appearance,
                solder_mask_expansion,
                ..Board::default()
            },
            notes,
            models: embedded,
            fonts,
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
