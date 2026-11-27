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
| Copper traces, pads, vias, pours | Thin solids on the board (overlay, 0.035 mm thick): green under the mask, gold where bare |
| Solder mask | A translucent green solid (0.020 mm thick) over the board face, open at the bare pads (each opening is the pad grown by the mask expansion: the board file's own value, for Altium its board-wide solder mask expansion rule, otherwise 0.1 mm, so a thin ring of bare laminate shows round it; tented vias get none), the copper under the mask and every drill, so tracks stand proud of it |
| Silkscreen | Text and lines as outlines, in white (TrueType text with its real letter outlines, followed to a four-hundredth of its height; Altium stroke text in the Hershey font nearest the one it names: Sans Serif in Roman Simplex, Serif in Times Roman, and Default in Roman Simplex letters on Default's own character spacing, so its width and placement match Altium but its letter shapes differ), clipped away from the mask openings (pads and their ring of laminate) and the drills, as a fab does |
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

The Dutchman side of add-ons is specified in dutchman-cad/dutchman#1005.

## Module interface

Dutchman runs this program as `dutchman electronics ...` and reads one JSON object from stdout per call:

| Command | Reply |
|---|---|
| `dutchman-electronics info --json` | `dutchman.module-info/1`: `name`, `display_name`, `version`, `repository`, `interface` (1), `extensions`, and `formats` (`id`, `name`, `extension`, `extensions`) |
| `dutchman-electronics detect <file> --json` | `dutchman.module-detect/1`: `detection` (`yes`, `maybe_by_name` or `no`) and `format` (the format id, or `null`) |
| `dutchman-electronics read <file> --out <dir> --json [--models <dir>]... [--copper outer\|all]` | Writes a placed-parts package into `<dir>` (the STEP files, then `manifest.json`) and prints the same manifest, `dutchman.parts/1`: `models` (`id`, `file`), `parts` (`name`, `model`, 3x4 `transform`, `color`, `metallic` and `roughness` (the `Board` and `Plating` parts, and a component with a colored extruded body, which adds `opacity`), `group` (the folder a loader shows the part in: `Components top` or `Components bottom` by the component's side; none on `Board` and `Plating`, which stay at the top level; one level, no nesting; a loader that does not know the field ignores it), `notes`), `overlays` (copper, silkscreen and solder mask as flat areas: `name`, `part`, `group` (always `Layers`), `kind` (`copper`, `silkscreen` or `mask`), `finish`, `side` (`top`, `bottom`, or `inner` for an inner copper layer, which `--copper all` adds), `z`, `thickness` (optional: each area is extruded from `z` away from the board, up for the top side and down for the bottom, into one solid per overlay; absent means a flat sheet), `color`, `metallic` and `roughness` (each 0 to 1; everything is non-metallic so it reads right in a viewport with nothing to reflect), `opacity` (the mask only), `areas` of `outer` and `holes` rings, agreed on dutchman-cad/dutchman#1005), `notes` (`Info`, `Degraded`, `Dropped`). Today the package holds the board itself (`Board`, `board.step`: outline, cutouts and drilled holes, exact), each component as a part named by its designator, with its 3D model: an embedded STEP model as the board holds it, a linked STEP model (one the board names but does not hold) found in the board's folder, its `Models` folder or a `--models` folder, in that order: for Altium a file of that name with the board's checksum; for KiCad, which keeps no checksum, the file by its name alone, as KiCad finds it (a link under `${KIPRJMOD}`, `${KICADn_3DMODEL_DIR}` or `${KISYS3DMOD}` is looked for below each of those folders, then by its bare name; an absolute link or one under another variable by its bare name only; a `.wrl` link takes a `.step` or `.stp` file of the same name, since a mesh cannot be used; a model more than 10 times the size of its part's pads in either direction is taken for the wrong file and not used) (an Info note names each one used, saying when it was matched by name only, a Warning each one missing, with another checksum, only a mesh or too large), or an extruded body written as an exact solid (`model-1.step`, ..., each distinct model once), the copper lining plated holes (`Plating`, `plating.step`: one closed solid per hole in one file, each a tube from the drill radius in to 0.025 mm less, from the board's bottom to its top; every via and every pad whose drill is plated; none when no hole is plated; the thickness is assumed, which an Info note says), copper, silkscreen and solder mask as overlays for each face (tracks, arcs, pours, vias and pads united, drills cut out, masked copper stopping at the pads, within 0.02 mm, then thinned: a point whose removal moves its outline by at most 0.005 mm is left out (only where the area stays valid and touches no other area of its overlay; an area with more than 1000 holes is not thinned), and areas under 0.001 mm2, slivers with no surface, are left out with an Info note; the mask is the board outline, arcs included, less its cutouts, the openings of the bare pads (each pad grown by the mask expansion on every side, corners kept square: the board's own value when the file gives one (for Altium the board-wide `SolderMaskExpansion` rule; a rule for some objects only, which an Info note names, and an override on one pad are not read), otherwise 0.1 mm; vias are tented and get none), the copper under the mask and every drill, and the silkscreen is cut from those openings and every drill, with `opacity` 1 (solid: the mask lies only on bare laminate, which a translucent mask would let show through paler than the copper beside it); the stacking, with T the board thickness and the bottom side mirrored below z = 0: copper under the mask and bare pads at `z` = T, `thickness` 0.035; mask at `z` = T, `thickness` 0.020, so tracks stand 0.015 mm proud of it; silkscreen at `z` = T + 0.035, `thickness` 0.015 (real ink; it starts at the top of the copper, so it clears both the copper and the mask, which tops out at T + 0.020); when the solids of a board would pass 250000 faces (Dutchman's own limit) the largest overlays are written flat, without `thickness`, and a Degraded note names each; each flat sheet is lifted off the face so it does not share its plane: copper under the mask 0.007 mm, mask 0.005, pads 0.001, silkscreen 0.01; vias are drawn tented, as a solid copper disc under the mask whose drill cuts only the silkscreen, because most boards tent them; the board and plating keep the hole, and a through-hole pad's drill cuts every overlay), a `color` on every overlay and on the `Board` and `Plating` parts as lowercase `#rrggbb`: the board file's own where it gives one (the solder mask and silkscreen color of each side; the copper under the mask is the mask color with red x1.39, green x1.158, blue x1.155, capped at 255; the bare pads of each side by the surface finish process, ENIG, ENEPIG and immersion gold gold `#d4af37` with `roughness` 0.45, HASL, lead-free HASL, immersion tin and immersion silver silver `#c9ccd1` with 0.4, OSP and bare copper copper `#b87333` with 0.5, and an unknown process gold with an Info note naming it; the `Plating` takes the top finish), otherwise module defaults (mask `#1f5f3a`, copper under the mask `#2b6e43`, bare pads and plating `#d4af37`, silkscreen `#f2f2f2`, board edge `#c8b27a`); a component part carries a `color`, an `opacity` (0 to 1), `metallic` 0 and `roughness` 0.6 only when its extruded body is colored in the file, and none for an embedded STEP model, which brings its own, text: a TrueType text whose font file the board embeds is drawn with the real letter outlines as filled areas (letters scaled so the font's ascender less descender is the text height, glyphs on their advances, the left edge of the ink at the text's position and the baseline at its y, mirrored and turned about the position; holes stay holes; curves within 0.02 mm; bold and italic come from the matching embedded file; no note), any other text, and a TrueType text whose font is not embedded or cannot be read, as pen strokes in the Hershey Roman Simplex font (a Degraded note names the font), all under one point budget that a note reports when it runs out, and notes for what is not in it (linked models not found; inner copper layers, which lie inside the board and are left out unless `--copper all` is given: then each inner copper layer is an overlay of its own named `Copper` and the layer's name, side `inner`, with its tracks, arcs, pours and texts and every via and drilled pad through it, every drill cut out (an Altium internal plane instead holds its split-plane regions, each less a round clearance of the hole plus twice the plane clearance around every via and through-hole pad of another net, and a thermal relief around each of its own net: air from the hole plus twice the relief expansion out by twice the air gap, crossed by four spokes of the conductor width at 45 degrees, and less every board cutout grown by the plane's pull-back from the board edge (its `PULLBACKDISTANCE` in the stack, else Altium's default 0.508); from the board-wide plane rules, overrides on single vias or pads not read; vias and pads get no pad of their own there), at its place in the stack (its `z` the layer's lower face, `thickness` its copper; from the file's stack-up, or for a KiCad board without one spread evenly through the thickness, which a note says), extruded upward, copper `#b87333` with no mask). |

A part's `transform` is 3 rows of 4 numbers, row by row: the first three columns are the rotation, the last column the translation in mm, so a model point `p` lands at `R p + t`. A top-side component's model sits on the board's top face (z = thickness), turned by the component's rotation; a bottom-side one is turned over by half a turn about its own X axis onto the bottom face (z = 0). The frame is written out in `board_model::Placement`.

A failure is `{"schema": ..., "ok": false, "error": "..."}` with a non-zero exit code.

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
component or layer the text belongs to; they are shown with their value, as Altium shows them. Reading a `.PrjPcb`
reads its board and uses the project's parameters; `--board` picks the board when a project holds several, by its
name or, when two share a name, by the link the project lists (`a\Main.PcbDoc`). Only boards inside the project's
folder are read, also when a symbolic link would lead elsewhere. A `.PcbDoc` on its own can take a project's
parameters with `--project file.PrjPcb`, and `--param NAME=VALUE` gives one by hand, winning over the project's. A
parameter value may be up to 64 KiB long.

New here? Read [CONTRIBUTING.md](CONTRIBUTING.md). Working with an AI? Point it at [AGENTS.md](AGENTS.md) first.

## License

Apache-2.0, the same as Dutchman. See [LICENSE](LICENSE). The one dependency so far, `cfb`, is MIT.
