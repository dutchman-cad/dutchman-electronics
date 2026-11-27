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
| copper | `segment`, `arc`, `via`, `pad` (circle, rect, oval, roundrect as a rounded rectangle; trapezoid, chamfered and custom pads as polygon pads; drills, plating, copper layers), `filled_polygon` of zones, split into an outer contour and its holes |
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
- Outline pieces are joined within 5 micrometers (our choice: KiCad gives an arc by three
  rounded points, which leave its ends about a micrometer off). Only the first 5000 loose
  Edge.Cuts pieces are joined; the rest are dropped with a note.
- Numbers beyond 100 m for a position or 10 m for a size make the item holding them unreadable
  (dropped with a note). A file may hold at most 4 million nodes (about 100 bytes each once
  read).
- Text widths (0.6 of the height per letter, used to place centered and right-aligned text) and
  the pen width of a text that sets none (0.12 of its height, from KiCad's 0.15 mm pen on a
  1.27 mm text) are our own estimates, not KiCad's metrics.
- A slotted hole is read as a round hole as wide as the slot's narrow side; a hole set off-center
  in its pad is drawn centered (a warning).
- A zone's fill is one ring in the file, with a zero-width bridge out to each hole. The reader cuts
  the ring into simple loops wherever it passes a vertex twice (the bridges collapse to nothing),
  calls the loops that wind like the biggest one copper and the others holes, and puts each hole in
  the smallest piece that holds most of its corners. A fill with arcs, one that splits into more than
  4096 loops or has more than 400000 corners, or one whose holes cost too much to assign, is kept
  as the single contour KiCad wrote, with a warning; a hole inside none of the pieces is left out
  with a warning.
- Custom pads: the anchor (rectangle of the pad's size, or circle of its width) plus filled
  `gr_poly`, `gr_rect` and `gr_circle` primitives, and `gr_line`, `gr_arc` and unfilled polygons,
  rectangles and circles drawn with their width as round-ended strokes (an unfilled circle is a ring
  of that width). An arc inside a `gr_poly`'s points and a `gr_arc` stroke are kept as exact arcs in
  the pad's contour, so the outline tessellates them as it does every other arc. Curves (`gr_curve`,
  `bezier`) and strokes without width are left out with a note; a stroke width on a filled primitive
  is not added to it (a warning). A pad with a chamfer keeps the rounding of its other corners
  (`roundrect_rratio`), as KiCad's own Gerber export shows. Corners of trapezoid, chamfered and custom
  pads are checked against that export (`tests/gerber`) for pads turned, in turned footprints and on
  the bottom; the arcs of the `pad-arcs` board are checked against it too.
- Fonts: a text set in a named font (`face`) is drawn with that font's own outlines when the board
  embeds the font (`embedded_files`, KiCad 9 and later): the data is base64 of a zstd-packed font
  file, unpacked within a size cap and handed over as `ReadOutcome::fonts`. The font's family, style
  and bold and italic marks come from the font file's own tables. A text whose font the board does
  not embed keeps the stroke font, with a warning naming the font. The checksum KiCad writes beside
  the data is not checked. Where a text sits still uses our width estimate.

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
layer) the behavior comes from those boards and from KiCad's own exports of them.

## Tests

`tests/boards/*.kicad_pcb` are CC0 boards, one feature each, with every number in the board's
`README.txt`; the `k10-` boards are the same boards made with KiCad 10.0.8 and must read the
same. `model-placements` pins where a model lands on a top and a bottom part against KiCad's own
VRML export. `fixtures/kicad/run.sh` rebuilds them (needs podman). `pad-shapes`, `pad-arcs`, `embedded-font`, `zone-thermal`
and `four-layer-stackup` cover the pad shapes, a pour with holes and the stack-up table (KiCad
cannot be scripted to write one, so the table was pasted into a saved board and KiCad rewrote the
file; see that board's README.txt). `tests/hand_written.rs` covers what no KiCad-made board holds
yet from text written by hand from the documentation: stack-ups that disagree with the general
thickness, older and newer version numbers, net names written as strings and fills that pinch.

`tests/open-hardware/<board>/` holds larger real-world boards made by others under an open
Creative Commons license (CC0, CC-BY 4.0 or CC-BY-SA 4.0), each with its license text and a
`README.txt` naming the source, commit, attribution and the numbers KiCad itself gives for the
board; `tests/open_hardware.rs` checks the reader against those numbers. The rules are in
CONTRIBUTING.md.
