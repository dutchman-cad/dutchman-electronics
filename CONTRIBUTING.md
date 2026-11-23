# Contributing to Dutchman Electronics

You design boards. You do not have to be a programmer. This project is built to be worked on by a hardware engineer together with an AI assistant (Claude or similar), and everything here is written for that.

## The idea in one minute

An Altium `.PcbDoc` file is a box with labelled drawers inside (a "container"). Each drawer holds one kind of data: board settings, tracks, components, text. We:

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

Write the exact numbers down in a `README.txt` beside each file (where you placed things, in millimetres), and the Altium version that saved it (e.g. `Altium Designer 26.12.2`). Those numbers are the "right answer" the tests check against; the version tells us which file layout a board uses.

Then put each board in `crates/altium-pcbdoc/tests/boards/` with its `README.txt` (either `README.txt` in the board's own folder, or `<board name>.README.txt` beside it), and this line at the top of that file:

```
Original test board made for Dutchman Electronics. Released under CC0 1.0 (public domain): https://creativecommons.org/publicdomain/zero/1.0/
```

CC0 means anyone may use it for anything with no conditions. Do not add a board without that line; a test refuses boards that lack it. Do not add 3D models you did not make or that you downloaded; use a simple box you modelled yourself.

### Check a board for personal traces before sharing it

A board file can carry more than the board: the folder it was saved in, a user or computer name,
a company or licence name. Before a board leaves your computer (a pull request, a zip, a chat), run:

```sh
cargo run -- privacy-check path/to/board.PcbDoc --watch "Your Name" --watch "Your Company" --watch "your-login"
```

Add a `--watch` for anything that would point to you or your employer: your name, your Windows
login, your computer's name, your company, your licence or serial number. It searches every part
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
