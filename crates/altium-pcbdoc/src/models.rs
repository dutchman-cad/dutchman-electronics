//! Roadmap step 5, second part: each component's 3D model.
//!
//! Worked out from our own CC0 test boards `one-resistor` (R1 with an extruded box) and
//! `one-step-resistor` (R1 with an embedded STEP box of our own) and the numbers in their
//! README.txt (see LEGAL.md):
//!
//! - `/ComponentBodies6/Data` is a run of records: a type byte (`0x0c`), a `u32` length and a
//!   body. Body bytes 7..9 are the component index (see `components.rs`), 0 for R1 on both
//!   boards. From body byte 18 come a `u32` text length and the text (`|KEY=VALUE…`), then a
//!   `u32` vertex count and that many `f64` (x, y) pairs in 1/10000 mil, absolute: the body's
//!   outline (the box corners (24.2, 14.6) to (25.8, 15.4) on `one-resistor`).
//! - `MODEL.MODELTYPE=0` is an extruded body: its shape is that outline, from `STANDOFFHEIGHT`
//!   (0) up to `OVERALLHEIGHT` (17.7165 mil, 0.45 mm). `MODEL.MODELTYPE=1` is a STEP model:
//!   `MODELID` names an entry of `/Models/Data`, `MODEL.EMBED=TRUE` says it is in the file, and
//!   `MODEL.2D.X`/`MODEL.2D.Y` (R1's center on `one-step-resistor`), `MODEL.3D.ROTX/ROTY/ROTZ`
//!   (degrees) and `MODEL.3D.DZ` place the model's origin.
//! - `/Models/Data` is a run of records, each a `u32` length and text, with `ID`, `NAME`
//!   (`box-1608.step`) and `EMBED=TRUE`. The n-th record (from 0) is stored in the stream
//!   `/Models/n`: `/Models/0` is the STEP file compressed with zlib. `two-step-models` confirms
//!   the numbering with two models (record 0 `box-offset.step` in `/Models/0`, record 1
//!   `box-1608.step` in `/Models/1`); on `shared-step-model` two parts use one model, stored once,
//!   and both bodies give the same `MODELID`. The reader hands each used model over unpacked, once,
//!   in `ReadOutcome::models`; a part's `ModelSource::Embedded` names it by that ID.
//! - With the off-center STEP box `box-offset.step` (x 0 to 1.6 in the model), `MODEL.3D.ROTZ`
//!   follows the part: 0 on `one-offset-step-resistor`, 90 on `one-rotated-step-resistor` (R1 at
//!   90), 180 on `one-bottom-step-resistor` (R1 flipped: `LAYER=BOTTOM`, rotation 180, center
//!   moved to (25.4, 15), `BODYPROJECTION=1`). `MODEL.2D.X/Y` is R1's center on all three. So on
//!   the part itself the model is the same each time; the turn on the part is ROTZ minus the
//!   part's rotation, and the flip is the part's side.
//! - On `one-rotated-resistor` (R1 at 90 degrees) the extruded box's outline is stored turned:
//!   x 24.6 to 25.4, y 14.2 to 15.8; turned back by the part's rotation it is the same
//!   (-0.8, -0.4) to (0.8, 0.4) box as on `one-resistor`. On `one-bottom-resistor` the body has
//!   `BODYPROJECTION=1` (0 on the top-side boards).
//! - The lopsided extruded outline (0, -0.4), (1.6, -0.4), (1.6, 0), (0, 0.4) about R1's center
//!   (`one-offset-extruded-resistor`) shows how a flip stores it: on
//!   `one-bottom-extruded-resistor` R1 is at (25.4, 15), rotation 180, and the outline is stored
//!   as (23.8, 14.6), (23.8, 15), (25.4, 15.4), (25.4, 14.6). Turned back by 180 degrees that is
//!   the top-side outline with y negated: the part is turned over about its own X axis before
//!   it is rotated. So the reader negates y again for a bottom-side part, and the outline is the
//!   same on both boards. `one-bottom-rotated-extruded-resistor` (R1 flipped, then at 90) gives the
//!   same outline too.
//! - A STEP model turned 30 degrees on R1 with its origin at (-0.350, 0.606) from R1's center
//!   (`one-turned-step-resistor`, top, ROTZ 30) keeps that place on the part when R1 is flipped
//!   (`one-bottom-turned-step-resistor`, R1 at 180, ROTZ 210) and then turned to 90
//!   (`one-bottom-rotated-turned-step-resistor`, ROTZ 300): turned back by the part's rotation,
//!   the origin's y is negated, like an extruded outline's, and the model's turn is ROTZ plus the
//!   part's rotation on a bottom-side part (ROTZ minus it on a top-side part).
//! - `MODEL.3D.ROTX` is a right-handed tilt about the model's own X axis, applied before the turn
//!   about Z: `one-tilted-step-resistor` (ROTX 90) lies on its side, y 14.6 to 15.05 and 0.4
//!   above and below the board, and `one-turned-tilted-step-resistor` (ROTX 90, ROTZ 30) spans
//!   exactly what the tilt and then the turn give (x 24.975 to 26.586, y 14.654 to 15.843), not
//!   the other order. On the flipped `one-bottom-tilted-step-resistor` ROTX stays 90, and in the
//!   part's own frame that is the same tilt.
//! - `MODEL.3D.ROTY` is a right-handed tilt about the model's own Y axis, also before the turn:
//!   `one-y-tilted-step-resistor` (ROTY 90, origin (25, 15.4)) spans x 25 to 25.45 and reaches
//!   1.6 into the board (STANDOFFHEIGHT -1.6), and `one-turned-y-tilted-step-resistor` (ROTY 90,
//!   ROTZ 30) spans exactly what the tilt and then the turn give (x 24.6 to 25.390, y 15 to
//!   15.918). On the flipped `one-bottom-y-tilted-step-resistor` ROTY stays 90 and, in the part's
//!   own frame, offset (0, 0.4) and no turn give Altium's x 24.55 to 25; a tilt of -90 would put
//!   it at x 25 to 25.45. So a flip keeps both tilts, and the reader hands them over unchanged.
//! - `BODYCOLOR3D` and `BODYOPACITY3D` are a body's color, a Windows COLORREF in decimal (red in
//!   the low byte), and opacity. Every body on the earlier boards has 8421504, (128, 128, 128),
//!   and 1.000; `one-colored-body-resistor` (R1's extruded box set to (32, 128, 224) and 0.5)
//!   has 14712864 and 0.500. The reader hands the color over for extruded bodies only: a STEP
//!   model brings its own colors, and its body keeps the gray all the same.
//! - `MODEL.MODELTYPE=2` is a cylinder (`body-cylinder`, R1's box changed to Cylinder in the
//!   panel, radius 0.5, height 1, upright): `MODEL.CYLINDER.RADIUS` (19.685 mil) and
//!   `MODEL.CYLINDER.HEIGHT` (39.3701 mil), standing on `STANDOFFHEIGHT` (0) with its axis at
//!   `MODEL.2D.X`/`MODEL.2D.Y` (R1's center); `OVERALLHEIGHT` is standoff plus height. Its outline
//!   after the text is only its square box, (24.5, 14.5) to (25.5, 15.5). The reader writes it as
//!   an extruded circle, which is the same solid. One tilted by `MODEL.3D.ROTX` or `ROTY` is not
//!   read yet (no test board has one), and the body is left out with a note.
//! - `MODEL.MODELTYPE=3` is a sphere (`body-sphere`, `MODEL.SPHERE.RADIUS`, its center on the top
//!   face: `STANDOFFHEIGHT` minus and `OVERALLHEIGHT` plus the radius). The board model has no
//!   sphere, so it is left out with a note.
//! - `/ShapeBasedComponentBodies6/Data` repeats the bodies; see `shape_based.rs`. Where it has
//!   arcs, an extruded body's outline is read from there: on `body-arc-outline` R1's body is a
//!   rounded slot, box (24.2, 14.6) to (25.8, 15.4) with ends of radius 0.4 about (24.6, 15) and
//!   (25.4, 15), a polygon in `/ComponentBodies6/Data` and two lines and two half circles here.
//!   Its arcs are brought into the part's frame like its corners: turned back by the part's
//!   rotation, and on a bottom-side part mirrored (y and the angles negated, which reverses the
//!   arc, so its start and end swap).
//! - A linked model (the body's Source set to "Link to Model", `linked-model/` with
//!   `box-1608.step` next to the board) is not in the file: `/Models/Data` is empty and the body
//!   has `MODEL.EMBED=FALSE`, `MODEL.MODELSOURCE=File` (`Undefined` when embedded),
//!   `MODEL.NAME=box-1608.step` and a new `MODELID`. `/ModelsNoEmbed/Data` lists it like
//!   `/Models/Data` lists an embedded one (ID, NAME, CHECKSUM, the rotations), with no stream
//!   holding the file; it is empty on the boards with embedded models. No folder is kept anywhere
//!   in the file,
//!   although Altium's panel showed the full path while the board was open. `MODEL.CHECKSUM` is
//!   1259363632, the same as for the embedded copy of the same file: see [`checksum`]. So the
//!   reader hands over the bare name and the checksum, and whoever has the user's folders looks
//!   for the file.

use std::collections::{BTreeMap, BTreeSet};

use board_model::{
    Color, Contour, EmbeddedModel, ModelRef, ModelSource, Note, Placement, Point, Segment,
    Severity, Side, MAX_INPUT_BYTES,
};

use crate::appearance::colorref;
use crate::board::{contour, Arc, Properties, Vertex};
use crate::components::{text_records, Components, COMPONENT_AT, NO_COMPONENT};
use crate::copper::{read_u16, read_u32};

const BODY_RECORD: u8 = 0x0c;
/// Bytes at the start of a body record's body that come before its text length.
const BODY_SKIPPED: usize = 18;
/// Millimeters per internal unit (1/10000 mil).
const MM_PER_UNIT: f64 = 0.0254 / 10_000.0;
const MAX_RECORDS: usize = 1_000_000;
const MAX_VERTICES: usize = 100_000;
const MAX_LENGTH_MM: f64 = 25_000.0;
const MAX_ANGLE_DEG: f64 = 360.0;

/// An embedded model as `/Models/Data` lists it: the stream that holds it and its file name.
pub(crate) struct Stored {
    pub(crate) stream: String,
    pub(crate) name: String,
}

/// A linked model as `/ModelsNoEmbed/Data` lists it: its file name and checksum.
struct Linked {
    name: String,
    checksum: Option<u32>,
}

/// What a body whose model `/ModelsNoEmbed/Data` does not list falls back to.
static NOT_LISTED: Linked = Linked {
    name: String::new(),
    checksum: None,
};

/// Reads the component bodies and gives each placed component the model of its first body.
/// Returns the embedded models `/Models/Data` lists, by ID, for [`unpack`]. `linked6` is
/// `/ModelsNoEmbed/Data`, the list of linked models.
pub(crate) fn attach(
    bodies6: &[u8],
    arcs: &[Option<Vec<Vertex>>],
    models6: Option<&[u8]>,
    linked6: Option<&[u8]>,
    components: &mut Components,
    notes: &mut Vec<Note>,
) -> Result<BTreeMap<String, Stored>, String> {
    let models = match models6 {
        Some(data) => model_streams(data)?,
        None => BTreeMap::new(),
    };
    let linked = match linked6 {
        Some(data) => linked_models(data)?,
        None => BTreeMap::new(),
    };
    let mut extra = 0usize;
    let mut free = 0usize;
    let mut unread: BTreeMap<String, usize> = BTreeMap::new();
    let mut no_color = 0usize;
    for (n, body) in records(bodies6, notes)?.into_iter().enumerate() {
        let arcs = arcs.get(n).and_then(Option::as_ref);
        let Some(component) = read_u16(body, COMPONENT_AT).filter(|&c| c != NO_COMPONENT) else {
            free += 1;
            continue;
        };
        let Some(&Some(index)) = components.placement_of.get(usize::from(component)) else {
            *unread
                .entry("bodies of components that were not read".into())
                .or_default() += 1;
            continue;
        };
        let Some(placement) = components.placements.get_mut(index) else {
            continue;
        };
        if !placement.models.is_empty() {
            extra += 1;
            continue;
        }
        match model(body, arcs, placement, &models, &linked) {
            Ok((m, color_read)) => {
                if !color_read {
                    no_color += 1;
                }
                placement.models.push(m);
            }
            Err(why) => *unread.entry(why).or_default() += 1,
        }
    }
    if extra > 0 {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!(
                "{extra} more 3D bod(ies) of components that already have one not read"
            ),
        });
    }
    if free > 0 {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!("{free} 3D bod(ies) that belong to no component not read"),
        });
    }
    if no_color > 0 {
        notes.push(Note {
            severity: Severity::Warning,
            message: format!(
                "{no_color} extruded 3D bod(ies) have a color or opacity that could not be read;                  left without a color"
            ),
        });
    }
    for (why, n) in unread {
        notes.push(Note {
            severity: Severity::Dropped,
            message: format!("{n} component 3D bod(ies) not read: {why}"),
        });
    }
    Ok(models)
}

/// The stream and name of each embedded model, by its ID: the n-th record (from 0) is stored in
/// `/Models/n` (`two-step-models`).
fn model_streams(data: &[u8]) -> Result<BTreeMap<String, Stored>, String> {
    let mut map = BTreeMap::new();
    for (n, record) in text_records(data, "/Models/Data")?.iter().enumerate() {
        if let Some(id) = record.get("ID") {
            map.insert(
                id.trim().to_string(),
                Stored {
                    stream: format!("/Models/{n}"),
                    name: record.get("NAME").unwrap_or_default().trim().to_string(),
                },
            );
        }
    }
    Ok(map)
}

/// The name and checksum of each linked model, by its ID. `/ModelsNoEmbed/Data` has the same
/// records as `/Models/Data`, with `EMBED=FALSE` and `MODELSOURCE=File`, and no stream holds the
/// file (`linked-model`).
fn linked_models(data: &[u8]) -> Result<BTreeMap<String, Linked>, String> {
    let mut map = BTreeMap::new();
    for record in text_records(data, "/ModelsNoEmbed/Data")? {
        if let Some(id) = record.get("ID") {
            map.insert(
                id.trim().to_string(),
                Linked {
                    name: record.get("NAME").unwrap_or_default().trim().to_string(),
                    checksum: stored_checksum(record.get("CHECKSUM")),
                },
            );
        }
    }
    Ok(map)
}

/// Unpacks each embedded model the placed parts use, once however many parts use it. `read`
/// gives a stream's bytes (`None` when the file has no such stream). All models together may
/// unpack to at most [`MAX_INPUT_BYTES`]. A model that cannot be unpacked is left out with a
/// note naming the parts that use it, and those parts get no model.
pub(crate) fn unpack(
    placements: &mut [Placement],
    stored: &BTreeMap<String, Stored>,
    read: impl FnMut(&str) -> Result<Option<Vec<u8>>, String>,
    notes: &mut Vec<Note>,
) -> Vec<EmbeddedModel> {
    unpack_within(placements, stored, read, MAX_INPUT_BYTES, notes)
}

/// [`unpack`] with all models together allowed to unpack to at most `budget` bytes.
fn unpack_within(
    placements: &mut [Placement],
    stored: &BTreeMap<String, Stored>,
    mut read: impl FnMut(&str) -> Result<Option<Vec<u8>>, String>,
    mut budget: usize,
    notes: &mut Vec<Note>,
) -> Vec<EmbeddedModel> {
    let mut users: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in placements.iter() {
        for m in &p.models {
            if let ModelSource::Embedded { id } = &m.source {
                users
                    .entry(id.clone())
                    .or_default()
                    .push(p.designator.clone());
            }
        }
    }
    let limit = budget;
    let mut models = Vec::new();
    let mut failed: BTreeSet<String> = BTreeSet::new();
    for (id, designators) in &users {
        let Some(s) = stored.get(id) else {
            continue;
        };
        let step = read(&s.stream).and_then(|data| {
            let data = data.ok_or("the stream is missing")?;
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&data, budget).map_err(|e| {
                if e.status == miniz_oxide::inflate::TINFLStatus::HasMoreOutput {
                    format!("all models together unpack to more than {limit} bytes")
                } else {
                    format!("not zlib data ({:?})", e.status)
                }
            })
        });
        match step {
            Ok(step) => {
                budget = budget.saturating_sub(step.len());
                models.push(EmbeddedModel {
                    id: id.clone(),
                    name: s.name.clone(),
                    step,
                });
            }
            Err(why) => {
                notes.push(Note {
                    severity: Severity::Dropped,
                    message: format!(
                        "3D model {} ({}) could not be unpacked: {why}; {} left without a model",
                        s.name,
                        s.stream,
                        designators.join(", ")
                    ),
                });
                failed.insert(id.clone());
            }
        }
    }
    for p in placements.iter_mut() {
        p.models
            .retain(|m| !matches!(&m.source, ModelSource::Embedded { id } if failed.contains(id)));
    }
    models
}

/// The model of one body record and whether its color could be read (always true for a model
/// file, which has no color here), or why it was left out.
fn model(
    body: &[u8],
    arcs: Option<&Vec<Vertex>>,
    placement: &Placement,
    models: &BTreeMap<String, Stored>,
    linked: &BTreeMap<String, Linked>,
) -> Result<(ModelRef, bool), String> {
    let text_len = read_u32(body, BODY_SKIPPED).ok_or("record cut short")?;
    let text_start = BODY_SKIPPED + 4;
    let text_end = text_start
        .checked_add(text_len)
        .ok_or("record out of range")?;
    let text = body.get(text_start..text_end).ok_or("record cut short")?;
    let props = Properties::parse(text);
    let to_component = |p: Point| to_component(p, placement);
    match props.get("MODEL.MODELTYPE").map(str::trim) {
        Some("0") => {
            let points = outline(body, text_end)?;
            let polygon = closed(
                &points
                    .into_iter()
                    .map(|p| on_part(to_component(p), placement.side))
                    .collect::<Vec<_>>(),
            );
            // With arcs where the shape-based copy has them; should an arc not meet its
            // corners there, the polygon is kept instead.
            let outline = match arcs {
                Some(vertices) => {
                    let on_part: Vec<Vertex> = vertices
                        .iter()
                        .map(|v| vertex_on_part(v, placement))
                        .collect();
                    let mut problems = Vec::new();
                    let with_arcs = contour("an extruded 3D body", &on_part, &mut problems);
                    if problems.is_empty() {
                        with_arcs
                    } else {
                        polygon
                    }
                }
                None => polygon,
            };
            let standoff = props.length_mm("STANDOFFHEIGHT")?.unwrap_or(0.0);
            let height = props
                .length_mm("OVERALLHEIGHT")?
                .ok_or("no OVERALLHEIGHT")?;
            let color = body_color(&props);
            let color_read = color.is_some() || props.get("BODYCOLOR3D").is_none();
            let model = ModelRef {
                name: String::new(),
                source: ModelSource::Extruded {
                    outline,
                    standoff,
                    height,
                },
                offset: [0.0; 3],
                rotation_deg: [0.0; 3],
                color,
            };
            Ok((model, color_read))
        }
        Some("1") => {
            let model_id = props.get("MODELID").map(|id| id.trim().to_string());
            let listed = model_id
                .as_ref()
                .and_then(|id| linked.get(id))
                .unwrap_or(&NOT_LISTED);
            let mut name = props
                .get("MODEL.NAME")
                .unwrap_or_default()
                .trim()
                .to_string();
            let embedded = props.get("MODEL.EMBED").map(str::trim) == Some("TRUE");
            let id = model_id.filter(|id| models.contains_key(id));
            let source = match (embedded, id) {
                (true, Some(id)) => ModelSource::Embedded { id },
                (true, None) => return Err("embedded model not found in /Models/Data".into()),
                (false, _) => {
                    // The body names the file and its checksum; `/ModelsNoEmbed/Data` repeats
                    // both, and stands in where the body leaves one out.
                    if name.is_empty() {
                        name.clone_from(&listed.name);
                    }
                    ModelSource::External {
                        file: name.clone(),
                        checksum: stored_checksum(props.get("MODEL.CHECKSUM")).or(listed.checksum),
                    }
                }
            };
            let at = Point {
                x: props.length_mm("MODEL.2D.X")?.ok_or("no MODEL.2D.X")?,
                y: props.length_mm("MODEL.2D.Y")?.ok_or("no MODEL.2D.Y")?,
            };
            let local = on_part(to_component(at), placement.side);
            let dz = props.length_mm("MODEL.3D.DZ")?.unwrap_or(0.0);
            // MODEL.3D.ROTZ includes the part's own rotation, seen from the top: on a top-side
            // part it is the model's turn plus the part's (`one-rotated-step-resistor`,
            // `one-turned-step-resistor`), on a bottom-side part the model's turn minus the
            // part's (`one-bottom-turned-step-resistor`: 210 at 180, and
            // `one-bottom-rotated-turned-step-resistor`: 300 at 90, both a turn of 30 on the part).
            let rotz = angle(&props, "MODEL.3D.ROTZ")?;
            let turn = normalize(match placement.side {
                Side::Top => rotz - placement.rotation_deg,
                Side::Bottom => rotz + placement.rotation_deg,
            });
            let model = ModelRef {
                name,
                source,
                offset: [local.x, local.y, dz],
                rotation_deg: [
                    angle(&props, "MODEL.3D.ROTX")?,
                    angle(&props, "MODEL.3D.ROTY")?,
                    turn,
                ],
                color: None,
            };
            Ok((model, true))
        }
        Some("2") => {
            if angle(&props, "MODEL.3D.ROTX")? != 0.0 || angle(&props, "MODEL.3D.ROTY")? != 0.0 {
                return Err("a tilted cylinder not read yet".into());
            }
            let radius = props
                .length_mm("MODEL.CYLINDER.RADIUS")?
                .filter(|r| *r > 0.0)
                .ok_or("a cylinder with no radius")?;
            let tall = props
                .length_mm("MODEL.CYLINDER.HEIGHT")?
                .filter(|h| *h > 0.0)
                .ok_or("a cylinder with no height")?;
            let standoff = props.length_mm("STANDOFFHEIGHT")?.unwrap_or(0.0);
            let axis = Point {
                x: props.length_mm("MODEL.2D.X")?.ok_or("no MODEL.2D.X")?,
                y: props.length_mm("MODEL.2D.Y")?.ok_or("no MODEL.2D.Y")?,
            };
            let color = body_color(&props);
            let color_read = color.is_some() || props.get("BODYCOLOR3D").is_none();
            let model = ModelRef {
                name: String::new(),
                source: ModelSource::Extruded {
                    // A whole circle: equal start and end angles.
                    outline: Contour {
                        segments: vec![Segment::Arc {
                            center: on_part(to_component(axis), placement.side),
                            radius,
                            start_deg: 0.0,
                            end_deg: 0.0,
                            clockwise: false,
                        }],
                    },
                    standoff,
                    height: standoff + tall,
                },
                offset: [0.0; 3],
                rotation_deg: [0.0; 3],
                color,
            };
            Ok((model, color_read))
        }
        Some("3") => Err("a sphere has no place in the board model yet".into()),
        other => Err(format!("model type {} not read yet", other.unwrap_or("?"))),
    }
}

/// Altium's checksum of a model file, as `MODEL.CHECKSUM` and `/Models/Data`'s `CHECKSUM` give
/// it: every byte times its offset in the file, the first byte (offset 0) counted once, summed and
/// kept to 32 bits. The file's bytes as they are, line ends included.
///
/// From our own STEP boxes (see `linked-model/README.txt`): 1259363632 for `box-1608.step` and
/// 1259260868 for `box-offset.step` (6493 and 6477 bytes, LF line ends), and 1234716065 for a
/// 490672-byte copy of `box-1608.step` with CR LF line ends and a long comment, whose sum is
/// about 6.9e12: so the sum wraps at 32 bits, and a CR counts like any byte. Every STEP file starts
/// with `ISO-10303-21;`, so this does not tell a first byte counted once from a constant 73 ("I").
pub fn checksum(file: &[u8]) -> u32 {
    let mut sum = u32::from(file.first().copied().unwrap_or(0));
    let mut offset = 1u32;
    for &byte in file.iter().skip(1) {
        sum = sum.wrapping_add(u32::from(byte).wrapping_mul(offset));
        offset = offset.wrapping_add(1);
    }
    sum
}

/// A stored checksum (`MODEL.CHECKSUM`): a decimal 32-bit value, read as signed or unsigned.
fn stored_checksum(value: Option<&str>) -> Option<u32> {
    let v: i64 = value?.trim().parse().ok()?;
    u32::try_from(v)
        .ok()
        .or_else(|| i32::try_from(v).ok().map(|s| s as u32))
}

/// A body's color (`BODYCOLOR3D`, a COLORREF) and opacity (`BODYOPACITY3D`, 1 when missing).
fn body_color(props: &Properties) -> Option<Color> {
    let opacity = match props.get("BODYOPACITY3D") {
        None => Some(1.0),
        Some(v) => v.trim().parse::<f64>().ok(),
    };
    colorref(props.get("BODYCOLOR3D")?, opacity)
}

/// An angle in degrees brought into -180..=180 (a whole turn is the same turn; -0.0 becomes 0).
fn normalize(deg: f64) -> f64 {
    let mut a = deg % 360.0;
    if a > 180.0 {
        a -= 360.0;
    } else if a <= -180.0 {
        a += 360.0;
    }
    a + 0.0
}

/// A board point in the component's own coordinates: its origin at (0, 0), not rotated.
fn to_component(p: Point, placement: &Placement) -> Point {
    let (dx, dy) = (p.x - placement.at.x, p.y - placement.at.y);
    let (sin, cos) = (-placement.rotation_deg).to_radians().sin_cos();
    Point {
        x: dx * cos - dy * sin,
        y: dx * sin + dy * cos,
    }
}

/// A shape-based outline vertex (and its arc) in the part's own frame, as [`to_component`] and
/// [`on_part`] bring a corner there.
fn vertex_on_part(v: &Vertex, placement: &Placement) -> Vertex {
    let at = on_part(to_component(v.at, placement), placement.side);
    let arc = v.arc.map(|a| {
        let (start, end) = (
            a.start_deg - placement.rotation_deg,
            a.end_deg - placement.rotation_deg,
        );
        let (start_deg, end_deg) = match placement.side {
            Side::Top => (start, end),
            // Mirrored, the counter-clockwise arc from s to e runs from -e to -s.
            Side::Bottom => (-end, -start),
        };
        Arc {
            center: on_part(to_component(a.center, placement), placement.side),
            radius: a.radius,
            start_deg,
            end_deg,
        }
    });
    Vertex { at, arc }
}

/// A point in the component's coordinates as it lies on the part itself: a bottom-side part is
/// the part turned over about its own X axis, so a stored point is flipped back (y negated). For
/// an outline this keeps its turning direction, since Altium stores the flipped one reversed.
fn on_part(p: Point, side: Side) -> Point {
    match side {
        Side::Top => p,
        Side::Bottom => Point { x: p.x, y: -p.y },
    }
}

fn angle(props: &Properties, key: &str) -> Result<f64, String> {
    match props.get(key) {
        None => Ok(0.0),
        Some(text) => text
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|a| a.is_finite() && a.abs() <= MAX_ANGLE_DEG)
            .ok_or_else(|| format!("{key}={text} is not an angle")),
    }
}

/// The outline after the text: a `u32` count, then that many `f64` (x, y) pairs.
fn outline(body: &[u8], at: usize) -> Result<Vec<Point>, String> {
    let n = read_u32(body, at).ok_or("outline count cut short")?;
    if !(3..=MAX_VERTICES).contains(&n) {
        return Err(format!("an outline of {n} vertices"));
    }
    let mut points = Vec::with_capacity(n);
    for k in 0..n {
        let p = at + 4 + 16 * k;
        let coord = |at: usize| -> Result<f64, String> {
            let bytes: [u8; 8] = body
                .get(at..at + 8)
                .and_then(|b| b.try_into().ok())
                .ok_or("outline cut short")?;
            let mm = f64::from_le_bytes(bytes) * MM_PER_UNIT;
            if mm.is_finite() && mm.abs() <= MAX_LENGTH_MM {
                Ok(mm)
            } else {
                Err("outline coordinate out of range".into())
            }
        };
        points.push(Point {
            x: coord(p)?,
            y: coord(p + 8)?,
        });
    }
    Ok(points)
}

fn closed(points: &[Point]) -> Contour {
    let next = points.iter().skip(1).chain(points.first());
    Contour {
        segments: points
            .iter()
            .zip(next)
            .map(|(from, to)| Segment::Line {
                from: *from,
                to: *to,
            })
            .collect(),
    }
}

/// Splits `/ComponentBodies6/Data` into record bodies.
fn records<'a>(data: &'a [u8], notes: &mut Vec<Note>) -> Result<Vec<&'a [u8]>, String> {
    let mut bodies = Vec::new();
    let mut at = 0usize;
    while at < data.len() {
        if bodies.len() >= MAX_RECORDS {
            return Err(format!(
                "/ComponentBodies6/Data has more than {MAX_RECORDS} records"
            ));
        }
        let kind = data.get(at).copied().ok_or("body record cut short")?;
        if kind != BODY_RECORD {
            notes.push(Note {
                severity: Severity::Dropped,
                message: format!(
                    "/ComponentBodies6/Data: unknown record type {kind:#04x} at byte {at}; the \
                     rest of the stream was not read"
                ),
            });
            break;
        }
        let len = read_u32(data, at + 1).ok_or("/ComponentBodies6/Data: length cut short")?;
        let start = at + 5;
        let end = start
            .checked_add(len)
            .filter(|&e| e <= data.len())
            .ok_or("/ComponentBodies6/Data: record is longer than the stream")?;
        bodies.push(data.get(start..end).ok_or("record out of range")?);
        at = end;
    }
    Ok(bodies)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `tests/boards/linked-model/box-1608.step`, the box Altium gave 1259363632.
    fn box_1608() -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/boards/linked-model/box-1608.step");
        std::fs::read(path).unwrap_or_default()
    }

    #[test]
    fn checksum_is_altiums_for_our_box() {
        let file = box_1608();
        assert_eq!(file.len(), 6493);
        assert_eq!(checksum(&file), 1_259_363_632);
    }

    /// The large copy linked on the probe board (see `linked-model/README.txt`): `box-1608.step`
    /// with CR LF line ends and 4000 comment lines after `HEADER;`. Altium gave 1234716065.
    #[test]
    fn checksum_wraps_at_32_bits_and_counts_crs() {
        let text = String::from_utf8(box_1608()).unwrap_or_default();
        let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
        let header = lines.iter().position(|l| l == "HEADER;").unwrap_or(0);
        let pad = (0..4000).map(|n| {
            format!(
                "/* padding so the file is large: {}*/",
                format!("{n:06} ").repeat(12)
            )
        });
        lines.splice(header + 1..header + 1, pad);
        let big = lines.join("\r\n").into_bytes();
        assert_eq!(big.len(), 490_672);
        assert_eq!(checksum(&big), 1_234_716_065);
    }

    #[test]
    fn checksum_of_tiny_files() {
        assert_eq!(checksum(b""), 0);
        assert_eq!(checksum(b"I"), 73);
        assert_eq!(checksum(b"IS"), 73 + 83);
        assert_eq!(checksum(b"ISO"), 73 + 83 + 2 * 79);
    }

    #[test]
    fn the_linked_model_list_gives_name_and_checksum() -> Result<(), String> {
        // `/ModelsNoEmbed/Data` of `one-linked-step-resistor`, as Altium wrote it.
        let text = b"EMBED=FALSE|MODELSOURCE=File|ID={A80E16CD-AB39-46A1-AD50-5C02186E37B5}|\
                     ROTX=0.000|ROTY=0.000|ROTZ=0.000|DZ=0|CHECKSUM=1259363632|\
                     NAME=box-1608.step\0";
        let mut data = (text.len() as u32).to_le_bytes().to_vec();
        data.extend_from_slice(text);
        let list = linked_models(&data)?;
        let entry = list
            .get("{A80E16CD-AB39-46A1-AD50-5C02186E37B5}")
            .ok_or("not listed")?;
        assert_eq!(entry.name, "box-1608.step");
        assert_eq!(entry.checksum, Some(1_259_363_632));
        assert!(linked_models(&[]).map_err(|e| e.to_string())?.is_empty());
        assert!(linked_models(&[0xff, 0xff, 0xff, 0x7f]).is_err());
        Ok(())
    }

    #[test]
    fn stored_checksums_read_signed_or_unsigned() {
        assert_eq!(stored_checksum(Some("1259363632")), Some(1_259_363_632));
        assert_eq!(stored_checksum(Some(" 4294967295 ")), Some(u32::MAX));
        assert_eq!(stored_checksum(Some("-1")), Some(u32::MAX));
        assert_eq!(stored_checksum(Some("-2147483648")), Some(0x8000_0000));
        assert_eq!(stored_checksum(Some("4294967296")), None);
        assert_eq!(stored_checksum(Some("-2147483649")), None);
        assert_eq!(stored_checksum(Some("x")), None);
        assert_eq!(stored_checksum(None), None);
    }

    #[test]
    fn hostile_streams_are_errors_or_notes_not_panics() {
        let mut components = Components::default();
        let mut notes = Vec::new();
        assert!(attach(&[BODY_RECORD], &[], None, None, &mut components, &mut notes).is_err());
        assert!(attach(
            &[BODY_RECORD, 0xff, 0xff, 0xff, 0x7f],
            &[],
            None,
            Some(&[0xff, 0xff, 0xff, 0x7f]),
            &mut components,
            &mut notes
        )
        .is_err());
        // A body of a component that does not exist is a note.
        let mut body = vec![0u8; 30];
        body[7] = 3;
        let mut data = vec![BODY_RECORD];
        data.extend_from_slice(&(body.len() as u32).to_le_bytes());
        data.extend_from_slice(&body);
        assert!(attach(&data, &[], None, None, &mut components, &mut notes).is_ok());
        assert!(notes.iter().any(|n| n.severity == Severity::Dropped));
        assert!(attach(&[0x42], &[], None, None, &mut components, &mut notes).is_ok());
    }

    fn part(designator: &str, id: &str) -> Placement {
        Placement {
            designator: designator.into(),
            footprint: String::new(),
            at: Point::default(),
            rotation_deg: 0.0,
            side: Side::Top,
            models: vec![ModelRef {
                name: String::new(),
                source: ModelSource::Embedded { id: id.into() },
                offset: [0.0; 3],
                rotation_deg: [0.0; 3],
                color: None,
            }],
            comment: String::new(),
        }
    }

    fn stored() -> BTreeMap<String, Stored> {
        ["good", "bad", "gone"]
            .iter()
            .enumerate()
            .map(|(n, id)| {
                (
                    id.to_string(),
                    Stored {
                        stream: format!("/Models/{n}"),
                        name: format!("{id}.step"),
                    },
                )
            })
            .collect()
    }

    fn streams(path: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(match path {
            "/Models/0" => Some(miniz_oxide::deflate::compress_to_vec_zlib(
                b"ISO-10303-21;",
                6,
            )),
            "/Models/1" => Some(b"not zlib".to_vec()),
            _ => None,
        })
    }

    #[test]
    fn each_used_model_is_unpacked_once_and_a_broken_one_is_a_note() {
        let mut parts = vec![
            part("R1", "good"),
            part("R2", "good"),
            part("U1", "bad"),
            part("U2", "gone"),
        ];
        let mut notes = Vec::new();
        let models = unpack(&mut parts, &stored(), streams, &mut notes);
        assert_eq!(models.len(), 1, "{models:?}");
        assert_eq!(
            (models[0].id.as_str(), models[0].name.as_str()),
            ("good", "good.step")
        );
        assert_eq!(models[0].step, b"ISO-10303-21;");
        let have_model: Vec<bool> = parts.iter().map(|p| !p.models.is_empty()).collect();
        assert_eq!(have_model, [true, true, false, false]);
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(notes.iter().all(|n| n.severity == Severity::Dropped));
        assert!(notes[0].message.contains("bad.step") && notes[0].message.contains("U1"));
        assert!(notes[1].message.contains("missing") && notes[1].message.contains("U2"));
    }

    #[test]
    fn unpacking_stops_at_the_size_cap() {
        let mut parts = vec![part("R1", "good")];
        let mut notes = Vec::new();
        let models = unpack_within(&mut parts, &stored(), streams, 5, &mut notes);
        assert!(models.is_empty());
        assert!(parts[0].models.is_empty());
        assert!(notes[0].message.contains("more than 5 bytes"), "{notes:?}");
    }

    #[test]
    fn a_body_color_is_read_with_its_opacity() {
        let color = |text: &str| body_color(&Properties::parse(text.as_bytes()));
        let blue = |opacity| {
            Some(Color {
                r: 32,
                g: 128,
                b: 224,
                opacity,
            })
        };
        assert_eq!(
            color("|BODYCOLOR3D=14712864|BODYOPACITY3D=0.500|"),
            blue(0.5)
        );
        assert_eq!(color("|BODYCOLOR3D=14712864|"), blue(1.0));
        assert_eq!(color("|BODYOPACITY3D=0.500|"), None);
        assert_eq!(color("|BODYCOLOR3D=red|BODYOPACITY3D=1|"), None);
        assert_eq!(color("|BODYCOLOR3D=16777216|BODYOPACITY3D=1|"), None);
        assert_eq!(color("|BODYCOLOR3D=0|BODYOPACITY3D=1.5|"), None);
    }
}
