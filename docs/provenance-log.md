# Provenance log

A dated record of anything that came near a forbidden source (see LEGAL.md), what was seen, and what was done about it. This keeps the clean-room record complete. Add a line whenever it happens, even when nothing was used.

| Date | What happened | What was seen | Outcome |
|---|---|---|---|
| 2026-11-24 | While choosing a stroke font for silkscreen text, our AI assistant downloaded KiCad's `common/newstroke_font.cpp` from KiCad's source repository to check its license. | Only the file's license header (its first 40 lines): GPL-2.0-or-later. No font data or code was read. | Rejected as GPL and deleted at once. Nothing from it is in this repository. Text uses the Hershey fonts instead. No reader work was in progress, and KiCad's source tree is now blocked in AGENTS.md. |
| 2026-11-25 | Building the KiCad reader. We used only the published File Formats pages at dev-docs.kicad.org (sexpr-intro, sexpr-pcb, sexpr-footprint) and KiCad 9.0.9 as a program (the `kicad/kicad:9.0` container image) to make test boards and to export a box model to VRML, which measured how KiCad places a model. | Documentation pages and KiCad output files only. No KiCad source code, license header or mirror was fetched or read. | Nothing to remove. The sources are named in `crates/kicad-pcb/README.md`. |
