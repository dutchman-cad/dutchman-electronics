# kicad-pcb

Reads KiCad `.kicad_pcb` boards (KiCad 9 and 10) into the neutral board model in `board-model`.
No Dutchman dependencies; the only dependency is `board-model`.

An older file (before KiCad 9, file version 20241229) is not guessed at: the reader returns an
empty board with a `Dropped` note that says to open and save the file in KiCad 9 or later.

## What it reads

| Board model | From the file |
|---|---|
| outline and cutouts | `gr_line`, `gr_arc`, `gr_circle`, `gr_rect`, `gr_poly` on `Edge.Cuts` (also inside footprints), joined end to end; the biggest closed contour is the outline, the rest are cutouts |
| thickness | the stack-up (`setup` > `stackup`): copper, dielectric and solder mask layers added up; else `general` > `thickness` |
| solder mask opening | `setup` > `pad_to_mask_clearance` |
| layers | the `layers` table, in stack order (top silkscreen, paste, mask, copper top to bottom, bottom mask, paste, silkscreen, then the rest), with kind and side |
| placements | `footprint`: reference, library name, `at`, side from `layer`, first `model` (file, offset, rotate) |
| copper | `segment`, `arc`, `via`, `pad` (circle, rect, oval; roundrect, trapezoid and custom read as rectangles with a note; drills, plating, copper layers), `filled_polygon` of zones |
| silkscreen | `gr_*` and `fp_*` lines, arcs, circles, rectangles, polygons; `gr_text`, `fp_text` and the reference and value `property` texts |

Everything else is a note: keep-out zones, zones without a saved fill, dimensions, text boxes,
drawings on layers other than silkscreen, copper and `Edge.Cuts`, extra model references, model
scales other than 1, filled silkscreen shapes (drawn as outlines), and so on. External 3D model
files are named in an `Info` note, like Altium's external models.

Texts are anchored by their middle in KiCad and at the left end of their baseline in the model.
Without glyph widths the reader estimates the shift (0.6 of the height per letter) and says so in
a note.

## What is not exact

- Cutouts are told from the outline by their bounding boxes (an arc counts as its whole
  circle), so a contour that sticks out of the biggest one's box is dropped with a note, and a
  cutout is not tested to lie inside the outline's shape.
- A full circle is one arc with equal start and end angles, as the model defines it.
- Outline pieces are joined within 5 micrometres (our choice: KiCad gives an arc by three
  rounded points, which leave its ends about a micrometre off). Only the first 5000 loose
  Edge.Cuts pieces are joined; the rest are dropped with a note.
- Numbers beyond 100 m for a position or 10 m for a size make the item holding them unreadable
  (dropped with a note). A file may hold at most 4 million nodes (about 100 bytes each once
  read).
- Text widths (0.6 of the height per letter, used to place centred and right-aligned text) and
  the pen width of a text that sets none (0.12 of its height, from KiCad's 0.15 mm pen on a
  1.27 mm text) are our own estimates, not KiCad's metrics.
- A slotted hole is read as a round hole as wide as the slot's narrow side; a hole set off-centre
  in its pad is drawn centred (a warning).
- A zone's fill is the polygon KiCad saved, with its zero-width bridges to the holes.

## Frames

KiCad has Y pointing down; the model has Y up, so the reader negates every Y. A footprint's
parts are turned counter-clockwise on the board by the footprint's angle; a pad's angle in the
file already includes the footprint's. A bottom-side footprint is saved with its parts already in
board orientation. A model's offset is passed on as written (x right, y up, z out of the face the
part sits on); its rotation is negated, because KiCad turns a model the other way about each
axis. We measured both with KiCad's own export (`fixtures/kicad/model_axes`).

## Sources (LEGAL.md)

Derived only from KiCad's published file-format documentation and from real board files:

- https://dev-docs.kicad.org/en/file-formats/sexpr-intro/ (syntax, common tokens, footprints,
  pads, graphic items, zones)
- https://dev-docs.kicad.org/en/file-formats/sexpr-pcb/ (board file: header, layers, setup,
  stack-up, nets, segments, vias, arcs)
- https://dev-docs.kicad.org/en/file-formats/sexpr-footprint/ (footprint header)

and from the boards in `tests/boards`, which we made with KiCad 9.0.9 (the `kicad/kicad:9.0`
container image, a program binary) and our own scripts in `fixtures/kicad`. KiCad's source code
was not read. Where the documentation is silent (angles of pads and texts inside a rotated
footprint, the `model` token's `offset`, the order and sign of model rotations, the `*.Cu` pad
layer) the behaviour comes from those boards and from KiCad's own exports of them.

## Tests

`tests/boards/*.kicad_pcb` are CC0 boards, one feature each, with every number in the board's
`README.txt`; the `k10-` boards are the same boards made with KiCad 10.0.8 and must read the
same. `model-placements` pins where a model lands on a top and a bottom part against KiCad's own
VRML export. `fixtures/kicad/run.sh` rebuilds them (needs podman). `tests/hand_written.rs`
covers what no KiCad-made board holds yet from text written by hand from the documentation: the
stack-up table, older and newer version numbers and net names written as strings. Real
KiCad-made boards with a saved stack-up, and larger open-hardware boards, are still wanted (see
issue 75).
