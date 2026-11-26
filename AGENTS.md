# Instructions for AI agents

This repository is Dutchman Electronics: it reads circuit boards (Altium `.PcbDoc` first) into a neutral board model that Dutchman turns into SolidCraft designs. Your human is probably a hardware engineer with little software experience. Explain in plain words, one step at a time, and do the typing for them.

## Read first

- `README.md`: the goal and the roadmap. Work on one roadmap step at a time.
- `crates/board-model/src/lib.rs`: the module boundary (`InputFormat`, `Board`).
- `crates/altium-pcbdoc/src/lib.rs`: the Altium reader, its own crate.
- `crates/dutchman-electronics/`: the command Dutchman runs; it only lists the formats and calls them.
- `CONTRIBUTING.md`: how test boards are made.
- `LEGAL.md`: allowed and forbidden sources, and the provenance sentence a reader pull request needs.

## Hard rules

1. **Clean-room.** Learn the format only from (a) public format descriptions that carry no restrictive license, and (b) test files the contributor made and says so, with exact numbers they wrote down. Work out each stream's layout by comparing small test boards that differ by one thing.
2. **Allowed sources only (LEGAL.md).** For reader code, never use the Altium SDK, Altium's scripting API documentation, headers or samples, documents under NDA, or decompiled Altium programs. Never read, copy or translate GPL, LGPL or AGPL code, explicitly including KiCad's Altium importer (`pcbnew/plugins/altium`) and all of KiCad's own source code. **Never fetch, clone, browse or search KiCad's source tree** (`gitlab.com/kicad/code/*` and its mirrors, such as `github.com/KiCad/kicad-source-mirror`), not even to check a license. KiCad's file formats come only from its published file-format documentation. If you come near a forbidden source anyway, stop, delete what you fetched, and add a line to `docs/provenance-log.md`. MIT, Apache-2.0 or BSD code may be used with attribution: name the source in the pull request and keep its license notice. Do not write down file layouts you cannot source. A pull request that changes a reader carries LEGAL.md's provenance sentence, ticked, and only if it is true. Test-board builder scripts may use Altium's scripting; reader code never learns from them.
3. **No customer data.** Never add or open a board the contributor did not design as a test. Test boards in the repository carry a CC0 line (see CONTRIBUTING.md). Never upload a board anywhere.
4. **Rust only, every platform.** Pure Rust, no unsafe code (the workspace forbids it). No Windows-only (or any platform-only) dependencies, paths or path separators: the reader runs on Windows, macOS and Linux. Pull requests are tested on Linux; `main` is also tested on Windows and macOS after each merge, and a failure there is fixed with a normal pull request. Dependencies must be MIT, Apache-2.0 or BSD, and each new one needs a stated reason.
5. **Never crash.** No `unwrap`, `expect`, `panic!`, `todo!`, indexing that can go out of range, or unchecked arithmetic on file values in shipped code. Input is hostile: bound sizes, counts and loops, return `ReadError`. Clippy denies these; tests may use them.
6. **Say what was dropped.** Anything the reader skips becomes a `Note`. Never lose data silently.
7. **Tests are required.** Every change ships with a test. A reader step is tested against a test board with known numbers from its README.txt (a tolerance of 0.001 mm).
8. **Keep the boundary.** `board-model` knows nothing about Altium and nothing about SolidCraft. Altium details stay in the `altium-pcbdoc` crate, which depends only on `board-model` (never on Dutchman). A new format is a new crate beside it. No Dutchman or SolidCraft types in this repository.
9. **Units.** Millimeters and degrees, counter-clockwise, Y up, in the neutral model. Convert at the edge.
10. **English** in code, comments, commit messages and docs.

## Before you hand back

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Do not push, open pull requests or post anywhere unless your human asks. One topic per pull request; the first sentence of the description must make sense to someone who never saw the chat. Fill in the pull-request template's declarations.
