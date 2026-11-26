Original test board made for Dutchman Electronics. Released under CC0 1.0 (public domain): https://creativecommons.org/publicdomain/zero/1.0/
Exception: the TrueType font files embedded in one-dejavu-text, one-dejavu-bold-text and one-dejavu-italic-text are not CC0; they keep the DejaVu / Bitstream Vera license (LICENSE-DejaVu.txt). Everything else in those three boards is CC0.

(The first line covers every board in this folder: empty-outline, outline-with-hole, one-track,
one-resistor, one-via, one-plated-pad, one-rotated-pad, one-oval-pad, one-bottom-track,
one-arc, one-fill, one-region, one-pour, one-text, one-rotated-text,
one-bottom-text, one-step-resistor, one-rotated-resistor, one-bottom-resistor, one-net,
one-offset-step-resistor, one-rotated-step-resistor, one-bottom-step-resistor and
one-rotated-fill.)

Made in Altium Designer 26.12.2 (build 26.12.2.9), saved as "PCB Binary Files (*.PcbDoc)".
The first four boards (empty-outline, outline-with-hole, one-track, one-resistor) were drawn by
hand on 2026-11-23 with typed or grid-snapped coordinates (snap grid 1 mm); the other six were
made by a script on 2026-11-24 (see "Scripted boards" below). Every number below is exact.

All numbers are in millimeters, as Altium shows them in its mm display. Coordinates are relative
to the origin marker Altium puts on a new PCB; the board's lower-left corner sits on that origin.
X to the right, Y up.

Layer stack (the same on all boards; total 1.600 mm):

    Top Overlay      (no thickness)
    Top Solder       solder resist   0.010
    Top Layer        copper          0.035   (Altium still labels it "1oz")
    Dielectric 1     FR-4            1.510   (Dk 4.8)
    Bottom Layer     copper          0.035   (Altium still labels it "1oz")
    Bottom Solder    solder resist   0.010
    Bottom Overlay   (no thickness)

    Copper + dielectric only: 1.580. With both solder masks: 1.600.


empty-outline.PcbDoc
--------------------
Board shape: rectangle 50.000 x 30.000.
    Corners (0, 0), (50, 0), (50, 30), (0, 30).
Nothing else: no tracks, pads, vias, text, components or cutouts.
(The outline was first drawn as four lines on Mechanical 1, turned into the board shape with
Design > Board Shape > Define Board Shape from Selected Objects, and then the lines were deleted.)


outline-with-hole.PcbDoc
------------------------
Same board shape and stack as empty-outline, plus:

1. One unplated hole, made as a free pad:
       center (10.000, 15.000), rotation 0
       hole: round, diameter 3.000, not plated
       pad: round, 3.000 x 3.000 on all layers (same as the hole, so no copper ring)
       layer Multi-Layer, designator "1", no net (Altium names its template c300hn300)
2. One rectangular board cutout:
       corners (30, 10), (40, 10), (40, 20), (30, 20): 10.000 x 10.000
       made from four Mechanical 1 lines with Tools > Convert > Create Board Cutout from
       Selected Primitives; the lines were then deleted.


one-track.PcbDoc
----------------
Same board shape and stack as empty-outline (no hole, no cutout), plus one track:
    layer Top Layer, no net
    start (10.000, 15.000), end (20.000, 15.000): 10.000 long, straight along +X
    width 0.500
Altium's default design rule allows a track width of at most 0.254, so Altium marks this
track as a rule violation (shown green). That is expected and does not change the file's numbers.


one-resistor.PcbDoc
-------------------
Same board shape and stack as empty-outline (a file copy of it), plus one component:
    designator R1, comment "10k", footprint R0603-TEST
    center (25.000, 15.000), rotation 0, on the top side
    no nets, no tracks

Footprint R0603-TEST (I drew it myself; numbers relative to the footprint origin, which is the
component center):
    pad 1: Top Layer only (SMD), rectangle 0.800 x 1.000, center (-0.800, 0)
    pad 2: Top Layer only (SMD), rectangle 0.800 x 1.000, center (+0.800, 0)
    3D body: an extruded box I drew myself (no STEP file, nothing downloaded),
        outline corners (-0.8, -0.4), (0.8, -0.4), (0.8, 0.4), (-0.8, 0.4): 1.600 x 0.800
        standoff 0, overall height 0.450 (from the board's top surface up)
        Altium layer "Top 3D Body"
So on the board the pads are centered at (24.2, 15) and (25.8, 15), and the box spans
x 24.2 to 25.8, y 14.6 to 15.4, z 0 to 0.45 above the top surface.

The "R1" designator text is on Top Overlay where Altium put it by default; its position is not
one of the numbers above. The footprint itself is stored inside the board file.


Scripted boards: one-via, one-plated-pad, one-rotated-pad, one-oval-pad, one-bottom-track, one-arc
------------------------------------------------------------------------------------------------
Made on 2026-11-24 in the same Altium Designer 26.12.2, by a DelphiScript of my own run inside
Altium (kept outside this repository). Each board started as a file copy of empty-outline.PcbDoc,
so it has the same board shape and stack, and the script added exactly one object with the
numbers below, then saved. The script read every number back from Altium and logged it; the log
matches the numbers here. None of these objects has a net or belongs to a component.

one-via.PcbDoc
    one via, center (25.000, 15.000), diameter 1.000, hole 0.500
    from Top Layer to Bottom Layer (a through via)

one-plated-pad.PcbDoc
    one free pad, layer Multi-Layer, designator "1", rotation 0
    center (25.000, 15.000), round, 2.000 x 2.000 on top, middle and bottom
    hole: round, diameter 1.000, plated

one-rotated-pad.PcbDoc
    one free pad, layer Top Layer only (SMD), designator "1", no hole
    center (25.000, 15.000), rectangle 2.000 x 1.000 (X x Y before rotation)
    rotation 30 degrees (counter-clockwise)

one-oval-pad.PcbDoc
    one free pad, layer Top Layer only (SMD), designator "1", no hole, rotation 0
    center (25.000, 15.000), Altium shape "Round" with 2.000 x 1.000, which Altium draws as an
    oval (a slot shape, 2.000 long along X, 1.000 wide)

one-bottom-track.PcbDoc
    one track, layer Bottom Layer
    start (10.000, 15.000), end (20.000, 15.000): 10.000 long, straight along +X
    width 0.500

one-arc.PcbDoc
    one arc (a curved track), layer Top Layer
    center (25.000, 15.000), radius 5.000, width 0.500
    from 0 degrees to 90 degrees, counter-clockwise: it starts at (30, 15) and ends at (25, 20)


Scripted boards, second batch: one-fill, one-region, one-pour
--------------------------------------------------------------
Made on 2026-11-24 the same way as the first scripted batch (a file copy of empty-outline plus
exactly one object, added by my own script, every number read back and logged). None of these
objects has a net.

one-fill.PcbDoc
    one fill (a filled rectangle), layer Top Layer, rotation 0
    corners (20.000, 10.000) and (30.000, 20.000): 10.000 x 10.000
one-region.PcbDoc
    one solid copper region, layer Top Layer
    corners (20.000, 10.000), (30.000, 10.000), (30.000, 20.000), (20.000, 20.000)
one-pour.PcbDoc
    one polygon pour, layer Top Layer, solid (not hatched), no net, "remove dead copper" off
    pour outline (the shape I drew): corners (5.000, 5.000), (45.000, 5.000), (45.000, 25.000),
    (5.000, 25.000)
    Altium filled it with copper when the script rebuilt the pour; the poured copper is stored
    in the file as well. Its exact edge comes from Altium's own clearance rules, so it is not
    one of the numbers above.


Scripted boards, third batch: one-text, one-rotated-text, one-bottom-text
--------------------------------------------------------------------------
Made on 2026-11-24 the same way as the other scripted boards (a file copy of empty-outline plus
exactly one object, added by my own script, every number read back and logged). Each is a text
in Altium's stroke font (not a TrueType font), height 2.000, stroke width 0.200, with no
component and no net. The position is the text's reference point as Altium shows it in the
text's properties (X/Y), before rotation and mirroring.

one-text.PcbDoc
    text "TEST", layer Top Overlay, position (10.000, 10.000), rotation 0, not mirrored
one-rotated-text.PcbDoc
    text "ROT", layer Top Overlay, position (25.000, 15.000), rotation 90 degrees
    (counter-clockwise), not mirrored
one-bottom-text.PcbDoc
    text "BOT", layer Bottom Overlay, position (40.000, 10.000), rotation 0, mirrored (so it
    reads correctly when the board is seen from below)

The bounding rectangle Altium reports for each text (read back on 2026-11-25 by my own script,
which opened a copy of each board and closed it without saving; Altium gives more digits, these
are rounded to 0.001):
    one-text          x  9.800 to 17.531, y  9.800 to 12.199
    one-rotated-text  x 22.801 to 25.200, y 14.800 to 20.532
    one-bottom-text   x 34.468 to 40.200, y  9.800 to 12.199
Unrounded, as Altium gives them: one-text 9.8, 9.8 to 17.53097, 12.19936; one-rotated-text
22.80065, 14.8 to 25.2, 20.53162; one-bottom-text 34.46838, 9.8 to 40.2, 12.19936. Seen in each
text's own direction, the rectangle reaches 0.200 (the stroke width) behind the reference point
and 0.200 below it: the rotated text runs up (+y) from its reference point with its top towards
-x, and the mirrored text runs to the left (-x).

Scripted board with a STEP model: one-step-resistor
----------------------------------------------------
Made on 2026-11-24 by my own script, from a file copy of one-resistor.PcbDoc: R1 keeps its
place (25.000, 15.000), rotation 0, top side, footprint R0603-TEST and its pads. Its extruded
box was removed and replaced by a 3D body that holds a STEP model, embedded in the board file.
The STEP model (box-1608.step) is my own: a box 1.600 x 0.800 x 0.450, centered on its origin in
X and Y, from z = 0 to z = 0.450, written by this repository's board-step crate (CC0 like the
board). Altium was given no rotation and no standoff; the script read back the body's extent:
x 24.2 to 25.8, y 14.6 to 15.4, height 0.450 above the top surface.

Scripted boards, fourth batch: one-rotated-resistor, one-bottom-resistor, one-net
----------------------------------------------------------------------------------
Made on 2026-11-24 by my own script, each from a file copy of a board above with exactly one
change; the script read every number back and logged it.

one-rotated-resistor.PcbDoc  (from one-resistor)
    R1 rotated to 90 degrees (counter-clockwise) about its center, which stays at (25, 15);
    still on the top side. Its pads moved with it:
        pad 1 at (25.000, 14.200), pad 2 at (25.000, 15.800), both with rotation 90
    Its extruded box turned with it: x 24.6 to 25.4, y 14.2 to 15.8, still 0.450 high.
one-bottom-resistor.PcbDoc  (from one-resistor)
    R1 flipped to the bottom side, center still (25, 15). Altium itself then shows R1 with
    rotation 180 (I set only the side). Its pads are on Bottom Layer and mirrored:
        pad 1 at (25.800, 15.000), pad 2 at (24.200, 15.000), both with rotation 180
    Its extruded box went to the bottom side too (Altium marks it as projecting from the bottom):
    x 24.2 to 25.8, y 14.6 to 15.4, standoff 0, height 0.450.
one-net.PcbDoc  (from one-track)
    the track (Top Layer, (10, 15) to (20, 15), width 0.500) now belongs to a net named "GND";
    nothing else has a net.

Scripted boards, fifth batch: an off-center STEP model, rotated and flipped; a rotated fill;
TrueType text
-------------------------------------------------------------------------------------------
Made on 2026-11-24 by my own script; every number was read back and logged.

The STEP model box-offset.step is my own, written by this repository's board-step crate (CC0
like the boards): a box 1.600 x 0.800 x 0.450 that lies on the +X side of its origin, x 0 to
1.6, y -0.4 to 0.4, z 0 to 0.45. Being off-center, it shows how Altium turns and flips models.

one-offset-step-resistor.PcbDoc  (from one-resistor)
    R1's extruded box replaced by a 3D body holding box-offset.step, its origin on R1's center
    (25, 15), embedded in the board. Top side, rotation 0. The body spans x 25 to 26.6,
    y 14.6 to 15.4, height 0.450.
one-rotated-step-resistor.PcbDoc  (the same, then R1 rotated to 90 degrees)
    R1 at (25, 15), rotation 90; pads at (25, 14.2) and (25, 15.8). The body spans x 24.6 to
    25.4, y 15 to 16.6: the +X box turned onto +Y.
one-bottom-step-resistor.PcbDoc  (the same as one-offset-step-resistor, then R1 flipped)
    Altium turns a part about the middle of everything it holds, model included, so flipping
    R1 to the bottom moved its center to (25.400, 15.000) and gave it rotation 180 (I set only
    the side). Pads on Bottom Layer at (26.2, 15) and (24.6, 15), rotation 180. The body spans
    x 23.8 to 25.4, y 14.6 to 15.4, height 0.450, projecting from the bottom.
one-rotated-fill.PcbDoc  (from empty-outline)
    the fill of one-fill (corners (20, 10) and (30, 20) before rotation) with rotation 30
    degrees, counter-clockwise about its center (25, 15); its bounding box is x 18.170 to
    31.830, y 8.170 to 21.830.

one-ttf-text.PcbDoc  (from empty-outline)
    text "TTF" in the TrueType font Arial (not the stroke font), layer Top Overlay, position
    (10.000, 10.000), height 2.000, rotation 0, not mirrored, no component and no net.

    Altium always embeds the TrueType font file it uses in the board (in EmbeddedFonts6), and
    Arial may not be shared under CC0. So after the last save I removed the font file: my own
    small tool copied every stream of the saved board into a new file unchanged, except
    EmbeddedFonts6, which it wrote the way a board without TrueType text has it (Header 4 zero
    bytes, Data empty). The board still names the font "Arial" in its text record. Altium
    Designer 26.12.2 opens it normally and draws the text with the Arial font installed on the
    PC.

Scripted boards, sixth batch: an off-center extruded body, top and flipped
--------------------------------------------------------------------------
Made on 2026-11-24 by my own script, each from a file copy of one-resistor.PcbDoc with R1's
extruded box given a new outline; every number was read back and logged. The outline is
lopsided in X and in Y, so a flip shows its mirror axis: about R1's center it is
(0, -0.4), (1.6, -0.4), (1.6, 0), (0, 0.4), from standoff 0 up to 0.450.

one-offset-extruded-resistor.PcbDoc
    R1 at (25, 15), rotation 0, top side. The body's outline is (26.6, 14.6), (26.6, 15),
    (25, 15.4), (25, 14.6); its box x 25 to 26.6, y 14.6 to 15.4, height 0.450.
one-bottom-extruded-resistor.PcbDoc  (the same, then R1 flipped to the bottom)
    Altium turns a part about the middle of everything it holds, so R1's center moved to
    (25.400, 15.000) and it shows rotation 180 (I set only the side). The body's outline is
    (23.8, 14.6), (23.8, 15), (25.4, 15.4), (25.4, 14.6), projecting from the bottom: the +X
    lobe went to -X, the slanted edge kept its Y.

Scripted boards, seventh batch: two parts with one shared or two different STEP models
--------------------------------------------------------------------------------------
Made on 2026-11-24 by my own script, each from a file copy of one-step-resistor.PcbDoc (R1 at
(25, 15) with the embedded box-1608.step); every number was read back and logged. The script
copied R1 with its two pads as R2, 10 mm to the right: R2 at (35, 15), rotation 0, top side,
footprint R0603-TEST, pads at (34.2, 15) and (35.8, 15). Both STEP models are my own, written
by this repository's board-step crate (see the fifth batch).

shared-step-model.PcbDoc
    R2 got a copy of R1's 3D body: the same embedded box-1608.step, centered on R2 (body x 34.2
    to 35.8, y 14.6 to 15.4, height 0.450). The board holds the model once: one record in
    Models/Data and one stream Models/0, and both bodies name the same MODELID.
two-step-models.PcbDoc
    R2 got a new 3D body holding the embedded box-offset.step instead, its origin on R2's
    center (body x 35 to 36.6, y 14.6 to 15.4, height 0.450). R1 keeps box-1608.step. The
    board holds two models: record 0 of Models/Data is box-offset.step, stored in Models/0 (x 0
    to 1.6 once unpacked), and record 1 is box-1608.step, stored in Models/1 (x -0.8 to 0.8).
    So the n-th record (from 0) is stored in Models/n; Altium put the newer model first.

Scripted boards, eighth batch: a STEP model turned on its part; bottom-side parts at 90
---------------------------------------------------------------------------------------
Made on 2026-11-24 by my own script; every number was read back and logged.

The first three are made from one-resistor. R1's extruded box was replaced by a 3D body holding
the embedded box-offset.step (fifth batch), its origin first put 0.7 above R1's center. Then the
model was turned 30 degrees on the part: R1 was turned to 30 degrees with its body, the body
was taken off R1, R1 was turned back to 0 and the body put back. On the part the model's origin
is then at (-0.350, 0.606) from R1's center (the point (0, 0.7) turned by 30 degrees), and the
model is turned 30 degrees counter-clockwise.

one-turned-step-resistor.PcbDoc
    R1 at (25, 15), rotation 0, top side. The model's origin (MODEL.2D) is at
    (24.650, 15.606), MODEL.3D.ROTZ 30. The body spans x 24.45 to 26.236, y 15.260 to 16.753.
one-bottom-turned-step-resistor.PcbDoc  (the same, then R1 flipped to the bottom)
    Altium turned R1 about the middle of everything it holds: R1 at (25.036, 15), rotation
    180, bottom side. Model origin (25.386, 15.606), ROTZ 210. The body spans x 23.8 to
    25.586, y 15.260 to 16.753.
one-bottom-rotated-turned-step-resistor.PcbDoc  (the same, flipped, then R1 turned to 90)
    R1 at (23.747, 15), rotation 90, bottom side. Model origin (24.354, 14.650), ROTZ 300. The
    body spans x 24.007 to 25.5, y 14.45 to 16.236.
one-bottom-rotated-extruded-resistor.PcbDoc  (from one-offset-extruded-resistor)
    R1 with the lopsided extruded body of the sixth batch, flipped to the bottom (R1 then at
    (25.4, 15), rotation 180) and turned to 90: R1 at (25.4, 15), rotation 90, bottom side. The
    body's outline is (25, 16.6), (25.4, 16.6), (25.8, 15), (25, 15), height 0.450.

Scripted boards, ninth batch: lines and arcs on the overlay (silkscreen) layers
-------------------------------------------------------------------------------
Made on 2026-11-24 by my own script, each from a file copy of a board above with exactly one
object added; every number was read back and logged. No net on any of them.

one-overlay-line.PcbDoc  (from empty-outline)
    a line on Top Overlay from (10, 10) to (20, 10), width 0.200
one-overlay-arc.PcbDoc  (from empty-outline)
    an arc on Top Overlay, center (25, 15), radius 3.000, from 30 to 300 degrees
    (counter-clockwise), width 0.200
one-bottom-overlay-line.PcbDoc  (from empty-outline)
    a line on Bottom Overlay from (30, 20) to (40, 25), width 0.150
one-resistor-silkscreen.PcbDoc  (from one-resistor)
    R1 got its own silkscreen line, added to the component: Top Overlay, (24, 15.8) to
    (26, 15.8), width 0.150. It belongs to R1 the way its pads do.

Scripted boards, tenth batch: a STEP model tilted 90 degrees about X
---------------------------------------------------------------------
Made on 2026-11-24 by my own script, each from a file copy of one-resistor.PcbDoc: R1's extruded
box was replaced by a 3D body holding the embedded box-offset.step (fifth batch: x 0 to 1.6,
y -0.4 to 0.4, z 0 to 0.45 in the model), tilted 90 degrees about its X axis, so it lies on its
side. Every number was read back and logged.

one-tilted-step-resistor.PcbDoc
    R1 at (25, 15), rotation 0, top side. Model origin (MODEL.2D) at (25, 15.05),
    MODEL.3D.ROTX 90, ROTY 0, ROTZ 0, DZ 0. The body spans x 25 to 26.6, y 14.6 to 15.05, and
    reaches 0.4 above and below the top surface (STANDOFFHEIGHT -0.4, OVERALLHEIGHT 0.4).
one-bottom-tilted-step-resistor.PcbDoc  (the same, then R1 flipped to the bottom)
    R1 at (25.4, 15), rotation 180, bottom side (Altium turned it about the middle of everything
    it holds). Model origin (25.4, 15.05), ROTX 90, ROTZ 180, DZ 0. The body spans x 23.8 to
    25.4, y 14.6 to 15.05, 0.4 above and below the bottom surface.
one-turned-tilted-step-resistor.PcbDoc  (like the first, then the model turned 30 degrees on R1)
    The model was turned the same way as in the eighth batch (R1 turned to 30 with its body, the
    body taken off, R1 turned back to 0, the body put back). R1 at (25, 15), rotation 0, top
    side. Model origin (24.975, 15.043), ROTX 90, ROTZ 30. The body spans x 24.975 to 26.586,
    y 14.654 to 15.843, 0.4 above and below the top surface.

Eleventh board: an outline with arcs and a round cutout
-------------------------------------------------------
Made on 2026-11-24 from a file copy of empty-outline.PcbDoc, partly by my own script and partly
with two of Altium's own menu commands, since a script cannot reshape the board:

1. The script drew the new outline as helper lines and arcs (width 0.1) on Mechanical 1 and
   selected them, and a circle as two half arcs on Mechanical 2.
2. By hand: Design > Board Shape > Define Board Shape from Selected Objects.
3. The script selected the circle instead.
4. By hand: Tools > Convert > Create Board Cutout from Selected Primitives.
5. The script removed all helpers on Mechanical 1 and 2 and saved the board.

outline-with-arcs.PcbDoc
    The board shape is the 50 x 30 rectangle of empty-outline with every corner rounded,
    radius 3.000:
        straight edges  (3, 0) to (47, 0), (50, 3) to (50, 27), (47, 30) to (3, 30),
                        (0, 27) to (0, 3)
        corner arcs     center (47, 3) from 270 to 360 degrees, center (47, 27) from 0 to 90,
                        center (3, 27) from 90 to 180, center (3, 3) from 180 to 270
                        (counter-clockwise)
    One round board cutout: center (40, 15), radius 2.000.
    Nothing else is on the board; the layer stack and thickness are those of empty-outline.

Twelfth board: four copper layers
---------------------------------
Made on 2026-11-24 from a file copy of empty-outline.PcbDoc. By hand, in Design > Layer Stack
Manager: one "Insert layer below > Signal" on Top Layer. With the stack kept symmetric, Altium
added two signal layers and a thin dielectric next to each, all with its default names and
thicknesses, then the stack and board were saved. Then my own script added the copper below
and saved again. Saving put an embedded copy of the Arial font into the file although the board
has no text; it was removed the same way as for one-ttf-text (see above).

four-layer.PcbDoc
    Layer stack (total 1.812):

        Top Overlay      (no thickness)
        Top Solder       solder resist   0.010
        Top Layer        copper          0.035
        Dielectric 2                     0.071   (2.8 mil)
        Layer 1          copper          0.035   (Altium's "Mid Layer 1")
        Dielectric 1     FR-4            1.510
        Layer 2          copper          0.035   (Altium's "Mid Layer 2")
        Dielectric 3                     0.071   (2.8 mil)
        Bottom Layer     copper          0.035
        Bottom Solder    solder resist   0.010
        Bottom Overlay   (no thickness)

    a track on Layer 1 from (10, 10) to (20, 10), width 0.250
    a track on Layer 2 from (10, 20) to (20, 20), width 0.250
    a via at (30, 15), diameter 0.600, hole 0.300, from Top Layer to Bottom Layer

Scripted boards, thirteenth batch: a STEP model tilted 90 degrees about Y
--------------------------------------------------------------------------
Made on 2026-11-24 the same way as the tenth batch, each from a file copy of one-resistor.PcbDoc,
but with box-offset.step (x 0 to 1.6, y -0.4 to 0.4, z 0 to 0.45 in the model) tilted 90 degrees
about its Y axis, so it stands on end and reaches down into the board. Every number was read
back and logged.

one-y-tilted-step-resistor.PcbDoc
    R1 at (25, 15), rotation 0, top side. Model origin (MODEL.2D) at (25, 15.4),
    MODEL.3D.ROTX 0, ROTY 90, ROTZ 0, DZ 0. The body spans x 25 to 25.45, y 15 to 15.8, and
    reaches 1.6 into the board from the top surface (STANDOFFHEIGHT -1.6, OVERALLHEIGHT 0).
one-bottom-y-tilted-step-resistor.PcbDoc  (the same, then R1 flipped to the bottom)
    R1 at (25, 15), rotation 180, bottom side. Model origin (25, 15.4), ROTY 90, ROTZ 180,
    DZ 0. The body spans x 24.55 to 25, y 15 to 15.8, and reaches 1.6 into the board from the
    bottom surface.
one-turned-y-tilted-step-resistor.PcbDoc  (like the first, then the model turned 30 degrees on R1)
    The model was turned the same way as in the eighth batch. R1 at (25, 15), rotation 0, top
    side. Model origin (24.8, 15.346), ROTY 90, ROTZ 30. The body spans x 24.6 to 25.390,
    y 15 to 15.918, 1.6 into the board from the top surface.

Fourteenth board: an outline with a notch, concave and convex arcs
------------------------------------------------------------------
Made on 2026-11-24 from a file copy of empty-outline.PcbDoc, the same way as outline-with-arcs:
my own script drew the new outline as helper lines and arcs (width 0.1) on Mechanical 1 and
selected them; by hand, Design > Board Shape > Define Board Shape from Selected Objects; the
script then removed the helpers and saved.

outline-with-notch.PcbDoc
    The 50 x 30 rectangle of empty-outline with a 10 x 10 notch in its top edge, x 20 to 30,
    down to y 20. The notch's corners are rounded:
        at its mouth (convex)   center (19, 29), radius 1.000, from 0 to 90 degrees
                                center (31, 29), radius 1.000, from 90 to 180 degrees
        at its bottom (concave) center (22, 22), radius 2.000, from 180 to 270 degrees
                                center (28, 22), radius 2.000, from 270 to 360 degrees
                                (all counter-clockwise)
    Straight edges: (0, 0) to (50, 0), (50, 0) to (50, 30), (50, 30) to (31, 30),
    (30, 29) to (30, 22), (28, 20) to (22, 20), (20, 22) to (20, 29), (19, 30) to (0, 30) and
    (0, 30) to (0, 0). Nothing else is on the board.

Fifteenth board: a small board where the features meet
------------------------------------------------------
Made on 2026-11-24 by my own script from a file copy of one-resistor-silkscreen.PcbDoc. Every
number was read back and logged. Saving put an embedded copy of the Arial font into the file
although the board has no text; it was removed the same way as for one-ttf-text (see above).

small-board.PcbDoc
    Parts (R2 and R3 are copies of R1 with its pads, extruded body and silkscreen line):
        R1  center (25, 15), rotation 0, top side (as on one-resistor-silkscreen)
        R2  center (13, 22), rotation 0, top side
        R3  center (37, 8), copied to (37, 8) and then flipped: rotation 180, bottom side;
            its pads at (37.8, 8) (pad 1) and (36.2, 8) (pad 2), its silkscreen line on Bottom
            Overlay from (36, 8.8) to (38, 8.8)
    Nets:
        GND  R2 pad 1; a track on Top Layer (8, 22) to (12.2, 22), width 0.250; a via at
             (8, 22), diameter 0.600, hole 0.300, Top to Bottom; both pours
        SIG  R1 pad 2; a track on Top Layer (25.8, 15) to (30, 15), width 0.250; a via at
             (30, 15), diameter 0.600, hole 0.300, Top to Bottom; tracks on Bottom Layer
             (30, 15) to (30, 8) and (30, 8) to (35, 8), width 0.250
        N2   R1 pad 1 and R2 pad 2; tracks on Top Layer (13.8, 22) to (20, 22) and (20, 22) to
             (24.2, 15), width 0.250
        R3's pads belong to no net.
    Pours: one on Top Layer and one on Bottom Layer, both on net GND, solid, "remove dead
    copper" off, outline (2, 2), (48, 2), (48, 28), (2, 28). Altium cut the copper away around
    everything that is not GND (with its own default clearance) and joined the GND pad and via
    with thermal-relief spokes. The edges of those cuts come from Altium's rules, so they are not
    among the numbers here.
    Silkscreen (not owned by a part): a line on Top Overlay (5, 27) to (45, 27), width 0.200, and
    an arc on Top Overlay, center (40, 18), radius 3.000, from 0 to 270 degrees
    (counter-clockwise), width 0.200, both over the top pour; a line on Bottom Overlay
    (10, 4) to (40, 4), width 0.150, over the bottom pour.

Sixteenth batch: solder mask and silkscreen colors
---------------------------------------------------
Made on 2026-11-24 by hand, each from a file copy of one-resistor.PcbDoc: in the 3D view, the
View Configuration panel, one board color changed, then the board was saved. Altium keeps these
colors with the board in the 3DCONFIGURATION setting of /Board6/Data (they are how its own 3D
view draws the board). Colors below are red, green, blue from 0 to 255; nothing else changed.

    Altium's defaults (one-resistor and every board before this batch):
        solder mask, top and bottom   36, 120, 50   at opacity 0.6
        silkscreen, top and bottom    250, 250, 250 at opacity 1.0

one-blue-mask-resistor.PcbDoc
    solder mask, top and bottom: 17, 85, 204 (stored as 13391121), opacity 0.6 as before
one-yellow-silkscreen-resistor.PcbDoc
    silkscreen, top and bottom: 255, 255, 0 (stored as 65535), opacity 1.0 as before

Seventeenth batch: surface finish
--------------------------------
Made on 2026-11-24 by hand, each from a file copy of one-resistor.PcbDoc: in Design > Layer Stack
Manager, right-click Top Layer, Insert layer above, Surface Finish; with the stack kept
symmetric Altium added the same finish below Bottom Layer. The finish's Process was picked from
Altium's own list, which filled in its material, color and thickness; then the stack and the
board were saved. Altium shows the finish layers in the stack and counts them in its total
thickness; they are not among the stack's layers in the plain /Board6/Data keys.

one-enig-resistor.PcbDoc
    Top Surface Finish and Bottom Surface Finish: process ENIG, material "Nickel, Gold", color
    #FFFFC400 (opaque; red 255, green 196, blue 0), thickness 0.004. Altium's total thickness:
    1.608.
one-hasl-resistor.PcbDoc
    Top Surface Finish and Bottom Surface Finish: process HASL, material "PbSn", color
    #FFF2F2F2 (opaque; 242, 242, 242), thickness 0.020. Altium's total thickness: 1.640.

3D body color and opacity
--------------------------
Made on 2026-11-25 by my own script, from a file copy of one-resistor.PcbDoc: R1's extruded box
was given another color and opacity, read back and logged, and the board saved. Nothing else
was changed on purpose.

one-colored-body-resistor.PcbDoc
    R1's extruded body: color red 32, green 128, blue 224, opacity 0.5. Before the change it
    had Altium's default, red 128, green 128, blue 128, opacity 1, like every body on the boards
    above, extruded or STEP. The box itself is the one of one-resistor: 1.600 x 0.800, height
    0.450, centered on R1 at (25, 15).
    On saving, Altium gave the body a new random model ID and left out the footprint library
    field that was blanked on one-resistor (see "Paths inside the files").

Dense board: DUTCHMAN NODE 1
----------------------------
Made on 2026-11-25 by my own script, from a file copy of outline-with-arcs.PcbDoc, laid out like
a small real product. Every footprint is my own (numbers below), drawn by the script: no
library footprints and no downloaded models. The parts, nets and tracks come from a layout file
of my own, routed by my own small router; the script built the board from it, read every part,
pad, body and designator back from Altium and compared them with that file (all matched). Altium
Designer 26.12.2's design rule check reports no violations, with the board's own two rules
below.

dense-board.PcbDoc
    Outline: that of outline-with-arcs (50 x 30, corners rounded to radius 3.000) without its
    round cutout. Two layers, 1.600 thick, the stack and colors of empty-outline.
    Rules set on the board: track width 0.150 to 0.500 (0.200 preferred) on both copper layers,
    hole size up to 3.500. Everything else is Altium's default (clearance 0.254).
    Mounting holes: two free pads, round 5.000, plated hole 3.200, no net, at (4.5, 4.5) and
    (45.5, 25.5).

    Footprints (pads relative to the part's center, before rotation; all SMD pads rectangular on
    one copper layer, through-hole pads plated, hole 1.000):
        R0603-DN: 2 pads 0.800 x 1.000 at x -0.800 and +0.800 (resistor, 0603)
        C0603-DN: the same pads (capacitor, 0603)
        LED0603-DN: the same pads; pad 1 is the cathode (LED, 0603)
        R0402-DN: 2 pads 0.600 x 0.600 at x -0.500 and +0.500 (resistor, 0402)
        C0402-DN: the same pads (capacitor, 0402)
        SOT23-DN: pads 0.600 x 0.800: 1 at (-0.950, -1.000), 2 at (0.950, -1.000), 3 at (0, 1.000)
        SOIC8-DN: pads 1.500 x 0.600, pitch 1.270: 1 to 4 at x -2.700, y 1.905 down to -1.905; 5 to
            8 at x 2.700, y -1.905 up to 1.905
        TQFP32-DN: pads 1.450 x 0.500 (0.500 x 1.450 on the top and bottom rows), pitch 0.800,
            centers 4.200 from the middle: 1 to 8 down the left side from y 2.800, 9 to 16 along the
            bottom from x -2.800, 17 to 24 up the right side, 25 to 32 along the top from x 2.800
        HDR1X4-DN: 4 through-hole pads 1.700 (pad 1 square, the others round) at x 0, y 3.810 down
            to -3.810, pitch 2.540

    Parts (center, rotation and side as Altium shows them; a bottom part was placed on top and
    flipped, which Altium turns to 180). Body: an extruded box over the footprint, standoff 0,
    color and height as given, opacity 1; R3 and R4 instead each have the STEP model box-1608.step
    of one-step-resistor, embedded twice (the script gave each part the file on its own).
        designator  footprint   value    center            rotation  side    body
        J1          HDR1X4-DN   PWR/I2C  ( 3.500, 15.000)      0     top     RGB (20, 20, 20), height 2.500
        J2          HDR1X4-DN   ANALOG   (46.500, 15.000)      0     top     RGB (20, 20, 20), height 2.500
        U1          TQFP32-DN   MCU      (27.000, 15.000)      0     top     RGB (30, 30, 30), height 1.200
        U2          SOIC8-DN    EEPROM   (13.500,  7.000)      0     top     RGB (30, 30, 30), height 1.750
        U3          SOT23-DN    3V3 LDO  ( 9.000, 23.500)      0     top     RGB (30, 30, 30), height 1.100
        Q1          SOT23-DN    NPN      (20.000, 25.000)      0     top     RGB (30, 30, 30), height 1.100
        D1          LED0603-DN  GREEN    (33.000, 25.500)      0     top     RGB (250, 250, 235), height 0.600
        R1          R0603-DN    4k7      (17.500, 12.500)     90     top     RGB (40, 40, 40), height 0.450
        R2          R0603-DN    4k7      (19.500, 12.500)     90     top     RGB (40, 40, 40), height 0.450
        R3          R0603-DN    1k       (16.600, 23.600)      0     top     box-1608.step
        R4          R0603-DN    330      (36.600, 25.500)      0     top     box-1608.step
        R5          R0603-DN    10k      (24.000, 22.400)      0     top     RGB (40, 40, 40), height 0.450
        R6          R0603-DN    1k       (40.500, 20.600)      0     top     RGB (40, 40, 40), height 0.450
        R7          R0603-DN    1k       (40.500, 16.270)      0     top     RGB (40, 40, 40), height 0.450
        R8          R0603-DN    1k       (40.500, 11.400)      0     top     RGB (40, 40, 40), height 0.450
        R9          R0603-DN    100k     (13.000, 17.000)      0     top     RGB (40, 40, 40), height 0.450
        R10         R0603-DN    100k     (13.000, 14.000)      0     top     RGB (40, 40, 40), height 0.450
        C1          C0603-DN    10u      ( 5.500, 25.000)     90     top     RGB (190, 150, 100), height 0.800
        C2          C0603-DN    10u      (12.500, 25.500)      0     top     RGB (190, 150, 100), height 0.800
        C3          C0603-DN    100n     (21.000,  6.500)     90     top     RGB (190, 150, 100), height 0.800
        C4          C0603-DN    100n     (34.000,  9.000)      0     top     RGB (190, 150, 100), height 0.800
        C5          C0603-DN    1u       (34.000, 21.000)      0     top     RGB (190, 150, 100), height 0.800
        C6          C0603-DN    100n     ( 9.000,  3.500)      0     top     RGB (190, 150, 100), height 0.800
        C7          C0603-DN    100n     (27.600, 22.400)      0     top     RGB (190, 150, 100), height 0.800
        C8          C0402-DN    10n      (41.000, 23.000)    180     bottom  RGB (190, 150, 100), height 0.500
        C9          C0402-DN    10n      (41.000,  9.500)    180     bottom  RGB (190, 150, 100), height 0.500
        C10         C0402-DN    10n      (41.000,  6.500)    180     bottom  RGB (190, 150, 100), height 0.500
        C11         C0402-DN    100n     (24.500, 18.000)    180     bottom  RGB (190, 150, 100), height 0.500
        C12         C0402-DN    100n     (29.500, 12.000)    180     bottom  RGB (190, 150, 100), height 0.500
        C13         C0402-DN    100n     (29.500, 18.000)    180     bottom  RGB (190, 150, 100), height 0.500
        C14         C0402-DN    1u       (24.500, 12.000)    180     bottom  RGB (190, 150, 100), height 0.500
        C15         C0402-DN    100n     ( 9.000, 18.000)    180     bottom  RGB (190, 150, 100), height 0.500
        R11         R0402-DN    0        (16.000, 26.000)    180     bottom  RGB (40, 40, 40), height 0.350
        R12         R0402-DN    10k      (35.000, 15.000)    180     bottom  RGB (40, 40, 40), height 0.350

    Nets (pins as designator.pad):
        +3V3: U3.2 C2.1 U1.1 U1.7 U1.20 U1.28 R1.1 R2.1 R5.1 U2.8 C3.1 C4.1 C5.1 C11.1 C12.1 C13.1
            C14.1 R4.2 R11.1 C6.1
        AIN1: U1.17 R8.2 C9.1
        AIN2: U1.18 R7.2 C10.1 R12.1
        AIN3: U1.19 R6.2 C8.1
        GND: J1.4 J2.4 U3.1 Q1.2 C1.2 C2.2 C3.2 C4.2 C5.2 C6.2 C7.2 C8.2 C9.2 C10.2 C11.2 C12.2
            C13.2 C14.2 C15.2 R10.2 U1.2 U1.8 U1.21 U1.29 U2.1 U2.2 U2.3 U2.4 R12.2
        IN1: J2.3 R8.1
        IN2: J2.2 R7.1
        IN3: J2.1 R6.1
        LED_A: D1.2 R4.1
        LED_CTRL: U1.9 R3.1
        LED_K: Q1.3 D1.1
        NRST: U1.30 R5.2 C7.1
        Q1_B: R3.2 Q1.1
        SCL: J1.3 U1.4 U2.6 R2.2
        SDA: J1.2 U1.3 U2.5 R1.2
        VIN: J1.1 U3.3 C1.1 R9.1
        VSENSE: U1.6 R9.2 R10.1 C15.1
        WP: U2.7 R11.2

    Copper: 162 track segments with 45-degree bends (VIN 0.400 wide, +3V3 and the GND
    stubs 0.300, the signals 0.200) and 40 vias (diameter 0.600, hole 0.300, top to
    bottom): signal vias, GND stitching vias, a GND via under the middle of U1 that U1's GND pins
    and C11 to C14 are tracked to, and a GND via for R10 pad 2, which the pour cannot reach. A
    solid GND pour on each copper layer, outline (0.6, 3), (3, 0.6), (47, 0.6), (49.4, 3),
    (49.4, 27), (47, 29.4), (3, 29.4), (0.6, 27), "remove dead copper" off; Altium joins the GND
    pads and vias with thermal-relief spokes.
    Silkscreen: every designator in the stroke font, height 0.800, stroke 0.120, upright, top
    parts on Top Overlay and bottom parts on Bottom Overlay (mirrored); lines 0.150 wide beside
    each chip, outlines on the ICs, headers and LED, pin-1 dots; "DUTCHMAN NODE 1" on Top Overlay
    at (14, 1.2), height 1.200, stroke 0.180; "v1 CC0" on Bottom Overlay, mirrored, at (44, 2.6),
    height 0.800, stroke 0.120. Comments are hidden.
    Altium embedded Arial on saving although no text uses a TrueType font; it was removed the
    same way as for one-ttf-text (see above).

Embedded TrueType font: DejaVu Sans Mono
----------------------------------------
Made on 2026-11-25 by my own script, each from a file copy of empty-outline.PcbDoc plus one text,
every number read back and logged. Each text is in the TrueType font DejaVu Sans Mono (not the
stroke font), layer Top Overlay, position (10.000, 10.000), height 2.000, rotation 0, not
mirrored, no component and no net. The box is the text's bounding rectangle as Altium reports
it.

one-dejavu-text.PcbDoc
    text "DejaVu", not bold, not italic. Box x 10.000 to 15.936, y 9.643 to 11.305.
one-dejavu-bold-text.PcbDoc
    text "Bold", bold, not italic. Box x 10.000 to 13.905, y 9.977 to 11.305.
one-dejavu-italic-text.PcbDoc
    text "Italic", italic, not bold. Box x 10.000 to 16.075, y 9.977 to 11.313.

Altium embeds the font file each text uses in EmbeddedFonts6: one font on each board, with
full name, family and style "DejaVu Sans Mono" / "DejaVu Sans Mono" / "Book", "DejaVu Sans Mono
Bold" / "DejaVu Sans Mono" / "Bold" and "DejaVu Sans Mono Oblique" / "DejaVu Sans Mono" /
"Oblique". Unpacked, each is byte for byte the public DejaVu Sans Mono 2.37 file, with SHA-256:
    DejaVuSansMono.ttf          b4a6c3e4faab8773f4ff761d56451646409f29abedd68f05d38c2df667d3c582
    DejaVuSansMono-Bold.ttf     bce60f1b4421acd9ea51ba6623d7024ecbe6817a953e3654df62a5e6bdf8f769
    DejaVuSansMono-Oblique.ttf  742097840c541870e8d6dc5c9b37bb1ceeea6c0dedd1d475faf903ef9df734b0
No other font is embedded.

The embedded font files are not CC0. They keep their own license: the Bitstream Vera fonts
license, with the DejaVu changes in the public domain (the font names
http://dejavu.sourceforge.net/wiki/index.php/License). It allows the fonts to be copied and
passed on as long as the copyright and trademark notices and the license go with them; each font
file carries them in its own name table, which Altium keeps unchanged, and LICENSE-DejaVu.txt
in this folder holds the same text. Everything else in these three boards is CC0 like the
others.

special-strings: Altium's own special strings
---------------------------------------------
Made on 2026-11-25 by my own script, from a file copy of one-resistor.PcbDoc (R1, comment "10k",
comment hidden), with no project open. Each text is in the stroke font, height 1.500, stroke
0.150, rotation 0; read back and logged with the string Altium 26.12.2 shows for it and the box
Altium reports around that string. "Owner" R1 means the text was added to component R1 (it
belongs to R1, like a text drawn in its footprint); "-" is a free text.

    Stored text      Owner Layer           At        Mir. Altium shows    Box (x, y to x, y)
    .Designator      R1    Top Overlay     30, 25    no   R1              29.850, 24.850 to 32.149, 26.650
    .Comment         R1    Top Overlay     30, 21.5  no   10k             29.850, 21.350 to 33.649, 23.150
    R: .Designator   R1    Top Overlay     30, 18    no   R: R1           29.850, 17.850 to 36.398, 19.650
    .designator      R1    Top Overlay     30, 8.5   no   R1              29.850, 8.350 to 32.149, 10.150
    .Layer_Name      R1    Top Overlay     30, 5     no   Top Overlay     29.850, 4.350 to 45.895, 6.650
    .Designator      -     Top Overlay     2, 25     no   .Designator     1.850, 24.350 to 17.145, 26.650
    .Comment         -     Top Overlay     2, 21.5   no   .Comment        1.850, 21.350 to 12.897, 23.150
    .Layer_Name      -     Top Layer       2, 18     no   Top Layer       1.850, 17.350 to 14.896, 19.650
    L: .Layer_Name   -     Top Overlay     2, 14.5   no   L: Top Overlay  1.850, 13.850 to 22.144, 16.150
    .layer_name      -     Top Overlay     2, 11     no   Top Overlay     1.850, 10.350 to 17.895, 12.650
    .Print_Date      -     Top Overlay     2, 7.5    no   the day's date  1.850, 7.350 to 15.646, 9.150
    .NoSuchString    -     Top Overlay     2, 4      no   .NoSuchString   1.850, 3.350 to 20.394, 5.650
    .Layer_Name      -     Bottom Overlay  48, 1.5   yes  Bottom Overlay  28.106, 0.850 to 48.150, 3.150

So .Designator and .Comment show the designator and comment of the component the text belongs to,
and stay as written on a free text; .Layer_Name shows the name of the text's own layer.
.Print_Date showed the date the script ran (25.11.2026), so it changes from day to day; the reader
leaves it as written. They follow the same rule as project parameters (see
param-project/README.txt): "R: .Designator" and "L: .Layer_Name" keep what comes before the dot,
and the name matches in any case. R1's own designator text "R1" is on the board as before.

Paths inside the files
----------------------
Altium writes a few paths from the PC that saved a board into the file. After the last save, I
blanked their values: each value is overwritten with spaces of exactly the same length, so no
record or stream size changed and no other byte in the stream moved (one-ttf-text,
four-layer and small-board also lost their embedded font, see above). The fields are:
    Board6 FILENAME                    the board's own save path
    Board6 2DCONFIGFULLFILENAME        the 2D view settings file (it contains the Windows login
                                       and the Altium install ID)
    Components6 SOURCEFOOTPRINTLIBRARY the footprint library (one-resistor only)
These are notes Altium keeps for itself; none of them changes the board's geometry. A reader must
expect them to be blank. Altium Designer 26.12.2 opens the blanked files normally.
