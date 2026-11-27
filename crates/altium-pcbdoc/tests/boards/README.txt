Original test board made for Dutchman Electronics. Released under CC0 1.0 (public domain): https://creativecommons.org/publicdomain/zero/1.0/
Exception: the TrueType font files embedded in one-dejavu-text, one-dejavu-bold-text, one-dejavu-italic-text, all-features and text-kinds are not CC0; they keep the DejaVu / Bitstream Vera license (LICENSE-DejaVu.txt). Everything else in those five boards is CC0.

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

Scripted board: a through-hole header with a STEP model (one-step-header)
------------------------------------------------------------------------
Made on 2026-11-26 by my own script, from a file copy of empty-outline.PcbDoc; every number was
read back and logged.

The STEP model header-1x4.step is my own, written by this repository's board-step crate (CC0 like
the board): five solids in one file. The housing is a box x -1.270 to 1.270, y -5.080 to 5.080,
z 0 to 2.500, with four square holes 0.640 x 0.640 through it, centred at x 0 and y 3.810,
1.270, -1.270 and -3.810. The four pins are boxes 0.640 x 0.640 at those centres, from z -3.000
to 8.500, so on the board they reach through its 1.600 thickness and 1.400 below it. The pins
fill the holes exactly.

one-step-header.PcbDoc
    one component: designator J1, comment "HEADER" (hidden), footprint HDR1X4-STEP, centre
    (25.000, 15.000), rotation 0, top side. Designator shown on Top Overlay at (23.600, 21.000),
    height 1.000, stroke 0.150.
    Pads, all on Multi-Layer, 1.700 x 1.700, rotation 0, hole round 1.000, plated:
        pad 1 at (25.000, 18.810), rectangle (square)
        pad 2 at (25.000, 16.270), round
        pad 3 at (25.000, 13.730), round
        pad 4 at (25.000, 11.190), round
    Silkscreen (Top Overlay, width 0.150, part of J1): a rectangle (23.600, 9.790) to
    (26.400, 20.210).
    3D body on Mechanical 1 holding header-1x4.step, embedded, its origin on J1's centre, not
    turned or raised. Altium read back x 23.73 to 26.27, y 9.92 to 20.08, standoff -3.000
    (the pins' tips below the top face) and overall height 8.500. Altium gave the model
    MODELID {AFE25888-7D73-4AF3-9861-6835E8C67860} and stored the file unchanged.
Scripted boards: objects on mechanical layers (mechanical-layers, mechanical-pair-resistor,
mechanical-pair-bottom-resistor)
------------------------------------------------------------------------------------------
Made on 2026-11-26 by my own script; every object on a mechanical layer was read back and logged
after the change. Altium numbers its first 16 mechanical layers 57 (Mechanical 1) to 72
(Mechanical 16) in the records, and names each in /Board6/Data (LAYER57NAME=Mechanical 1, ...).
A layer the script used was switched on (LAYERnMECHENABLED=TRUE). A pair of mechanical layers is
stored as MECHPAIRnL1 and MECHPAIRnL2: one-resistor already had Mechanical 2 paired with
Mechanical 3 (MECHPAIR0, Altium's own), and the two pair boards add Mechanical 13 with
Mechanical 15 (MECHPAIR1L1=MECHANICAL13, MECHPAIR1L2=MECHANICAL15).

mechanical-layers.PcbDoc  (from empty-outline)
    Free objects (no net, no component):
        Mechanical 1    track (5.000, 5.000) to (45.000, 5.000), width 0.200
        Mechanical 1    arc centre (10.000, 20.000), radius 3.000, 0 to 360 degrees (a full
                        circle), width 0.150
        Mechanical 1    arc centre (18.000, 20.000), radius 3.000, 0 to 90 degrees, width 0.150
        Mechanical 2    fill (30.000, 10.000) to (34.000, 14.000), rotation 0
        Mechanical 2    text "MECH", stroke font, reference point (20.000, 10.000), height 1.500,
                        stroke 0.150, rotation 0, not mirrored
        Mechanical 13   region, a triangle (38.000, 18.000), (46.000, 18.000), (42.000, 26.000)
                        (Altium lists its vertices from (46, 18)); KIND 0

mechanical-pair-resistor.PcbDoc  (from one-resistor)
    Mechanical 13 and Mechanical 15 paired. R1 (still at (25.000, 15.000), rotation 0, top) got an
    assembly outline of five tracks on Mechanical 13, width 0.100, all part of R1:
        a rectangle (24.100, 14.500) to (25.900, 15.500), and a pin-1 mark on the pad 1 side
        from (24.100, 15.000) to (24.600, 15.500)

mechanical-pair-bottom-resistor.PcbDoc  (from one-resistor)
    The same, then R1 flipped to the bottom side. Altium shows R1 with rotation 180 (I set only
    the side), centre still (25, 15), and moved its five tracks to Mechanical 15, the other layer
    of the pair, mirrored about R1's centre in x:
        the rectangle (24.100, 14.500) to (25.900, 15.500) as before (it is symmetric), and the
        pin-1 mark now from (25.400, 15.500) to (25.900, 15.000)

Scripted board: pad shapes (pad-shapes)
---------------------------------------
Made on 2026-11-26 from a file copy of empty-outline.PcbDoc by my own script; every pad was read
back and logged, and the board saved. Free pads (no net, no component), all in simple mode (one
size and shape for every layer). Altium's script names the shapes eOctagonal (3) and
eRoundedRectangular (9); the corner of a rounded rectangle is set as a percentage. Altium's
Gerber file of the top copper (made only to check the numbers below, not kept) draws each corner
radius as the percentage of half the shorter side, and each octagon as the pad's rectangle with
its corners cut off at 45 degrees by a quarter of the shorter side.

pad-shapes.PcbDoc  (from empty-outline)
    Pad  Centre              Layer        Size           Rotation  Shape              Corner
    1    (10.000, 15.000)    Top Layer    2.000 x 1.000  0         rounded rectangle  50%: radius 0.250
    2    (20.000, 15.000)    Top Layer    2.000 x 1.000  0         rounded rectangle  25%: radius 0.125
    3    (30.000, 15.000)    Top Layer    2.000 x 1.000  30        rounded rectangle  25%: radius 0.125
    4    (40.000, 15.000)    Top Layer    2.000 x 1.000  0         octagon            corners cut 0.250
    5    (40.000, 24.000)    Top Layer    1.500 x 1.500  0         octagon            corners cut 0.375
    6    (20.000, 24.000)    Multi-Layer  1.600 x 1.600  0         rounded rectangle  30%: radius 0.240
    Pad 6 has a round plated hole of 0.800; the others have none.

Scripted board: two internal planes (four-layer-planes)
------------------------------------------------------
Made on 2026-11-26 from a file copy of empty-outline.PcbDoc. By hand, in Design > Layer Stack
Manager: Add > Insert layer below Top Layer > Plane, count 1. With the stack kept symmetric,
Altium added two plane layers, each with a thin prepreg next to it, with its default names and
thicknesses, as for four-layer (see the twelfth board). The rest by my own scripts, each change
read back and logged:
1. Two nets, GND and VCC. Layer 1 got the net GND and Layer 2 the net VCC as plane nets
   (PLANE1NETNAME=GND, PLANE2NETNAME=VCC). Altium's default pull-back of 0.508 (20 mil) was kept.
2. The vias, pad and split line below, then the nets of the vias and the pad.
3. The net of each split-plane region (Altium had made the regions before the nets existed, so
   they had none): all of Layer 1 GND, the part of Layer 2 left of the split line VCC, the part
   right of it left without a net.
4. The plane clearance rule set to 0.300 instead of Altium's default 0.508, so it differs from
   the relief expansion.
By hand, Tools > Split Planes > Rebuild Split Planes on All Layers after steps 3 and 4 (without
it Altium did not connect any via to a plane), and a save. Altium's Gerber files of the two
planes (made only to check the numbers below, not kept) match them.

four-layer-planes.PcbDoc  (from empty-outline)
    Layer stack (total 1.812, the same thicknesses as four-layer):

        Top Overlay      (no thickness)
        Top Solder       solder resist   0.010
        Top Layer        copper          0.035
        Dielectric 2     prepreg         0.071   (2.8 mil)
        Layer 1          plane           0.035   GND, pull-back 0.508
        Dielectric 1     FR-4            1.510
        Layer 2          plane           0.035   VCC, pull-back 0.508
        Dielectric 3     prepreg         0.071   (2.8 mil)
        Bottom Layer     copper          0.035
        Bottom Solder    solder resist   0.010
        Bottom Overlay   (no thickness)

    Vias, from Top Layer to Bottom Layer, diameter 0.600, hole 0.300:
        (10.000, 10.000)   GND
        (10.000, 20.000)   VCC
        (35.000, 20.000)   no net
    Pad "1" (no component) at (40.000, 10.000), Multi-Layer, round 1.600, hole 0.900, plated,
    GND.
    Split line on Layer 2: a track from (25.000, 0.000) to (25.000, 30.000), width 0.500.

    Design rules (the only plane rules, both for every object):
        Plane clearance    0.300
        Plane connect      relief, expansion 0.508, air gap 0.254, conductor width 0.254,
                           4 entries

    What Altium shows and writes for the planes:
        Layer 1 (GND)              one region; thermal reliefs on the GND via and the GND pad,
                                   clearances around the other two vias
        Layer 2, left of x = 25    VCC; a relief on the VCC via, a clearance around the GND via
        Layer 2, right of x = 25   no net; clearances around the via at (35, 20) and the pad
    Round clearances, from the Gerber files: 0.900 across around a via (its hole plus twice the
    clearance), 1.500 around the pad. Reliefs: air from 1.316 across (the hole plus twice the
    expansion) to 1.824 around a via, 1.916 to 2.424 around the pad, crossed by four spokes
    0.254 wide at 45, 135, 225 and 315 degrees.

All features: where the reader stands (all-features)
---------------------------------------------------
A test board of everything the reader handles, on one board, to see where it stands. It is not a
reference for the look (that is dense-board). Whenever the reader learns something new, add it
to this board (by hand or by script, from the board as it is now), describe it below and add a
check to tests/all_features.rs; whatever this board still loses is listed there as the known
gaps.

Made on 2026-11-26 from a file copy of dense-board.PcbDoc (all of its parts, nets, tracks, vias,
pours, silkscreen and 3D bodies stay as described there), partly by hand and partly by my own
scripts, every scripted change read back and logged:
1. By hand, in Design > Layer Stack Manager: Insert layer below Top Layer > Plane, and Insert
   layer above Top Layer > Surface Finish, process ENIG from Altium's list. With the stack kept
   symmetric Altium added two planes and a finish on each face, with its default names and
   thicknesses:

        Top Overlay           (no thickness)
        Top Solder            solder resist   0.010
        Top Surface Finish    ENIG            0.004   "Nickel, Gold", color #FFFFC400
        Top Layer             copper          0.035
        Dielectric 2          prepreg         0.071   (2.8 mil)
        Layer 1               plane           0.035   GND, pull-back 0.508
        Dielectric 1          FR-4            1.510
        Layer 2               plane           0.035   +3V3, pull-back 0.508
        Dielectric 3          prepreg         0.071   (2.8 mil)
        Bottom Layer          copper          0.035
        Bottom Surface Finish ENIG            0.004
        Bottom Solder         solder resist   0.010
        Bottom Overlay        (no thickness)

2. By script: the plane nets (PLANE1NETNAME=GND, PLANE2NETNAME=+3V3, nets the board already
   had); the plane rules stay Altium's defaults (clearance 0.508; relief expansion 0.508, air gap
   0.254, conductor width 0.254, 4 entries).
3. By script, pad shapes (simple mode, the same on every layer; sizes as in dense-board unless
   given):
        U2 pads 1 to 8      rounded rectangle, corner 25% (radius 0.075 on 1.500 x 0.600)
        J1 pads 2 to 4      octagon (1.700 x 1.700, plated hole 1.000)
        J2 pad 1            rounded rectangle, corner 30% (radius 0.255 on 1.700 x 1.700)
        J2 pads 2 to 4      oval 2.200 x 1.700 (round shape, wider than high)
4. By script, texts on Top Overlay:
        "DejaVu"    TrueType DejaVu Sans Mono, not bold, not italic, at (31.600, 1.300), height
                    1.200, rotation 0, no component; box x 31.600 to 35.162, y 1.086 to 2.083
        ".Comment"  stroke font, owned by U1, at (25.400, 12.700), height 0.800, stroke 0.120;
                    Altium shows U1's comment, "MCU"
5. By script, free copper on Top Layer, no net, no component:
        fill        (35.800, 1.600) to (37.400, 3.200), rotation 0
        fill        (39.200, 1.700) to (40.800, 3.100), rotation 30 (about its center (40, 2.4))
        region      triangle (41.800, 1.400), (44.200, 1.400), (43.000, 3.600), KIND 0
        arc         center (25.000, 3.600), radius 1.000, 0 to 180 degrees, width 0.200
6. By script, objects on mechanical layers (Mechanical 1, 4 and 13 switched on):
        Mechanical 1    track (26.000, 0.900) to (44.000, 0.900), width 0.100
        Mechanical 1    arc center (46.500, 5.500), radius 2.200, 0 to 360 degrees, width 0.100
        Mechanical 2    text "ASSY", stroke font, at (30.000, 4.000), height 1.000, stroke 0.100
        Mechanical 4    arc center (46.500, 5.500), radius 1.500, 0 to 360 degrees, width 0.100
                        (the circle the cutout was made from; it stays on the board)
        Mechanical 13   region, triangle (21.000, 27.600), (23.000, 27.600), (22.000, 29.000)
7. By hand: the Mechanical 4 circle selected, Tools > Convert > Create Board Cutout from Selected
   Primitives: one round board cutout, center (46.500, 5.500), radius 1.500. Then Tools >
   Polygon Pours > Repour All and Tools > Split Planes > Rebuild Split Planes on All Layers.
8. By hand, in the 3D view's View Configuration: solder mask, top and bottom, 17, 85, 204
   (stored as 13391121), opacity 0.6 as before. Silkscreen stays Altium's default.
9. By script, later the same day, from the board as described so far: J1's extruded body (the
   2.540 x 10.160 box, height 2.500, of dense-board) taken off, and a 3D body on Mechanical 1
   holding the embedded header-1x4.step of one-step-header put on, its origin on J1's centre
   (3.500, 15.000), not turned or raised. Altium read back x 2.23 to 4.77, y 9.92 to 20.08,
   standoff -3.000 and overall height 8.500, and gave the model MODELID
   {EAA8DA22-03CC-4142-A160-71B7E4E5E84E}; the file is stored unchanged. J2 keeps its extruded
   body.
10. A project, all-features.PrjPcb, beside the board. By hand: File > New > Project, Local
   Projects, PCB, <Empty>, name all-features, with two text parameters:
        PCBNumber      DN-0001
        BoardRevision  A
   then Add Existing to Project with a file copy of the board, and the project saved. Altium
   put the project and the board in a folder of their own; both files were moved here, and the
   project links the board by its plain name (DocumentPath=all-features.PcbDoc). Altium wrote
   the name of the PC's default printer into the project's ten TargetPrinter= lines; each was
   emptied, as for param-project. Then, by my own script with the project open, two texts
   in the stroke font, rotation 0, read back with the string Altium shows:
        Stored text          Layer           At            Height  Stroke  Mir.  Altium shows
        .PCBNumber           Top Overlay     6.200, 27.600 0.700   0.100   no    DN-0001
        Rev .BoardRevision   Bottom Overlay  44.000, 3.750 0.600   0.100   yes   Rev A
   Their boxes, around the shown string: x 6.100 to 10.732, y 27.500 to 28.400, and x 41.101
   to 44.100, y 3.650 to 4.450. Saving put Arial into the board again; it was removed as before.
11. On 2026-11-27, from the board as described so far. By my own script, read back and logged:
   two texts on Top Overlay, rotation 0, no component, height 0.700, stroke 0.100:
        "Sans"   Sans Serif stroke font, at (25.000, 27.000); box x 24.900 to 27.226,
                 y 26.900 to 27.800
        "Serif"  Serif stroke font, at (28.000, 27.000); box x 27.900 to 30.410,
                 y 26.900 to 27.800
   and four vias, no net, diameter 0.600, hole 0.300, Top Layer to Bottom Layer, at y 7.000.
   Then by hand, in each via's Properties panel, Solder Mask Expansion set to Manual and:
        x       solder mask
        27.600  Tented on top only (expansion 0.1 on the open bottom)
        29.000  Tented on the bottom only
        30.400  Tented on both sides
        31.800  open on both sides, expansion 0.2 (top and bottom linked)
   Every other via keeps the board's rule (open, 0.1). Then Tools > Polygon Pours > Repour All
   and Tools > Split Planes > Rebuild Split Planes on All Layers, and the board saved; Arial
   was removed again as before.
12. Later on 2026-11-27, from the board as described so far, opened through a copy of
   all-features.PrjPcb (so with its parameters PCBNumber DN-0001 and BoardRevision A). Free
   spots were found from the board's own objects (every object's box as Altium reports it, with
   tracks and vias by their exact shape). By my own script, read back and logged: four free pads
   (no component, no net, simple pad stack) and four texts on Top Overlay in the stroke font
   (Default), height 0.600, stroke 0.100, rotation 0, not mirrored:
        pad  at              kind
        M1   35.600, 28.150  SMD, Top Layer, rectangle 1.200 x 0.800, no hole
        M2   37.600, 28.150  SMD, Top Layer, rectangle 1.200 x 0.800, no hole
        M3   43.300,  8.500  through-hole, Multi-Layer, round 1.600, hole 0.800, plated
        M4   45.400,  8.500  through-hole, Multi-Layer, round 1.600, hole 0.800, plated

        Stored text                       At             Owner  Altium shows  Box
        Rev. '.BoardRevision'             33.700, 18.500 -      Rev. A        33.600, 18.400 to
                                                                              37.099, 19.200
        '.PCBNumber' / '.BoardRevision'   5.400, 11.700  -      DN-0001 / A   5.300, 11.600 to
                                                                              11.798, 12.400
        '.Designator'                     41.300, 14.500 R7     R7            41.200, 14.400 to
                                                                              42.400, 15.200
        (no characters)                   9.000, 10.600  -      (nothing)     the anchor point
   The file keeps each text as typed (/WideStrings6/Data); the empty text has an entry of length
   2 with no bytes, as on empty-texts. R7 is the part at (40.500, 16.270).
   Then by hand, in each pad's Properties panel (Pad Stack > Solder), one setting per pad:
        M1   Manual Expansion, 0.2mm
        M2   Manual Expansion (0.1mm kept), Tented ticked
        M3   Manual Expansion, top and bottom unlinked (the link button in the expanded Top
             Solder row), top 0.2mm, bottom 0.05mm
        M4   Manual Expansion, 0.1mm, From Hole Edge ticked (top and bottom linked); from the
             pad's edge that is (0.8 + 0.2 - 1.6) / 2 = -0.300
   Paste stays by the rule on every pad. The script read back: M1 0.2, M2 tented on top with
   0.1, M3 0.2 (the top value), M4 0.1. Then, in the 2D view, Tools > Polygon Pours > Repour
   All and Tools > Split Planes > Rebuild Split Planes on All Layers, the board saved and closed
   and the project copy closed. Arial was removed again as before.
A first try put the TrueType text and the free copper over "DUTCHMAN NODE 1"; a second script
removed them and placed them as above before the board was first saved with the cutout.

Altium embedded Arial on saving as well as DejaVu Sans Mono. After each save my own tool
copied every stream into a new file unchanged, except EmbeddedFonts6, which now holds only the
DejaVu Sans Mono record (Header 1): byte for byte the DejaVu Sans Mono 2.37 file of
one-dejavu-text, under its own license (see "Embedded TrueType font" above and
LICENSE-DejaVu.txt). Everything else on this board is CC0 like the others.

What Altium stores for the planes: each plane is one split-plane region, the board outline less
the pull-back. The cutout is not part of that region: Altium draws the pull-back around the
cutout (and along the board edge) as tracks on the plane layer.

Solder mask expansion (mask-expansion-0, -0-05, -0-15, -0-2; every board re-saved)
----------------------------------------------------------------------------------
Made on 2026-11-26 by my own script; the rule was read back and logged before and after the
change. Every board here had one solder mask expansion rule (Design > Rules > Manufacturing >
Solder Mask Expansion), Altium's own record in /Rules6/Data: RULEKIND=SolderMaskExpansion,
NAME=SolderMaskExpansion, SCOPE1EXPRESSION=All, SCOPE2EXPRESSION=All, ENABLED=TRUE,
PRIORITY=1, EXPANSION=0mil. So far every board opened its mask exactly at the pad's edge.

1. Every board in this folder, its subfolders included, was opened in Altium, its rule's
   expansion set to 0.100 mm (Altium writes EXPANSION=3.937mil), saved and closed; nothing else
   was changed on purpose. A board copied from any of them now starts with 0.1 mm. The one
   exception is linked-model/one-linked-step-resistor: saved again (with box-1608.step beside
   it), Altium dropped its link to the model (MODEL.NAME became empty and the checksum
   changed), so it was kept as it was, with 0mil.
2. Four new boards, each a file copy of one-resistor.PcbDoc with the rule set as below (nothing
   else changed; R1's two bare pads of 0.800 x 1.000 at (24.200, 15.000) and (25.800, 15.000)):
        mask-expansion-0.PcbDoc       0.000 mm   EXPANSION=0mil
        mask-expansion-0-05.PcbDoc    0.050 mm   EXPANSION=1.9685mil
        mask-expansion-0-15.PcbDoc    0.150 mm   EXPANSION=5.9055mil
        mask-expansion-0-2.PcbDoc     0.200 mm   EXPANSION=7.874mil
Saving embedded Arial again in one-ttf-text, one-via, small-board and param-project/param-board
(none of their texts uses it); it was removed the same way as before (see "Paths inside the
files"). The DejaVu boards and all-features keep only their DejaVu Sans Mono file.

Vias and stroke fonts (vias-tenting, vias-tented-rule, vias-sizes, vias-blind-buried, stroke-fonts)
---------------------------------------------------------------------------------------------------
Made on 2026-11-26 by my own script, partly by hand where noted; every via and text was read back
and logged. vias-tenting, vias-sizes and stroke-fonts are file copies of empty-outline,
vias-tented-rule of vias-sizes, vias-blind-buried of four-layer (its track and via stay). All of
them carry the 0.1 mm mask expansion rule (see "Solder mask expansion"). Each via has a label on
Top Overlay (stroke font, height 0.800, stroke 0.120) just above it, naming its variant. All vias
have no net.

vias-tenting.PcbDoc
    Five vias, diameter 0.600, hole 0.300, Top Layer to Bottom Layer, at y 15.000:
        x       label   solder mask (in the via's Properties panel, by hand)
        8.000   RULE    by the rule: open on both sides, expansion 0.1
        16.000  TOP     Manual, Tented on top only (expansion 0.1 on the open bottom)
        24.000  BOT     Manual, Tented on the bottom only
        32.000  BOTH    Manual, Tented on both sides
        40.000  M0.2    Manual, 0.200 on both sides, not tented
    The board's rule says ISTENTINGTOP=FALSE. Setting tenting or a manual expansion by script did
    not work in Altium 26.12.2 (tenting did not stick, and setting the expansion stopped the
    scripting engine), so these were set by hand; the script read them back afterwards.

vias-tented-rule.PcbDoc  (from vias-sizes)
    The five vias of vias-sizes. By hand, in Design > Rules, the SolderMaskExpansion rule's Tented
    box ticked (top and bottom linked); the rule then reads ISTENTINGTOP=TRUE, EXPANSION=3.937mil.
    Altium reads every via back as tented on both sides, and reports each via's mask expansion as
    minus its diameter (-0.6 for the 0.6 mm via).

vias-sizes.PcbDoc
    Five vias, Top Layer to Bottom Layer, open (the rule), at y 15.000:
        x       diameter / hole   label
        8.000   0.400 / 0.200     0.4/0.2
        16.000  0.600 / 0.300     0.6/0.3
        24.000  0.800 / 0.400     0.8/0.4
        32.000  1.000 / 0.500     1.0/0.5
        41.000  1.500 / 0.800     1.5/0.8

vias-blind-buried.PcbDoc  (from four-layer)
    Besides four-layer's own through via at (30, 15), four vias, diameter 0.600, hole 0.300, at
    y 25.000, open (the rule):
        x       from          to             label
        10.000  Top Layer     Layer 1        T-L1   (blind)
        20.000  Layer 2       Bottom Layer   L2-B   (blind)
        30.000  Layer 1       Layer 2        L1-L2  (buried)
        40.000  Top Layer     Layer 2        T-L2   (blind, through one inner layer)
    Layer 1 and Layer 2 are Altium's Mid Layer 1 and Mid Layer 2. No via types were defined in the
    Layer Stack Manager; Altium accepted the spans as set and reads them back.

stroke-fonts.PcbDoc
    Nine texts on Top Overlay, stroke font, height 1.200, stroke 0.150, rotation 0, not mirrored,
    no component, each anchored at x 2.000, in Altium's three stroke fonts (the font set by
    script; Altium names them Default, Sans Serif and Serif and numbers them 1, 2 and 3):
        y       font         text                               box (x, y to x, y)
        26.000  Default      ABCDEFGHIJKLMNOPQRSTUVWXYZ         1.850, 25.850 to 32.740, 27.350
        23.500  Default      abcdefghijklmnopqrstuvwxyz         1.850, 22.950 to 32.140, 24.850
        21.000  Default      0123456789 .,:;-+=/()[]#%&*?!      1.850, 20.450 to 34.340, 22.550
        17.000  Sans Serif   ABCDEFGHIJKLMNOPQRSTUVWXYZ         1.850, 16.736 to 29.218, 18.350
        14.500  Sans Serif   abcdefghijklmnopqrstuvwxyz         1.850, 13.950 to 24.711, 15.907
        12.000  Sans Serif   0123456789 .,:;-+=/()[]#%&*?!      1.850, 11.450 to 28.030, 13.578
        8.000   Serif        ABCDEFGHIJKLMNOPQRSTUVWXYZ         1.850, 7.564 to 31.075, 9.350
        5.500   Serif        abcdefghijklmnopqrstuvwxyz         1.850, 4.950 to 28.133, 6.850
        3.000   Serif        0123456789 .,:;-+=/()[]#%&*?!      1.850, 2.450 to 26.790, 4.578
    Default is monospaced; Sans Serif and Serif are proportional. Altium draws Sans Serif's "[]"
    as a single box.

Saving embedded Arial in the four via boards (none of their texts uses it); it was removed as
before (see "Paths inside the files").

Pad mask overrides (pad-mask-overrides)
---------------------------------------
Made on 2026-11-27 from a file copy of one-plated-pad.PcbDoc (same outline and stack; its
SolderMaskExpansion rule is 0.1 mm for all objects). By my own script, read back and logged: its
pad removed and eight free pads added (no component, no net), simple pad stack, at y 15.000:
    SMD pads         Top Layer, rectangle 2.000 x 1.000, no hole
    through-hole     Multi-Layer, round 2.000, hole 1.000, plated
Then by hand, in each pad's Properties panel (Pad Stack > Solder / Paste), one setting changed
per pad, everything else left as Altium made it (paste and mask by the rule, not tented):
    pad  x       kind          setting
    P1   13.000  SMD           none: mask and paste by the rule (the reference pad)
    P2   17.000  SMD           Solder: Manual Expansion, 0mm
    P3   21.000  SMD           Solder: Manual Expansion, 0.2mm
    P4   25.000  SMD           Solder: Manual Expansion (0.1mm kept), Tented ticked; Altium
                               offers Tented only once the mask is Manual
    P5   29.000  through-hole  Solder: Manual Expansion, top and bottom unlinked (the link
                               button in the expanded Top Solder row), top 0.2mm, bottom 0.05mm
    P6   33.000  through-hole  Solder: Manual Expansion (0.1mm kept), Tented ticked on Bottom
                               Solder only
    P7   37.000  SMD           Paste: Manual Expansion, 0.05mm (Altium then shows 15.5 % in
                               the % column); solder mask by the rule
    P8   41.000  through-hole  Solder: Manual Expansion, 0.1mm, From Hole Edge ticked
Saved once and closed; Altium embedded Arial on saving and it was removed as before (no text on
this board uses it).


Quoted special strings (quoted-strings, issue 146)
--------------------------------------------------
Made on 2026-11-27 in a copy of param-project (PCBNumber DN-0042, BoardRevision B; see
param-project/README.txt), whose param-board-2 was replaced by a file copy of empty-outline. With
the project open, my own script added these texts and read each back with the string Altium
shows for it; the board was then saved and copied here alone (it is not part of any project
here: a reader gives it the parameters of param-project). Every text is in the stroke font
(Default), height 1.000, stroke 0.120, rotation 0, x 2.000, on Top Overlay unless noted:
    y       Stored text                          Altium shows
    27.500  Rev. '.BoardRevision'                Rev. B
    25.300  '.BoardRevision' end                 B end
    23.100  '.PCBNumber' Rev '.BoardRevision'    DN-0042 Rev B
    20.900  '.PCBNumber''.BoardRevision'         DN-0042B
    18.700  '.pcbnumber'                         DN-0042
    16.500  '.Layer_Name'                        Top Overlay
    14.300  L: '.Layer_Name' x                   L: Top Overlay x
    12.100  '.NoSuchParameter'                   '.NoSuchParameter'
    9.900   Rev '.BoardRevision                  Rev 'B
    7.700   Rev ''                               Rev ''
    5.500   'word' Rev                           'word' Rev
    3.300   Rev .BoardRevision end               Rev B end
    1.100   '.BoardRevision'  (Bottom Overlay)   B
The file keeps each text as typed. A name in quotes is replaced wherever it stands, as often as
it occurs, the quotes with it; an unknown name keeps its quotes; a quote with no name after it
stays. With no closing quote ("Rev '.BoardRevision") Altium drops the reference and keeps the
opening quote. Unquoted, a reference followed by more text ("Rev .BoardRevision end") is replaced
too here, although ".PCBNumber rev" on param-board stays as typed.

Quoted component strings (quoted-component-strings, issue 146)
--------------------------------------------------------------
Made on 2026-11-27 from a file copy of one-resistor.PcbDoc (R1 at 25, 15, comment "10k") by my
own script, with no project open; each text read back with the string Altium shows. Stroke font
(Default), height 1.000, stroke 0.120, rotation 0. "Owner" R1: the text was added to R1.
    Stored text                  Owner Layer           At       Mir. Altium shows
    '.Designator'                R1    Top Overlay     28, 27   no   R1
    R: '.Designator'             R1    Top Overlay     28, 24.8 no   R: R1
    '.Comment' ohm               R1    Top Overlay     28, 22.6 no   10k ohm
    '.Designator' '.Comment'     R1    Top Overlay     28, 20.4 no   R1 10k
    '.designator'                R1    Top Overlay     28, 18.2 no   R1
    L: '.Layer_Name'             R1    Top Overlay     30, 11   no   L: Top Overlay
    B: '.Designator'             R1    Bottom Overlay  48, 7    yes  B: R1
    '.Designator'                -     Top Overlay     2, 27    no   '.Designator'
    '.Comment'                   -     Top Overlay     2, 24.8  no   '.Comment'
    R: '.Designator' x           -     Top Overlay     2, 22.6  no   R: '.Designator' x
    '.Layer_Name' x              -     Bottom Overlay  48, 3    yes  Bottom Overlay x
So the component's special strings resolve inside quotes as the project parameters do, on either
side; on a free text '.Designator' and '.Comment' stay as typed, quotes included.


Empty texts (empty-texts)
-------------------------
Made on 2026-11-27 from a file copy of empty-outline by my own script, read back and logged:
seven stroke texts (Default, height 1.000, stroke 0.120, rotation 0), four with one letter and
three with no characters at all:
    Text  Layer           At
    A     Top Overlay     5, 25
    (none) Top Overlay    5, 20
    B     Top Overlay     5, 15
    (none) Bottom Overlay 25, 20
    C     Bottom Overlay  25, 15
    (none) Mechanical 1   5, 10
    D     Mechanical 1    5, 5
Altium reports an empty text's box as its anchor point. In /WideStrings6/Data each empty text has
an entry of its index and a byte length of 2 but no bytes after it: the stream reads
[1][2] [2][4]"A" ... [4][2] [5][4]"C" [6][2], where every other entry holds its UTF-16 text and a
closing 0 in its length.


Features seen on other people's boards (mask-opacity-100, origin-moved, overlay-region-fill,
body-two-extruded, body-cylinder, body-sphere, arc-region, pad-stack-tmb, solder-mask-region,
overlapping-holes, body-arc-outline)
---------------------------------------------------------------------------------------------
Made on 2026-11-27, each from a file copy of one of our own boards, one feature per board, so the
reader can learn features that public sample boards use (only our converter's notes on those
boards were looked at, never their bytes). Every object was read back and logged; each board was
saved once and closed.

mask-opacity-100.PcbDoc  (from one-resistor)
    By hand, in the 3D view's View Configuration panel (View Options, 3D Settings), the
    Transparency sliders of Top Solder Mask and Bottom Solder Mask dragged to 0. Nothing else
    changed on purpose. /Board6/Data then reads CFG3D.TOPSOLDERMASKCOLOROPACITY=1.000000 and
    CFG3D.BOTSOLDERMASKCOLOROPACITY=1.000000 (0.600000 on one-resistor); the colors stay
    CFG3D.TOPSOLDERMASKCOLOR=3307556 and CFG3D.BOTSOLDERMASKCOLOR=3307556 (36, 120, 50). Altium
    26.12.2 writes both opacity keys on every save, so a board with a mask color and no opacity
    key cannot be made by saving here. The board was saved from the 3D view
    (CURRENT2D3DVIEWSTATE=3D).

origin-moved.PcbDoc  (from one-resistor)
    By my own script, the board origin (what Edit > Origin > Set changes) moved from the
    outline's lower-left corner to the point 10.000 right and 5.000 up from it. Nothing moved on
    the board: /Board6/Data reads ORIGINX=393.7008mil, ORIGINY=196.8504mil (0mil before) and the
    grid origin GR0_OX and GR0_OY the same, while the outline (VX0 to VY4) and every object keep
    their stored coordinates. Read back from the new origin:
        outline     (-10, -5) to (40, 25)
        R1          center (15, 10); pads at (14.2, 10) and (15.8, 10)
    So stored coordinates still have the outline's corner at (0, 0); only the origin Altium
    shows them from moved.

overlay-region-fill.PcbDoc  (from empty-outline)
    By my own script, solid shapes on both silkscreen layers, no net, no component:
        Top Overlay      region  (5, 18), (15, 18), (13, 25), (7, 25), KIND 0, no holes
                         fill    (20, 19) to (26, 23), rotation 0
                         fill    (32, 19) to (38, 23), rotation 30 (about its center (35, 21));
                                 Altium's box x 31.402 to 38.598, y 17.768 to 24.232
        Bottom Overlay   region  (5, 5), (15, 5), (13, 12), (7, 12), KIND 0, no holes
                         fill    (20, 6) to (26, 10), rotation 0
                         fill    (32, 6) to (38, 10), rotation 30 (about its center (35, 8));
                                 Altium's box x 31.402 to 38.598, y 4.768 to 11.232

body-two-extruded.PcbDoc  (from one-resistor)
    By my own script, a second extruded body added to R1, on the same layer as its first
    (Mechanical 2, Altium's Top 3D Body), outline (24.6, 14.8), (25.4, 14.8), (25.4, 15.2),
    (24.6, 15.2), standoff 0.450, overall height 0.750 (so 0.300 tall, sitting on the first box),
    color red 0, green 160, blue 224 (stored as 14721024), opacity 1. R1's own body stays: the
    1.600 x 0.800 box of one-resistor, height 0.450, color 128, 128, 128. Both records read
    MODEL.MODELTYPE=0; the new one MODEL.EXTRUDED.MINZ=17.7165mil and MAXZ=29.5276mil.

body-cylinder.PcbDoc  (from one-resistor)
    By hand: R1's body selected (by my own script), and in the Properties panel its 3D Model
    Type changed from Extruded to Cylinder, Height 1mm, Radius 0.5mm; rotations X, Y and Z stay
    0 (upright) and standoff 0. Location (25, 15), R1's center. Read back: box (24.5, 14.5) to
    (25.5, 15.5), standoff 0, overall height 1.000, still owned by R1, model type 2. The record
    reads MODEL.MODELTYPE=2, MODEL.CYLINDER.RADIUS=19.685mil, MODEL.CYLINDER.HEIGHT=39.3701mil,
    STANDOFFHEIGHT=0mil, OVERALLHEIGHT=39.3701mil, MODEL.2D.X=984.252mil, MODEL.2D.Y=590.5512mil.

body-sphere.PcbDoc  (from one-resistor)
    The same, with 3D Model Type Sphere, Radius 0.5mm, standoff 0 in the panel. Read back: box
    (24.5, 14.5) to (25.5, 15.5), model type 3, and standoff -0.500 with overall height 0.500:
    the sphere's center lies on the board's top face, half of it below. The record reads
    MODEL.MODELTYPE=3, MODEL.SPHERE.RADIUS=19.685mil, STANDOFFHEIGHT=-19.685mil,
    OVERALLHEIGHT=19.685mil.

arc-region.PcbDoc  (from empty-outline)
    By my own script, two closed outlines of a rounded slot (two straight sides and two half
    circles), each of two tracks and two arcs, width 0.100:
        tracks (10, 13) to (16, 13) and (10, 17) to (16, 17); arcs center (16, 15) and
        (10, 15), radius 2.000, 270 to 90 and 90 to 270 degrees
        the same 20.000 to the right, on Mechanical 1: x 30 to 36, arcs centered (36, 15) and
        (30, 15)
    Then by hand: the first outline selected, Tools > Convert > Create Region from Selected
    Primitives, and its four tracks and arcs deleted; the second selected (by script), Tools >
    Convert > Create 3D Body From selected primitives. A script removed the second outline's
    tracks and arcs. Altium made the region on the active layer (Mechanical 1); my script moved
    it to Top Layer. Read back:
        region   Top Layer, KIND 0, no net; Altium's script interface lists its outline as 38
                 points (the half circles as straight pieces), from (8, 13) to (18, 17)
        3D body  Mechanical 2 (Top 3D Body), no component, extruded, box (28, 13) to (38, 17),
                 standoff 0, overall height 0.508 (Altium's default, OVERALLHEIGHT=20mil),
                 MODEL.2D.X=1299.2125mil (33.000), MODEL.2D.Y=590.5512mil (15.000),
                 ISSHAPEBASED=TRUE
    Both are stored twice, as the other shapes are: /Regions6 and /ComponentBodies6, and the
    shape-based copies /ShapeBasedRegions6 and /ShapeBasedComponentBodies6.

pad-stack-tmb.PcbDoc  (from empty-outline)
    By my own script, one free pad "1" (no component, no net) at (25, 15), Multi-Layer, plated
    hole 0.800, pad stack mode Top-Middle-Bottom (Altium's mode 1; simple is 0):
        top      round      2.000 x 2.000
        middle   round      1.400 x 1.400
        bottom   rectangle  2.400 x 1.600
    The board has only Top Layer and Bottom Layer, so the middle size has no layer to show on.

solder-mask-region.PcbDoc  (from one-fill)
    By my own script, a solid region on Top Solder, KIND 0, no net, corners (22, 12), (28, 12),
    (28, 18), (22, 18): a mask opening drawn by hand, over the middle of one-fill's copper fill
    (20, 10) to (30, 20) on Top Layer. Nothing else on Top Solder.

overlapping-holes.PcbDoc  (from empty-outline)
    By my own script, two free pads (no component, no net), simple pad stack, Multi-Layer, round
    1.600, plated hole 1.000: "1" at (24.6, 15) and "2" at (25.4, 15). Their centers are 0.800
    apart, so the two holes overlap by 0.200 and the two pads by 0.800.

body-arc-outline.PcbDoc  (from one-resistor)
    R1's extruded body replaced by one whose outline has arcs, made as for arc-region's body. By
    my own script: R1's body removed, and a rounded slot drawn on Mechanical 1 (width 0.100):
    tracks (24.6, 14.6) to (25.4, 14.6) and (24.6, 15.4) to (25.4, 15.4), arcs center
    (25.4, 15) and (24.6, 15), radius 0.400, 270 to 90 and 90 to 270 degrees. By hand: Tools >
    Convert > Create 3D Body From selected primitives. By my own script: the four tracks and
    arcs removed, and the new body given to R1 with standoff 0 and overall height 0.500. Read
    back: Mechanical 2 (Top 3D Body), owned by R1, extruded, box (24.2, 14.6) to (25.8, 15.4),
    color 128, 128, 128, opacity 1. The record reads ISSHAPEBASED=TRUE, MODEL.MODELTYPE=0,
    STANDOFFHEIGHT=0mil, OVERALLHEIGHT=19.685mil, MODEL.EXTRUDED.MINZ=0mil and
    MAXZ=19.685mil, MODEL.2D.X=984.2519mil, MODEL.2D.Y=590.5511mil (25.000, 15.000).

Saving embedded Arial in each of them (none of their texts uses it); it was removed as before
(see "Paths inside the files").

Feature gaps, first batch (pad-hole-shapes, keepouts, pour-modes, mech-layer-kinds)
-----------------------------------------------------------------------------------
Made on 2026-11-27, each from a file copy of empty-outline (mech-layer-kinds: of one-resistor),
one feature per board, by my own scripts and by hand where noted; every object was read back and logged, and each board was saved once and closed. These
are features Altium offers that no board here had before. "Units" below are the file's own
1/10000 mil (1 mm = 393700.787 units).

pad-hole-shapes.PcbDoc
    Five free pads (no component, no net), simple pad stack, Multi-Layer, at y 15.000:
        pad  x       pad shape and size        hole                                 plated
        1    8.000   round 2.000 x 2.000       round, 1.000                         yes
        2    16.000  round 3.000 x 1.600 (oval) slot, width 0.800, length 2.200, 0 deg yes
        3    25.000  round 3.000 x 3.000       slot, width 0.800, length 2.200, 45 deg yes
        4    34.000  rectangle 2.000 x 2.000   square, 1.000                        yes
        5    42.000  round 3.000 x 1.600 (oval) slot, width 0.800, length 2.200, 0 deg no
    The pads themselves are not turned; pad 3's hole is turned 45 degrees inside its round pad.
    What the file stores (/Pads6/Data, each pad a record of six blocks):
        block 4 (geometry), from byte 45: the hole size, which is the slot's width (0.800 on
            pads 2, 3 and 5) or the square's side (1.000 on pad 4); byte 60 the plating (0 on
            pad 5)
        block 5: empty (length 0) on pad 1; 651 bytes on pads 2 to 5. In it, byte 262 is the hole
            type (1 square, 2 slot), bytes 263..267 the slot's length (866142 units, 2.200, on
            pads 2, 3 and 5; 0 on pad 4), and bytes 267..275 the hole's rotation as an f64 in
            degrees (45.0 on pad 3, 0.0 on the others)
    Altium draws pads 2, 3 and 5 with a rounded slot and pad 4 with a square hole.

keepouts.PcbDoc
    Objects on the Keep-Out layer (layer byte 56), no net:
        track   (5, 24) to (15, 24), width 0.300
        arc     center (22, 24), radius 2.000, 0 to 180 degrees, width 0.300
        fill    (30, 22) to (36, 26), rotation 0
        region  (40, 22), (46, 22), (46, 26), (40, 26), KIND 0
    Objects on Top Layer (layer byte 1) with the keepout flag set (Altium's "Keepout" box), no
    net, and Altium's default keepout restrictions:
        track   (5, 8) to (15, 8), width 0.300
        fill    (30, 6) to (36, 10), rotation 0
        region  (40, 6), (46, 6), (46, 10), (40, 10), KIND 0
    And one plain Top Layer track, (5, 15) to (15, 15), width 0.300, to compare with.
    What the file stores: byte 2 of the track, fill and region record is 2 on the three flagged
    objects and 0 on every other one. The flagged track has 31 at byte 45 and the flagged fill 31
    at byte 46 (0 on the plain track); the flagged region's text part has
    KEEPOUTRESTRICTIONS=31, which no other region has. So a keepout on Top Layer looks like
    copper unless byte 2 is read: it is not copper on the made board.

pour-modes.PcbDoc
    One net, GND, and four polygon pours on Top Layer with that net, "remove dead copper" off,
    each the same 10.000 x 10.000 square, side by side at y 10 to 20:
        x         fill mode (Altium's Polygon Properties)   track width   grid
        2 to 12   Solid                                       (0.2032)      (0.508)
        14 to 24  Hatched, 90 degrees                         0.300         1.000
        26 to 36  Hatched, 45 degrees                         0.300         1.000
        38 to 48  None (outline only)                         0.300         1.000
    (the solid pour keeps Altium's defaults, 8 and 20 mil, which a solid pour does not use).
    Then by hand Tools > Polygon Pours > Repour All, and the board saved.
    /Polygons6/Data holds one record per pour, in the order None, 45, 90, Solid, with
    HATCHSTYLE=None, 45Degree, 90Degree and Solid, TRACKWIDTH=11.811mil and GRIDSIZE=39.3701mil
    on the three that are not solid, NET=0 (GND) and POURINDEX=3, 2, 1 and 0. What Altium poured:
        Solid     one region on Top Layer, KIND 0, SUBPOLYINDEX=0
        90        24 tracks, width 0.300
        45        34 tracks, width 0.300
        None      4 tracks, width 0.300, along the outline
    Bytes 5..7 of each of these tracks hold the index of its pour's record in /Polygons6/Data (2
    for the 90 degree pour, 1 for 45 degrees, 0 for None).
    A first try, whose script gave each pour four points and no closing point, left a track from
    the board origin (0, 0) to each hatched pour's outline; it was thrown away (Altium's pour
    outline wants its first point repeated after the last).

mech-layer-kinds.PcbDoc  (from one-resistor)
    By hand, in the View Configuration panel: Component Layer Pairs > right-click > Add
    Component Layer Pair, four times, each with a Layer Type from Altium's list. one-resistor
    already had Mechanical 2/3 as its 3D Body pair (LAYER58MECHKIND=3DBodyTop,
    LAYER59MECHKIND=3DBodyBottom, MECHPAIR0L1=MECHANICAL2, MECHPAIR0L2=MECHANICAL3).
        pair     layer type          names Altium gave them
        4 / 5    Courtyard           Top Courtyard / Bottom Courtyard
        6 / 7    Assembly            Top Assembly / Bottom Assembly
        8 / 9    Component Outline   Top Component Outline / Bottom Component Outline
        10 / 11  Designator          Top Designator / Bottom Designator
    Altium's list also offered Coating, Component Center, Dimensions, Glue Points, Gold Plating,
    Value, Die and Wire Bonding. Then by my own script, each part of R1 unless noted:
        Top Courtyard           rectangle (23.2, 14.0) to (26.8, 16.0), four tracks, width 0.050
        Top Assembly            rectangle (24.2, 14.6) to (25.8, 15.4), four tracks, width 0.100
        Top Component Outline   rectangle (23.6, 14.4) to (26.4, 15.6), four tracks, width 0.100
        Bottom Courtyard        one free track (20, 10) to (30, 10), width 0.050
        Top Designator          text ".Designator" (shows R1), stroke font, at (24.6, 14.75),
                                height 0.500, stroke 0.080
    Making the Designator pair, Altium put a second ".Designator" text of R1 on Top Designator
    by itself: at (23.984, 14.238), height 1.524, stroke 0.254, rotation 0, box (23.730,
    13.984) to (26.270, 16.016), centred on R1 (both are stored as ".Designator" in
    /WideStrings6/Data).
    What the file stores, in /Board6/Data, for each layer of a pair: LAYERnNAME (the name above),
    LAYERnMECHENABLED=TRUE and LAYERnMECHKIND, with n = 60 to 67 for Mechanical 4 to 11:
        CourtyardTop, CourtyardBottom, AssemblyTop, AssemblyBottom, ComponentOutlineTop,
        ComponentOutlineBottom, DesignatorTop, DesignatorBottom
    The same again in LAYER_V8_14 to LAYER_V8_21 and V9_CACHE_LAYER72 to 79 (with
    USEDBYPRIMS=TRUE on the layers that have objects), and the pairs in MECHPAIR1L1=MECHANICAL4,
    MECHPAIR1L2=MECHANICAL5, and so on to MECHPAIR4L1=MECHANICAL10, MECHPAIR4L2=MECHANICAL11. The
    objects themselves are the usual track and text records with layer bytes 60, 61, 62, 64 and
    66.

Saving embedded Arial in keepouts (no text uses it); it was removed as before (see "Paths inside
the files").

Feature gaps, second batch (teardrops, plane-polygon-connect, text-kinds, pad-shapes-custom,
paste)
---------------------------------------------------------------------------------------------
Made on 2026-11-27, one feature per board, by my own scripts and by hand where noted; every
object was read back and logged, and each board was saved and closed. Saving embedded Arial in
each (no text uses it); it was removed as before (see "Paths inside the files"); text-kinds keeps
only its DejaVu Sans Mono file, byte for byte the one of one-dejavu-text.

teardrops.PcbDoc  (from empty-outline)
    By my own script, net SIG and on it, all on Top Layer unless noted:
        pad "1"   (10, 15), Multi-Layer, round 1.600, plated hole 0.800
        via       (20, 15), diameter 0.600, hole 0.300, Top Layer to Bottom Layer
        pad "2"   (30, 15), SMD, rectangle 1.500 x 1.000
        via       (40, 15), diameter 0.600, hole 0.300
        tracks    (10, 15) to (20, 15), (20, 15) to (30, 15), (30, 15) to (40, 15) and
                  (40, 15) to (40, 22), width 0.254
    Then by hand Tools > Teardrops with Altium's settings (Add, All, style Curved, Via/TH pad
    30 % long and 70 % wide, SMD pad 100 % and 200 %, tracks and T-junctions on, Adjust
    teardrop size on) and Force teardrops ticked: without it Altium made none here, though its
    report said none failed. (A first try with tracks 0.250 wide, under the board's 0.254 width
    rule, got none either and was thrown away.) Altium made seven teardrops, each a region on
    Top Layer, KIND 0, net SIG, no polygon, with its teardrop flag set; their boxes:
        via (20, 15)   x 19.548 to 19.786, y 14.790 to 15.210 (left)
                       x 20.214 to 20.452, y 14.790 to 15.210 (right)
        via (40, 15)   x 39.548 to 39.786, y 14.790 to 15.210 (left)
                       x 39.790 to 40.210, y 15.214 to 15.452 (up)
        pad "2"        x 28.999 to 29.253, y 14.746 to 15.254 (left)
                       x 30.747 to 31.001, y 14.746 to 15.254 (right)
        pad "1"        x 10.571 to 11.270, y 14.440 to 15.560
    What the file stores: seven ordinary records in /Regions6/Data (and their copies in
    /ShapeBasedRegions6/Data), text part KIND=0, SUBPOLYINDEX=-1, ISSHAPEBASED=FALSE; byte 1 of
    each record is 28 (0x1C), where the other regions we looked at (keepouts, arc-region) have 12
    (0x0C), so 0x10 marks a teardrop. Altium also saves the tool's settings in /Board6/Data:
    TEARDROPPARAM_PERCENTFORVIAS_LENGTH=30, ..._WIDTH=70, ...PADS 100 / 200, ...TRACKS 100 / -1,
    ...TJUNCTIONS 100 / 300 and TEARDROPPARAM_FLAGSEX=1004.

plane-polygon-connect.PcbDoc  (from four-layer-planes)
    Everything of four-layer-planes stays. By my own script: three free pads on net GND,
    Multi-Layer, round 1.600, plated hole 0.800, named R, D and N, at (6, 25), (12, 25) and
    (18, 25); a solid polygon pour on Top Layer, net GND, "remove dead copper" off, outline
    (2, 22), (24, 22), (24, 28), (2, 28); and four rules:
        name              kind             scope                            style      priority
        PlaneConnect_N    PlaneConnect     IsPad And (Name = 'Free-N')      NoConnect  1
        PlaneConnect_D    PlaneConnect     IsPad And (Name = 'Free-D')      Direct     2
        PolygonConnect_N  PolygonConnect   IsPad And (Name = 'Free-N')      NoConnect  1
        PolygonConnect_D  PolygonConnect   IsPad And (Name = 'Free-D')      Direct     2
    The board's own rules stay last (priority 3): PlaneConnect, All, Relief (expansion 20mil,
    conductor 10mil, 4 entries) and PolygonConnect, All, Relief (air gap 10mil, conductor 10mil,
    4 entries, 90 Angle). Altium calls a free pad "Free-" and its name in a query (its teardrop
    report says "Pad Free-1" too); Test Queries matched one pad each. Then by hand Repour All and
    Rebuild Split Planes on All Layers in 2D. What Altium made on Top Layer: around R the relief
    of the board rule (four air gaps), around D only the drill (the pour runs into the pad), and
    around N a round clearance 2.120 across with no spoke; on Layer 1 (GND plane) the same three
    kinds as Altium draws them (relief, direct, isolated).

text-kinds.PcbDoc  (from empty-outline)
    By my own script, twelve texts on Top Overlay, TrueType DejaVu Sans Mono (not bold, not
    italic), height 1.000, rotation 0, no component; then by hand in each text's Properties
    panel (String or Frame, Font Type, Justification, Inverted, Border Mode):
        text    at (script)   setting by hand
        INV     (3, 25)       String, Inverted, Border Mode Margin, Margin Border 0.5mm
        OFS     (18, 25)      String, Inverted, Border Mode Offset, Text Offset 0.3mm
        PLAIN   (33, 25)      none: a plain TrueType string for comparison
        LT CT RT  y 17        Frame 10mm x 4mm; horizontal left / centre / right; vertical top
        LM CM RM  y 10.5      the same, vertical middle
        LB CB RB  y 4         the same, vertical bottom (Altium's default)
    with x 3, 18 and 33 for the left, centre and right column (the frame's lower-left corner).
    Altium offers Inverted only for TrueType (and barcode) texts, not for the stroke font, so
    there is no inverted stroke text. The same three horizontal and three vertical justification
    buttons are offered for a plain string too. Read back after the hand edits (TextKind 1, the
    TrueType kind, on all twelve; location and box as Altium reports them):
        INV     at (3, 25), box (2.5, 24.5) to (4.942, 26.126): the text plus the 0.5 margin
        OFS     at (18, 25), box (17.7, 24.688) to (19.445, 25.637)
        PLAIN   at (33, 25), box (33, 25) to (35.444, 25.626)
        frames  location = the frame's lower-left corner, box = the frame, 10.000 x 4.000:
                LT (3, 17), CT (18, 16.988), RT (33, 17), LM (3, 10.5), CM (18, 10.488),
                RM (33, 10.5), LB (3, 4), CB (18, 3.988), RB (33, 4)
    (The script placed the centre column at y 4, 10.5 and 17 like the others; Altium reports it
    0.012 lower, from the moment the texts were made.) While a frame is
    selected, Altium's panel shows the point its justification refers to (x 23 for the centre
    column, 43 for the right one).

pad-shapes-custom.PcbDoc  (from empty-outline)
    By my own script, three free SMD pads on Top Layer (no net, simple pad stack) and an
    L-shaped region; then by hand:
        pad  at          made as                     by hand
        1    (10, 15)    rectangle 2.000 x 1.000     Shape Rounded Rectangle: Altium's default
                                                     corner radius 50% (0.25mm), all four corners
        2    (20, 15)    rectangle 2.000 x 1.000     Shape Chamfered Rectangle, corner 50%
                                                     (0.25mm); Lower Left and Lower Right
                                                     unticked, so only the two upper corners are
                                                     cut
        3    (30.5, 14.5) rectangle 1.000 x 1.000    with the region selected too: Tools >
                                                     Convert > Add Selected Region to Custom Pad
    The region, KIND 0, Top Layer: (30, 14), (33, 14), (33, 15), (31, 15), (31, 17), (30, 17).
    After the conversion pad 3's panel shows Shape Custom Shape and a size of 3mm x 3mm (the L's
    box), center offset 0. (Tools > Convert > Create Custom Pad from Selected Outline did nothing
    with the region selected.)
    What the file stores: pads 2 and 3 have a size of 0.025 x 0.025 (10000 units, 1 mil) in
    their geometry block (block 4, bytes 21..29); pad 1 keeps its 2.000 x 1.000. Block 5 is 651
    bytes on pad 1, 666 bytes on pad 2 and empty (length 0) on pad 3. /Regions6/Data holds two regions on Top Layer, KIND 0, whose text part ends in
    PADINDEX=1 and PADINDEX=2: the copper of pads 2 and 3 (pad 2 cut at its upper corners, pad 3
    the L); their copies are in /ShapeBasedRegions6/Data, and a new stream, /CustomShapes/Data
    (186 bytes), appears. Our reader at origin/main c825c14 draws all three on the top copper
    without a note: pad 1 rounded, pad 2 a six-cornered outline from (19, 14.5) to (21, 15.5),
    pad 3 the L from (30, 14) to (33, 17).

paste.PcbDoc  (from one-resistor)
    By my own script, the board's paste mask expansion rule (Altium's PasteMaskExpansion, scope
    All, 0 before) set to -0.050; /Rules6/Data then reads EXPANSION=-1.9685mil. Three free SMD pads
    on Top Layer, rectangle 2.000 x 1.000, no net, at y 22 and x 15 (A), 25 (B) and 35 (C), and a
    region on Top Paste, KIND 0: (20, 5), (30, 5), (30, 9), (20, 9). Then by hand, in the pad's
    Properties panel (Pad Stack > Paste):
        A   nothing: paste by the rule (-0.05mm, Altium shows -14.5%)
        B   Manual Expansion, 0.1mm (Altium shows 32%)
        C   Manual Expansion (-0.05mm kept), Enabled unticked: no paste. Altium greys out
            Enabled while the paste follows the rule.
    R1's two pads keep the rule (-0.05). What the file stores in each pad's geometry block
    (block 4): bytes 86..90 the paste expansion (-0.050 on A and R1's pads, 0.100 on B, and
    -999990000 units, -99999 mil, on C), byte 101 the paste mode (1 by the rule, 2 Manual: B
    and C), byte 1 12 (0x0C) except 13 (0x0D) on C, and byte 185 3 except 1 on C. The region
    is an ordinary record in /Regions6/Data on layer byte 35 (Top Paste).

A variants project (one-resistor-based, with a part Not Fitted) was tried and not made: in a
project with only a board and no schematic, Altium 26.12.2's Project > Variants answered "Project
must be compiled first" even after Project > Validate PCB Project. The project was closed
without saving.

Paths inside the files
----------------------
Altium writes a few paths from the PC that saved a board into the file. After the last save, I
blanked their values: each value is overwritten with spaces of exactly the same length, so no
record or stream size changed and no other byte in the stream moved (one-ttf-text,
four-layer and small-board also lost their embedded font, and all-features its Arial, and the
boards re-saved for the mask expansion rule lost the Arial that saving put back, see above). The fields are:
    Board6 FILENAME                    the board's own save path
    Board6 2DCONFIGFULLFILENAME        the 2D view settings file (it contains the Windows login
                                       and the Altium install ID)
    Components6 SOURCEFOOTPRINTLIBRARY the footprint library (one-resistor only)
These are notes Altium keeps for itself; none of them changes the board's geometry. A reader must
expect them to be blank. Altium Designer 26.12.2 opens the blanked files normally.
