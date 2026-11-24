# Provenance log

A dated record of anything that came near a forbidden source (see LEGAL.md), what was seen, and what was done about it. This keeps the clean-room record complete. Add a line whenever it happens, even when nothing was used.

| Date | What happened | What was seen | Outcome |
|---|---|---|---|
| 2026-11-24 | While choosing a stroke font for silkscreen text, our AI assistant downloaded KiCad's `common/newstroke_font.cpp` from KiCad's source repository to check its licence. | Only the file's licence header (its first 40 lines): GPL-2.0-or-later. No font data or code was read. | Rejected as GPL and deleted at once. Nothing from it is in this repository. Text uses the Hershey fonts instead. No reader work was in progress, and KiCad's source tree is now blocked in AGENTS.md. |
