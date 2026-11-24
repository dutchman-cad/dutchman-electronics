# Dutchman Electronics

Reads circuit boards into [SolidCraft](https://github.com/tangency-labs/solidcraft) designs, through [Dutchman](https://github.com/dutchman-cad/dutchman).

## Formats

| Format | Status |
|---|---|
| Altium (`.PcbDoc`) | In progress |
| tscircuit | In progress |
| KiCad | TBD; scope depends on the SolidCraft maintainers' answer on [tangency-labs/solidcraft#21320](https://github.com/tangency-labs/solidcraft/issues/21320) |
| Fusion Electronics | TBD |

It is an optional add-on. Dutchman works without it, and installing Dutchman does not install it. It is not part of cadmpeg: cadmpeg reads mechanical CAD, this reads electronics.

## The goal

Open an Altium board in SolidCraft and see what SolidCraft already shows for a KiCad board:

| Part of the board | How it arrives |
|---|---|
| The board | An exact solid, with its cutouts and drilled holes, in the colour of its FR4 edge |
| Plated holes | The copper lining every via and every plated pad hole, as thin tubes (`Plating`) |
| Copper traces, pads, vias, pours | Flat surfaces on the board (overlay): green under the mask, gold where bare, or modelled with thickness |
| Solder mask | A translucent green surface over the whole board face, open at the bare pads and every drill |
| Silkscreen | Text and lines as outlines, in white |
| Components | Each one placed on the board with its 3D model (STEP) |

It reads `.PcbDoc` files on Windows, macOS and Linux. Altium itself is only needed to make test boards. Pull requests are tested on Linux; every change on `main` is also tested on Windows and macOS.

## How it plugs into Dutchman

Dutchman finds add-ons by name. Any program called `dutchman-<something>` that sits next to `dutchman` or on your PATH is found by Dutchman's launcher (`crates/dutchman-launcher` in the Dutchman repository). This module builds one such program, `dutchman-electronics`. If it is not installed, Dutchman says so and links here; nothing else changes.

```
board.PcbDoc --reader--> neutral board model --package writer--> parts package --Dutchman--> board.sc
```

The parts package (`dutchman.parts/1`) is what Dutchman reads back from `dutchman-electronics read <file> --out <dir> --json`: one STEP file per distinct 3D model (the board itself included), a list of parts named by designator with their placement, copper, silkscreen and solder mask as flat overlays, and the copper lining plated holes. The contract is in dutchman-cad/dutchman#1005; a summary is in issue #1.

The "neutral board model" is plain Rust types in `crates/board-model`: outline, layers, copper, text, component placements and model references. It does not know Altium and it does not know SolidCraft. That is the one boundary to keep clean.

| Crate | What it is |
|---|---|
| `crates/board-model` | The neutral board model and the `InputFormat` trait |
| `crates/board-step` | Writes the board substrate (outline, cutouts, drilled holes, thickness) as an exact STEP solid, and the tubes of plated holes |
| `crates/altium-pcbdoc` | Reads `.PcbDoc` into the board model. No Dutchman dependencies |
| `crates/dutchman-electronics` | The `dutchman-electronics` command; gathers the formats |

Each further format is one new crate that fills the same model.

The Dutchman side of add-ons is specified in dutchman-cad/dutchman#1005.

## Module interface

Dutchman runs this program as `dutchman electronics ...` and reads one JSON object from stdout per call:

| Command | Reply |
|---|---|
| `dutchman-electronics info --json` | `dutchman.module-info/1`: `name`, `display_name`, `version`, `repository`, `interface` (1), `extensions`, and `formats` (`id`, `name`, `extension`, `extensions`) |
| `dutchman-electronics detect <file> --json` | `dutchman.module-detect/1`: `detection` (`yes`, `maybe_by_name` or `no`) and `format` (the format id, or `null`) |
| `dutchman-electronics read <file> --out <dir> --json` | Writes a placed-parts package into `<dir>` (the STEP files, then `manifest.json`) and prints the same manifest, `dutchman.parts/1`: `models` (`id`, `file`), `parts` (`name`, `model`, 3x4 `transform`, `color` (the `Board` and `Plating` parts only), `notes`), `overlays` (copper, silkscreen and solder mask as flat areas: `name`, `part`, `kind` (`copper`, `silkscreen` or `mask`), `finish`, `side`, `z`, `color`, `opacity` (the mask only), `areas` of `outer` and `holes` rings, agreed on dutchman-cad/dutchman#1005), `notes` (`Info`, `Degraded`, `Dropped`). Today the package holds the board itself (`Board`, `board.step`: outline, cutouts and drilled holes, exact), each component as a part named by its designator, with its 3D model: an embedded STEP model as the board holds it, or an extruded body written as an exact solid (`model-1.step`, ..., each distinct model once), the copper lining plated holes (`Plating`, `plating.step`: one closed solid per hole in one file, each a tube from the drill radius in to 0.025 mm less, from the board's bottom to its top; every via and every pad whose drill is plated; none when no hole is plated; the thickness is assumed, which an Info note says), copper, silkscreen and solder mask as overlays for each face (tracks, arcs, pours, vias and pads united, drills cut out, masked copper stopping at the pads, within 0.02 mm; the mask is the board outline, arcs included, less its cutouts, the bare pads and every drill, lifted 0.005 mm off the face with `opacity` 0.85), a `color` on every overlay and on the `Board` and `Plating` parts as lowercase `#rrggbb` (module defaults, since the board model holds no colours yet: mask `#1f5f3a`, copper under the mask `#3f8f4f`, bare pads and plating `#d4af37`, silkscreen `#f2f2f2`, board edge `#c8b27a`; component parts carry none), text drawn as pen strokes in the Hershey Roman Simplex font (the board's own font is not used yet, which a Degraded note says), and notes for what is not in it (models that live outside the board file, not yet; inner copper layers, which lie inside the board and are left out). |

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
```

New here? Read [CONTRIBUTING.md](CONTRIBUTING.md). Working with an AI? Point it at [AGENTS.md](AGENTS.md) first.

## Licence

Apache-2.0, the same as Dutchman. See [LICENSE](LICENSE). The one dependency so far, `cfb`, is MIT.
