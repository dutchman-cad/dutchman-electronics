# Dutchman Electronics

Reads circuit boards into [SolidCraft](https://github.com/tangency-labs/solidcraft) designs, through [Dutchman](https://github.com/dutchman-cad/dutchman). The first board format is Altium `.PcbDoc`.

It is an optional add-on. Dutchman works without it, and installing Dutchman does not install it. It is not part of cadmpeg: cadmpeg reads mechanical CAD, this reads electronics.

## The goal

Open an Altium board in SolidCraft and see what SolidCraft already shows for a KiCad board:

| Part of the board | How it arrives |
|---|---|
| The board | An exact solid, with its cutouts and drilled holes |
| Copper traces, pads, vias, pours | Flat surfaces on the board (overlay), or modelled with thickness |
| Silkscreen | Text and lines as outlines, in white |
| Components | Each one placed on the board with its 3D model (STEP) |

## How it plugs into Dutchman

Dutchman finds add-ons by name. Any program called `dutchman-<something>` that sits next to `dutchman` or on your PATH is found by Dutchman's launcher (`crates/dutchman-launcher` in the Dutchman repository). This module builds one such program, `dutchman-electronics`. If it is not installed, Dutchman says so and links here; nothing else changes.

```
board.PcbDoc --dutchman-electronics--> neutral board model --Dutchman--> board.sc
```

The "neutral board model" is plain Rust types in `crates/board-model`: outline, layers, copper, text, component placements and model references. It does not know Altium and it does not know SolidCraft. That is the one boundary to keep clean.

| Crate | What it is |
|---|---|
| `crates/board-model` | The neutral board model and the `InputFormat` trait |
| `crates/altium-pcbdoc` | Reads `.PcbDoc` into the board model. No Dutchman dependencies |
| `crates/dutchman-electronics` | The `dutchman-electronics` command; gathers the formats |

Adding KiCad or another format later means one new crate that fills the same model.

The full plan for add-ons is in `modules-spec.md` (kept in the Dutchman repository's docs once agreed).

## Roadmap

Each step is its own small pull request. Each ends with something you can run and see.

1. **List the streams.** Open the `.PcbDoc` container and print what is inside. Done as a stub: `cargo run -- streams board.PcbDoc`.
2. **Board outline.** Read the board shape and thickness into `Board::outline`.
3. **Copper.** Tracks, arcs, pads, vias, pours into `Board::copper`.
4. **Silkscreen.** Text and lines on the overlay layers into `Board::texts`.
5. **Components.** Designator, footprint, position, rotation, side, and the 3D model reference, into `Board::components`.
6. **Sync live link.** Dutchman Sync (dutchman-cad/dutchman#931) watches the `.PcbDoc` and re-converts it when you save in Altium, so SolidCraft follows your board.

## Try it

```sh
cargo build
cargo test
cargo run -- streams path/to/board.PcbDoc
```

New here? Read [CONTRIBUTING.md](CONTRIBUTING.md). Working with an AI? Point it at [AGENTS.md](AGENTS.md) first.

## Licence

Apache-2.0, the same as Dutchman. See [LICENSE](LICENSE). The one dependency so far, `cfb`, is MIT.
