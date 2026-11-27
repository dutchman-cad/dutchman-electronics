# Contributing to Dutchman Electronics

You design boards. You do not have to be a programmer. This project is built to be worked on by a hardware engineer together with an AI assistant (Claude or similar), and everything here is written for that.

## The idea in one minute

An Altium `.PcbDoc` file is a box with labeled drawers inside (a "container"). Each drawer holds one kind of data: board settings, tracks, components, text. We:

1. open the box and list the drawers (done),
2. learn what is in each drawer by making tiny test boards and watching what changes,
3. turn that into Rust code that fills a simple "board model",
4. let Dutchman turn the board model into a SolidCraft design.

You are the one who can make the test boards and say what they should look like. The AI writes most of the code from what you find. That is a real division of labour.

## What you need (once, about 20 minutes)

- Git and a GitHub account.
- Rust: install from https://rustup.rs (it gives you `cargo`, the Rust build tool).
- Altium Designer, to make your own test boards.
- An AI assistant that can read and run commands in the project folder (for example Claude Code).

## Build and test

In the project folder:

```sh
cargo build                 # compiles everything
cargo test                  # runs all tests; they must all pass
cargo clippy --all-targets -- -D warnings   # Rust's style and bug checker; must be silent
cargo fmt --all             # tidies the formatting
cargo run -- streams my-board.PcbDoc        # lists the drawers in a board file
```

Run `cargo test` and `cargo clippy` before every pull request. The same checks run on GitHub when you open one.

## Make a test board (the most useful thing you can do)

The rule: **only boards you designed yourself, for testing. Never a customer's board, an employer's board, or anything under an NDA.** If you are not sure it is yours to share, it is not a test board.

Make boards that are as small and boring as possible, one new thing at a time:

1. `empty-outline.PcbDoc`: a 50 mm by 30 mm rectangle, 1.6 mm thick, nothing else.
2. `outline-with-hole.PcbDoc`: the same, plus one 3 mm hole and one rectangular cutout.
3. `one-track.PcbDoc`: the rectangle plus one top-layer track, 10 mm long, 0.5 mm wide.
4. `one-resistor.PcbDoc`: a single 0603 resistor with a 3D model, designator `R1`, at a known position.

Write the exact numbers down in a `README.txt` beside each file (where you placed things, in millimeters), and the Altium version that saved it (e.g. `Altium Designer 26.12.2`). Those numbers are the "right answer" the tests check against; the version tells us which file layout a board uses.

Then put each board in `crates/altium-pcbdoc/tests/boards/` with its `README.txt` (either `README.txt` in the board's own folder, or `<board name>.README.txt` beside it), and this line at the top of that file:

```
Original test board made for Dutchman Electronics. Released under CC0 1.0 (public domain): https://creativecommons.org/publicdomain/zero/1.0/
```

CC0 means anyone may use it for anything with no conditions. Do not add a board without that line; a test refuses boards that lack it. Do not add 3D models you did not make or that you downloaded; use a simple box you modeled yourself.

### Check a board for personal traces before sharing it

A board file can carry more than the board: the folder it was saved in, a user or computer name,
a company or license name. Before a board leaves your computer (a pull request, a zip, a chat), run:

```sh
cargo run -- privacy-check path/to/board.PcbDoc --watch "Your Name" --watch "Your Company" --watch "your-login"
```

Add a `--watch` for anything that would point to you or your employer: your name, your Windows
login, your computer's name, your company, your license or serial number. It searches every part
of the file, in both text encodings Altium uses, ignoring upper and lower case. It must end with
`clean`. If it does not, it shows where the text is; tell us in #electronics-dev (without pasting
the personal text itself) and we will work out how the builder script blanks it.

`--all-text` also lists every piece of text in the file, so you can read through it yourself.
The words you watch for are never saved. CI checks every committed board for paths, e-mail
addresses and computer names as well.

### Scripted test boards (preferred)

Making boards with a script inside Altium is better than by hand: the script holds the exact numbers, and changing one thing means changing one value. If you script a board:

- Put the script next to the board it makes, with the same name (`empty-outline.PcbDoc` and `empty-outline.pas`, or whatever extension your script language uses), so anyone with Altium can make it again.
- The `README.txt` still lists the numbers in plain words; they should match the values in the script.
- Write the script yourself. Do not copy Altium's example scripts or code from their documentation into this repository; using Altium's scripting to drive Altium is fine. The script is released under CC0 with its board (same line at the top of the README.txt).


### KiCad test boards

KiCad boards go in `crates/kicad-pcb/tests/boards/`, each `<name>.kicad_pcb` with a
`<name>.README.txt` that starts with the same CC0 line and lists every number. A KiCad file is
text, so a test (`boards_are_ours.rs`) also refuses a board that names a path, an e-mail address
or a computer. The board scripts are in `fixtures/kicad`; `fixtures/kicad/run.sh` rebuilds all
boards with KiCad 9 in a container (needs podman).

### Open-hardware boards made by others (KiCad)

Larger real-world boards from other people's open-hardware projects go in
`crates/kicad-pcb/tests/open-hardware/<board>/`, one folder per board. These are the only boards
that may carry a license line other than the CC0 one, and only with one of these licenses:

| License line id | What it asks of us |
|---|---|
| `CC0-1.0` | nothing |
| `CC-BY-4.0` | name the creator, the license and the source, and say it was changed |
| `CC-BY-SA-4.0` | the same, and the board file (and any changed copy of it) stays under CC BY-SA 4.0 |

We accept only version 4.0 of the two attribution licenses, and no `-NC` (non-commercial) or
`-ND` (no changes) variant, because we must be free to re-save a board and to ship the repository
commercially. Reading a board with our code does not make our code a copy of it, so the
Apache-2.0 license of the rest of the repository stays as it is; the board folder keeps its own
license. CERN-OHL, TAPR, GPL-family and other hardware licenses are not accepted for now: ask first.

Rules for such a folder (the test `boards_are_ours.rs` enforces them):

- The board is saved by KiCad 9 or later. If the project published an older file, re-save it with
  KiCad 9 (`kicad/kicad:9.0`) and say so in the README.txt. Change nothing else, and strip nothing
  silently: if a board names a person, a company, an e-mail address or a path on someone's computer
  that is not the project's own public attribution, choose another board.
- The board comes from the project's own published repository or release, never from KiCad's
  source repositories (they are GPL: see [LEGAL.md](LEGAL.md)). The download goes in its own empty
  directory and nothing in it is run.
- `README.txt` starts with this line, with the id and URL of one accepted license:
  `Openly licensed board by others, not CC0. License: CC-BY-4.0, https://creativecommons.org/licenses/by/4.0/`
- The README.txt then names the source URL (`Source: https://...`), the exact commit or tag, the
  attribution the license asks for (`Attribution ...`, not needed for CC0), how the file was
  re-saved, and the key numbers as KiCad itself reports them (`- Outline`, `- Thickness`,
  `- Copper layers`, `- Footprints`), never numbers from our own reader.
- The license text as the project published it is in `LICENSE-<id>.txt` beside the board.
- The board is privacy-scanned before it is committed (paths, e-mail addresses, computer names:
  the checks `privacy-check` runs, repeated on the KiCad text by a test in `crates/dutchman-electronics`).

## Fuzz the readers

A bad file must never crash the program, so GitHub also feeds the readers random and mangled input ([cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html), in the `fuzz/` folder). The `Fuzz` workflow runs every night and on request (Actions, Fuzz, Run workflow); it keeps any crashing input as an artifact. To run one target yourself (needs a nightly Rust and `cargo install cargo-fuzz`):

```sh
fuzz/seed.sh                              # copies our test boards in as starting points
cd fuzz
cargo +nightly fuzz run kicad_pcb -- -max_total_time=600 -timeout=10
```

The targets are `altium_pcbdoc`, `altium_project`, `kicad_pcb`, `package_build` (read, STEP and every overlay) and `truetype_text`. A crash lands in `fuzz/artifacts/<target>/`. Fix it, and add the (shrunk) input as a test fixture with a test that reads it: `cargo fuzz tmin <target> <file>` shrinks it.

## Make a change, step by step

1. Pick one roadmap step in the README, or an open issue. Comment "I'll take this" so nobody doubles up.
2. Make a branch: `git switch -c outline`.
3. Tell your AI: read AGENTS.md and the roadmap step, here is my test board and its README.txt, make the next step work with a test that checks the numbers.
4. Run `cargo test` and `cargo clippy --all-targets -- -D warnings`.
5. Push, open a pull request, say in the first sentence what changed in plain words.

Keep one topic per pull request. Small is good.

## Rules that matter

- **Clean-room.** We learn the format only from public information and from files you made. Read [LEGAL.md](LEGAL.md) before working on a reader: it lists the allowed and forbidden sources (no Altium SDK or scripting docs for reader code, no decompiled programs, no NDA material, no GPL code such as KiCad's Altium importer). A pull request that changes a reader carries the provenance sentence from LEGAL.md (a box in the pull-request template; CI checks it).
- **A bad file must never crash the program.** It returns an error with a clear message.
- **Every change has a test.**
- **English** for code, comments and messages.

## Getting stuck

Ask in the dev channel or open an issue with: what you tried, the command you ran, and what you saw. Pasting the exact error text helps more than describing it. There are no silly questions here.
