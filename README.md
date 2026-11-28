# Dutchman Electronics

Reads circuit boards into [SolidCraft](https://github.com/tangency-labs/solidcraft) designs, through [Dutchman](https://github.com/dutchman-cad/dutchman).

## Formats

| Format | Status |
|---|---|
| Altium (`.PcbDoc`) | In progress |
| Altium project (`.PrjPcb`) | In progress: reads the project's board and fills in the texts that refer to project parameters |
| tscircuit | In progress |
| KiCad (`.kicad_pcb`, KiCad 9 and 10) | In progress: reader in `crates/kicad-pcb` (scope: issue 75); more scope depends on the SolidCraft maintainers' answer on [tangency-labs/solidcraft#21320](https://github.com/tangency-labs/solidcraft/issues/21320) |
| Fusion Electronics | TBD |

It is an optional add-on. Dutchman works without it, and installing Dutchman does not install it. It is not part of cadmpeg: cadmpeg reads mechanical CAD, this reads electronics.

## The goal

Open an Altium board in SolidCraft and see what SolidCraft already shows for a KiCad board:

| Part of the board | How it arrives |
|---|---|
| The board | An exact solid, with its cutouts and drilled holes, in the color of its FR4 edge |
| Plated holes | The copper lining every via and every plated pad hole, as thin tubes (`Plating`) |
| Laminate | An opaque dark FR4 sheet on each face (overlay `laminate`, 0.005 mm thick), the color the mask shows over bare laminate; the board edge keeps its own FR4 color |
| Copper traces, pads, vias, pours | All copper of a face as one thin solid (overlay, 0.035 mm thick) in copper color, seen through the mask; the surface finish (gold, silver) covers it only where the mask is open |
| Solder mask | A conformal coat 0.010 mm thick in the board's mask color over the whole face and all its copper, see-through (it lies on the laminate's top where there is bare laminate, z = face + 0.005, and over copper (and a 0.010 skirt round it) from the laminate's top up to 0.010 above the copper's top (the coat drapes the copper's edge, so no bare copper wall shows), as two overlays of one color and opacity, `Solder mask top` and `Solder mask top over copper`, which never overlap; copper shows as relief even under an opaque mask; with no laminate overlay the coat over bare laminate lies on the face; opacity from the board file where it gives one, but at least 0.75, which is also the default), open at the pads (each opening is the pad grown by the mask expansion: the pad's own value for that side where the file gives one (KiCad's `solder_mask_margin` on the pad, else on its footprint, on both sides; for Altium one measured from the hole's edge is turned into one from the pad's edge, by the shorter side for a pad that is not round, with an Info note when the opening ends up smaller than the pad), else the board file's own value, for Altium its board-wide solder mask expansion rule, otherwise 0.1 mm, so a thin ring of bare laminate shows round it; a negative value shrinks the opening, never below the drill of a drilled pad and never below zero size, where a pad shrunk away has no opening; a negative value on a polygon pad (trapezoid, chamfered or custom shape) is not applied and an Info note counts it; a pad side the file says is tented gets none, its copper stays under the mask with no finish; a via side the file says is not tented gets one too, its ring grown by the via's own mask expansion or the board's, with the drill open on that side; tented vias get none) and the drills |
| Silkscreen | Text and lines as outlines, in white (TrueType text with its real letter outlines, followed to a four-hundredth of its height; Altium stroke text in the Hershey font nearest the one it names: Sans Serif in Roman Simplex, Serif in Times Roman, and Default in Roman Simplex letters on Default's own fixed character advance; the spacing of all three is fitted to the line widths of our own `stroke-fonts` test board (per kind of character for Sans Serif and Serif: capitals, lower case, and digits with punctuation, which Altium sets tighter than Hershey does), so a line's width and placement match Altium's to within about 3% at the heights we measured, but the letter shapes are Hershey's, not Altium's), clipped away from the mask openings (pads and their ring of laminate) and the drills, as a fab does; it lies on the mask coat under it, 0.015 mm thick from face + 0.015 over bare laminate and from face + 0.045 over copper (two overlays, `Silkscreen top` and `Silkscreen top over copper`) |
| Components | Each one placed on the board with its 3D model (STEP) |

It reads `.PcbDoc` files on Windows, macOS and Linux. Altium itself is only needed to make test boards. Pull requests are tested on Linux; every change on `main` is also tested on Windows and macOS.

## How it plugs into Dutchman

Dutchman finds add-ons by name. Any program called `dutchman-<something>` that sits next to `dutchman` or on your PATH is found by Dutchman's launcher (`crates/dutchman-launcher` in the Dutchman repository). This module builds one such program, `dutchman-electronics`. If it is not installed, Dutchman says so and links here; nothing else changes.

```
board.PcbDoc --reader--> neutral board model --package writer--> parts package --Dutchman--> board.sc
```

The parts package (`dutchman.parts/1`) is what Dutchman reads back from `dutchman-electronics read <file> --out <dir> --json`: one STEP file per distinct 3D model (the board itself included), a list of parts named by designator with their placement, copper, silkscreen and solder mask as thin overlays, and the copper lining plated holes. The contract is in dutchman-cad/dutchman#1005; a summary is in issue #1.

The "neutral board model" is plain Rust types in `crates/board-model`: outline, layers, copper, text, component placements and model references. It does not know Altium and it does not know SolidCraft. That is the one boundary to keep clean.

| Crate | What it is |
|---|---|
| `crates/board-model` | The neutral board model and the `InputFormat` trait |
| `crates/board-step` | Writes the board substrate (outline, cutouts, drilled holes, thickness) as an exact STEP solid, and the tubes of plated holes |
| `crates/altium-pcbdoc` | Reads `.PcbDoc` into the board model. No Dutchman dependencies |
| `crates/kicad-pcb` | Reads KiCad `.kicad_pcb` into the board model. No Dutchman dependencies |
| `crates/dutchman-electronics` | The `dutchman-electronics` command; gathers the formats |

Each further format is one new crate that fills the same model.

### Filled graphics, pad sides, and mask openings

We carry filled graphics as `Graphic::Area { layer, contour, holes, owner }`, in board
coordinates. We draw them on copper and silkscreen wherever we draw lines and text.
We also draw graphic lines and arcs on copper layers as copper under the mask.
Silkscreen areas keep their holes and are clipped by mask openings and drills, then split
between the coat over copper and the coat over laminate, at the same heights as silk lines.
Other layer kinds remain reported as dropped until we draw those layers.

We keep a pad's existing shape and dimensions as its top shape. `CopperItem::Pad.bottom`
is an optional `(PadShape, size_x, size_y)` override using the same center and rotation.
`CopperItem::PolygonPad.bottom` is an optional list of closed contours in board coordinates,
just like its existing `shapes`. `None` means the same shape on both faces. The bottom
copper, surface finish, and mask opening use the bottom geometry; inner layers keep the
existing top geometry. These overrides apply only to pads on both copper faces
(through pads on multi-layer or inner copper). For a single-face pad, we use its existing
geometry on its own face and ignore `bottom`, with one Info note counting those pads.
Side-specific tenting and expansion still apply.

`Board.mask_openings` holds `MaskOpening { side, contour, holes }` for hand-drawn openings,
independent of pads. We subtract their union from both conformal mask coats and from
silkscreen on that side. Copper inside these openings receives the surface finish. Holes
inside an opening leave islands of mask and ink; contours may wind either way.

`Color.opacity: Option<f64>` distinguishes an unknown opacity (`None`) from an explicit
value, including zero. Unknown mask opacity uses 0.75; known mask opacity retains the
0.75 floor. Unknown body opacity uses 1.0. We read silkscreen opacity but leave it unused;
package silkscreen is always opaque and carries no opacity field. Without laminate negotiation, mask opacity remains 1.0.
These are neutral model and drawing capabilities; readers currently fill the new shape
and opening fields with defaults until they implement the corresponding file features.
The package schema remains `dutchman.parts/1`: areas and optional opacity use the existing
manifest fields, with no new overlay kinds.

The Dutchman side of add-ons is specified in dutchman-cad/dutchman#1005.

## Module interface

Dutchman runs this program as `dutchman electronics ...` and reads one JSON object from stdout per call:

| Command | Reply |
|---|---|
| `dutchman-electronics info --json` | `dutchman.module-info/1`: `name`, `display_name`, `version`, `repository`, `interface` (1), `extensions`, and `formats` (`id`, `name`, `extension`, `extensions`) |
| `dutchman-electronics detect <file> --json` | `dutchman.module-detect/1`: `detection` (`yes`, `maybe_by_name` or `no`) and `format` (the format id, or `null`) |
| `dutchman-electronics read <file> --out <dir> --json [--models <dir>]... [--copper outer\|all] [--bodies file\|standard] [--accept-overlay laminate]...` | Writes a placed-parts package into `<dir>` (the STEP files, then `manifest.json`) and prints the same manifest, `dutchman.parts/1`. `--accept-overlay <kind>` (repeatable; `laminate` is the only kind today, any other value is an error naming the accepted kinds) says the loader draws that overlay kind. Without `--accept-overlay laminate` there is no `laminate` overlay and every mask's `opacity` is 1.0, so an older loader gets copper hidden under an opaque mask, as in any CAD tool's viewport, and never an overlay kind it does not know; what follows about the laminate and a see-through mask (the larger of the file's opacity and 0.75) is the output with it. Manifest: `models` (`id`, `file`), `parts` (`name`, `model`, 3x4 `transform`, `color`, `metallic` and `roughness` (the `Board` and `Plating` parts, and a component with a colored extruded body, which adds `opacity`), `group` (the folder a loader shows the part in: `Components top` or `Components bottom` by the component's side, or `Free bodies` for a 3D body that belongs to no component; none on `Board` and `Plating`, which stay at the top level; one level, no nesting; a loader that does not know the field ignores it), `notes`), `overlays` (copper, silkscreen and solder mask as flat areas: `name`, `part`, `group` (always `Layers`), `kind` (`laminate`, `copper`, `silkscreen` or `mask`; a loader that does not know `laminate` skips that overlay), `finish`, `side` (`top`, `bottom`, or `inner` for an inner copper layer, which `--copper all` adds), `z`, `thickness` (optional: each area is extruded from `z` away from the board, up for the top side and down for the bottom, into one solid per overlay; absent means a flat sheet), `color`, `metallic` and `roughness` (each 0 to 1; everything is non-metallic so it reads right in a viewport with nothing to reflect), `opacity` (the mask only), `areas` of `outer` and `holes` rings, agreed on dutchman-cad/dutchman#1005), `notes` (`Info`, `Degraded`, `Dropped`). Today the package holds the board itself (`Board`, `board.step`: outline, cutouts and drilled holes, exact), each component as a part named by its designator, with its 3D model (a component with several 3D bodies gets one part for each, all in the component's group and each with its own color and opacity: the first is named by the designator, the others `<designator> body 2`, `<designator> body 3`, and so on, made unique; a body is an embedded or linked STEP model, an extruded prism, or an exact sphere, a closed solid of two hemispherical faces on one `SPHERICAL_SURFACE`, one file for each radius, placed by its center; a 3D body that belongs to no component, which the board model keeps in `Board.free_models` in board coordinates (x and y as seen from the top, z up from the top face of the board), is a part named `Free body 1`, `Free body 2`, ..., in the group `Free bodies`; one component gets at most 256 bodies and the package at most 10000 free bodies and 100000 bodies in all, and what is left out is named in a `Dropped` note): an embedded STEP model as the board holds it, a linked STEP model (one the board names but does not hold) found in the board's folder, its `Models` folder or a `--models` folder, in that order: for Altium a file of that name with the board's checksum; for KiCad, which keeps no checksum, the file by its name alone, as KiCad finds it (a link under `${KIPRJMOD}`, `${KICADn_3DMODEL_DIR}` or `${KISYS3DMOD}` is looked for below each of those folders, then by its bare name; an absolute link or one under another variable by its bare name only; a `.wrl` link takes a `.step` or `.stp` file of the same name, since a mesh cannot be used; a model more than 10 times the size of its part's pads in either direction is taken for the wrong file and not used) (an Info note names each one used, saying when it was matched by name only, a Warning each one missing, with another checksum, only a mesh or too large), or an extruded body written as an exact solid (an Altium cylinder body too, as an extruded circle; a sphere body is not read yet; `model-1.step`, ..., each distinct model once), the copper lining plated holes (`Plating`, `plating.step`: one closed solid per hole in one file, each a tube from the drill radius in to 0.025 mm less, from the board's bottom to its top; every via and every pad whose drill is plated; none when no hole is plated; the thickness is assumed, which an Info note says), copper, silkscreen and solder mask as overlays for each face (tracks, arcs, pours, vias and pads united, drills cut out, within 0.02 mm, then thinned: a point whose removal moves its outline by at most 0.005 mm is left out (only where the area stays valid and touches no other area of its overlay; an area with more than 1000 holes is not thinned), and areas under 0.001 mm2, slivers with no surface, are left out with an Info note; the laminate is the board outline, arcs included, less its cutouts and drills (opaque, no `opacity`); the copper is all copper of the face united (tracks, arcs, pours, vias, pads) less every pad drill; the finish (`Finish top`, `Finish bottom`; kind `copper`, `finish` `bare`) is the copper inside the mask openings, so a track running into a pad is finished as far as the opening reaches; the mask is the outline less its cutouts, the openings of the bare pads (each pad grown by the mask expansion on every side, corners kept square: the pad's own value for that side when the file gives one (for Altium one measured from the hole's edge is turned into one from the pad's edge, by the shorter side for a pad that is not round, with an Info note when the opening ends up smaller than the pad), else the board's own value when the file gives one (for Altium the board-wide `SolderMaskExpansion` rule; a rule for some objects only, which an Info note names, is not read), otherwise 0.1 mm; a via side the file says is not tented is opened the same way, its ring grown by the via's own mask expansion for that side where the file gives one, with its drill open on that side only, and a via side the file says is tented, or does not mention, gets none) and every drill, and not less the copper, which it covers; its `opacity` is the larger of the board file's mask opacity for that side (Altium's 3D-view setting, 0.6 on the blue-mask test board; the KiCad reader gives none) and 0.75, which is also the value when the file gives none: SolidCraft blends a see-through solid linearly with what lies behind it, so at 0.6 a copper pour shows 40 percent of its orange and a blue mask comes out lilac-gray, where at 0.75 the mask keeps its color and the copper still shows as lines and pours; copper shows through it and laminate shows darker; the silkscreen is cut from the openings and every drill; the stacking, with T the board thickness and the bottom side mirrored below z = 0: laminate at `z` = T, `thickness` 0.005; copper at `z` = T, `thickness` 0.035; finish at `z` = T + 0.035, `thickness` 0.004; mask at `z` = T, `thickness` 0.045 (0.010 above the copper, which stands inside it, seen through it); silkscreen at `z` = T + 0.045, `thickness` 0.015 (real ink, on top of the mask); when the solids of a board would pass 250000 faces (Dutchman's own limit) the largest overlays are written flat, without `thickness`, and every other overlay of their side with them (a flat sheet would hide inside a solid one), and a Degraded note names each; each flat sheet is lifted off the face so it does not share its plane: laminate 0.001 mm, copper 0.003, finish 0.005, mask 0.007, silkscreen 0.01; a via side that is tented, or that the file does not mention (an Info note counts those: "N via(s) drawn tented; the file doesn't say"), is drawn as a solid copper disc under the mask whose drill cuts only the silkscreen; a blind or buried via is drawn only on the faces and inner layers its span reaches, with no ring or opening on a face it does not reach; the board and plating keep the hole, and a through-hole pad's drill cuts every overlay), a `color` on every overlay and on the `Board` and `Plating` parts as lowercase `#rrggbb`: the board file's own where it gives one (the solder mask and silkscreen color of each side; the bare pads of each side by the surface finish process, ENIG, ENEPIG and immersion gold gold `#d4af37` with `roughness` 0.45, HASL, lead-free HASL, immersion tin and immersion silver silver `#c9ccd1` with 0.4, OSP and bare copper copper `#b87333` with 0.5, and an unknown process gold with an Info note naming it; the `Plating` takes the top finish), otherwise module defaults (mask `#1f5f3a`, laminate `#2a2f1c`, copper `#b87333`, bare pads and plating `#d4af37`, silkscreen `#f2f2f2`, board edge `#c8b27a`); a component part carries a `color`, an `opacity` (0 to 1), `metallic` 0 and `roughness` 0.6 only when its extruded body is colored in the file, and none for an embedded STEP model, which brings its own, text: a TrueType text whose font file the board embeds is drawn with the real letter outlines as filled areas (letters scaled so the font's ascender less descender is the text height, glyphs on their advances, the left edge of the ink at the text's position and the baseline at its y, mirrored and turned about the position; holes stay holes; curves within 0.02 mm; bold and italic come from the matching embedded file; no note), any other text, and a TrueType text whose font is not embedded or cannot be read, as pen strokes in the Hershey Roman Simplex font (a Degraded note names the font), all under one point budget that a note reports when it runs out, and notes for what is not in it (linked models not found; inner copper layers, which lie inside the board and are left out unless `--copper all` is given: then each inner copper layer is an overlay of its own named `Copper` and the layer's name, side `inner`, with its tracks, arcs, pours and texts and every via and drilled pad through it, every drill cut out (an Altium internal plane instead holds its split-plane regions, each less a round clearance of the hole plus twice the plane clearance around every via and through-hole pad of another net, and a thermal relief around each of its own net: air from the hole plus twice the relief expansion out by twice the air gap, crossed by four spokes of the conductor width at 45 degrees, and less every board cutout grown by the plane's pull-back from the board edge (its `PULLBACKDISTANCE` in the stack, else Altium's default 0.508); from the board-wide plane rules, overrides on single vias or pads not read; vias and pads get no pad of their own there), at its place in the stack (its `z` the layer's lower face, `thickness` its copper; from the file's stack-up, or for a KiCad board without one spread evenly through the thickness, which a note says), extruded upward, copper `#b87333` with no mask). |

A part's `transform` is 3 rows of 4 numbers, row by row: the first three columns are the rotation, the last column the translation in mm, so a model point `p` lands at `R p + t`. A top-side component's model sits on the board's top face (z = thickness), turned by the component's rotation; a bottom-side one is turned over by half a turn about its own X axis onto the bottom face (z = 0). The frame is written out in `board_model::Placement`.

A sphere's center uses the same ModelRef offset and rotation as an extruded outline: the center turns about the part origin (X, then Y, then Z), then moves by the offset, before the component's placement puts it on the board.

A failure is `{"schema": ..., "ok": false, "error": "..."}` with a non-zero exit code.

## Standard bodies

A part with no usable 3D model would be a flat box, or missing. `read --bodies file|standard` gives such a part a realistic body, drawn as exact solids (boxes and prisms, never a mesh) from public package standards, with no vendor CAD file:

- `--bodies file` (the default) keeps the board's own models, and a part with no model at all (no embedded model, no linked model found, no extruded body) gets a standard body when its package is recognized. A part gets a standard body only when it has no usable model at all: a part with any body of its own (a sphere, a second prism, an embedded model) never does.
- `--bodies standard` also replaces a plain extruded body of a recognized package. An embedded or linked STEP model is always kept.
- A package is recognized from the part's pads first (their number, pitch, size and the distance between rows, in the part's own frame, so a turned or bottom-side part works), and the footprint name only breaks a tie or refuses a lookalike (a SOD-123 diode has chip pads). A part whose pads are no known package is left as before.
- Each generated body carries an `Info` note on both parts naming the package, and the package has one `Info` note per package listing the parts.
- A part in the manifest has one color, so a body is two parts in the same group: the part named by the designator (the molded body, ceramic or plastic) and `<designator> leads` (tin gray terminations and leads, gold pins). Equal bodies share their model files.

| Package | Recognized from | Standard and nominal numbers |
|---|---|---|
| Chip resistor, capacitor, inductor, LED 0201, 0402, 0603, 0805, 1206 | two pads in line, their center distance | EIA/IEC 60115-8 sizes 0.6 x 0.3, 1.0 x 0.5, 1.6 x 0.8, 2.0 x 1.25, 3.2 x 1.6 mm, terminations 0.12 to 0.5 mm; thickness is the usual datasheet value (not fixed by the standard); the designator or name says resistor (black), capacitor (tan), inductor or LED |
| SOT-23-3, -5, -6 | two rows, 0.95 mm pitch, 3, 5 or 6 pads | JEDEC TO-236 (body 2.92 x 1.30 mm) and MO-178 (2.90 x 1.60 mm), gull-wing leads |
| SOIC-8, -14, -16 | two rows, 1.27 mm pitch | JEDEC MS-012 (3.90 mm body, 6.00 mm over the leads) |
| TSSOP-8 to -28 | two rows, 0.65 mm pitch | JEDEC MO-153 (4.40 mm body, 6.40 mm over the leads) |
| TQFP and LQFP 32, 44, 48, 64, 100 | four sides, pitch and pin count | JEDEC MS-026 (7, 10 and 14 mm bodies, 9, 12 and 16 mm over the leads) |
| QFN and DFN | four sides or two rows of short pads, with an optional exposed pad | JEDEC MO-220 and MO-229 (terminals 0.4 mm long, body size from the ring of pads) |
| 2.54 mm pin headers, 1 or 2 rows | through-hole pads on a 2.54 mm grid in a connector part | black plastic strip, 0.64 mm square gold pins (the common 0.100 inch header; no standard fixes it) |

The numbers are in the doc comment of each package in `crates/dutchman-electronics/src/standard.rs`.

## Roadmap

Each step is its own small pull request. Each ends with something you can run and see.

1. **List the streams.** Open the `.PcbDoc` container and print what is inside. Done as a stub: `cargo run -- streams board.PcbDoc`.
2. **Board outline.** Read the board shape and thickness into `Board::outline`.
3. **Copper.** Tracks, arcs, pads, vias, pours into `Board::copper`.
4. **Silkscreen.** Text and lines on the overlay layers into `Board::texts`.
5. **Components.** Designator, footprint, position, rotation, side, and the 3D model reference, into `Board::components`.
6. **Parts package.** Write `dutchman.parts/1`: the board as a STEP solid, each component's model (extracted from the `.PcbDoc` when embedded), and the manifest. `read` and `info --json` commands.
7. **Sync live link.** Dutchman Sync (dutchman-cad/dutchman#931) watches the `.PcbDoc` and re-converts it when you save in Altium, so SolidCraft follows your board.

## Try it

```sh
cargo build
cargo test
cargo run -- streams path/to/board.PcbDoc
cargo run -- read path/to/project.PrjPcb --out out --board main --param BoardRevision=C
```

A text such as `.PCBNumber` refers to a project parameter, and `.Designator`, `.Comment` and `.Layer_Name` to the
component or layer the text belongs to; they are shown with their value, as Altium shows them, also in quotes
(`Rev '.BoardRevision'`), where a text may hold several. Reading a `.PrjPcb`
reads its board and uses the project's parameters; `--board` picks the board when a project holds several, by its
name or, when two share a name, by the link the project lists (`a\Main.PcbDoc`). Only boards inside the project's
folder are read, also when a symbolic link would lead elsewhere. A `.PcbDoc` on its own can take a project's
parameters with `--project file.PrjPcb`; without it we look in the board's own folder (not below it, at most 64
`.PrjPcb` files, each up to the size limit) for projects that link the board: exactly one gives its parameters (an Info
note names it), several give none (a warning names them and suggests `--project`), none changes nothing. A text whose
parameter is not found stays as written, as Altium shows it. `--param NAME=VALUE` gives one by hand, winning over the project's. A
parameter value may be up to 64 KiB long.

A KiCad text such as `Rev ${REVISION}` uses a text variable and is shown with its value, as KiCad shows it: the title
block (`${TITLE}`, `${REVISION}`, `${COMPANY}`, `${ISSUE_DATE}`, `${COMMENT1}`...), the board's own properties, the
text's layer (`${LAYER}`), the footprint's fields in its own texts (`${REFERENCE}`, `${VALUE}`, `${MPN}`), another
footprint's (`${J1:VALUE}`), `${FILENAME}` and `${PROJECTNAME}` (from the board's file name only), and the project's
text variables. Those come from the `.kicad_pro` of the board's own name in its folder, or from `--project
file.kicad_pro`; `--param NAME=VALUE` wins over both (KiCad's names are case-sensitive). Variables inside variables
are filled in up to 8 deep; a name nothing defines stays as typed, with a warning (the table in
`crates/kicad-pcb/tests/boards/k10-text-vars.README.txt` shows each case).

New here? Read [CONTRIBUTING.md](CONTRIBUTING.md). Working with an AI? Point it at [AGENTS.md](AGENTS.md) first.

## License

Apache-2.0, the same as Dutchman. See [LICENSE](LICENSE). The one dependency so far, `cfb`, is MIT.
