Original test board made for Dutchman Electronics. Released under CC0 1.0 (public domain): https://creativecommons.org/publicdomain/zero/1.0/

(This line covers every board in this folder: empty-outline, outline-with-hole, one-track,
one-resistor, one-via, one-plated-pad, one-rotated-pad, one-oval-pad, one-bottom-track,
one-arc, one-fill, one-region, one-pour, one-text, one-rotated-text,
one-bottom-text, one-step-resistor, one-rotated-resistor, one-bottom-resistor, one-net,
one-offset-step-resistor, one-rotated-step-resistor, one-bottom-step-resistor and
one-rotated-fill.)

Made in Altium Designer 26.12.2 (build 26.12.2.9), saved as "PCB Binary Files (*.PcbDoc)".
The first four boards (empty-outline, outline-with-hole, one-track, one-resistor) were drawn by
hand on 2026-11-23 with typed or grid-snapped coordinates (snap grid 1 mm); the other six were
made by a script on 2026-11-24 (see "Scripted boards" below). Every number below is exact.

All numbers are in millimetres, as Altium shows them in its mm display. Coordinates are relative
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
       centre (10.000, 15.000), rotation 0
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
    centre (25.000, 15.000), rotation 0, on the top side
    no nets, no tracks

Footprint R0603-TEST (I drew it myself; numbers relative to the footprint origin, which is the
component centre):
    pad 1: Top Layer only (SMD), rectangle 0.800 x 1.000, centre (-0.800, 0)
    pad 2: Top Layer only (SMD), rectangle 0.800 x 1.000, centre (+0.800, 0)
    3D body: an extruded box I drew myself (no STEP file, nothing downloaded),
        outline corners (-0.8, -0.4), (0.8, -0.4), (0.8, 0.4), (-0.8, 0.4): 1.600 x 0.800
        standoff 0, overall height 0.450 (from the board's top surface up)
        Altium layer "Top 3D Body"
So on the board the pads are centred at (24.2, 15) and (25.8, 15), and the box spans
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
    one via, centre (25.000, 15.000), diameter 1.000, hole 0.500
    from Top Layer to Bottom Layer (a through via)

one-plated-pad.PcbDoc
    one free pad, layer Multi-Layer, designator "1", rotation 0
    centre (25.000, 15.000), round, 2.000 x 2.000 on top, middle and bottom
    hole: round, diameter 1.000, plated

one-rotated-pad.PcbDoc
    one free pad, layer Top Layer only (SMD), designator "1", no hole
    centre (25.000, 15.000), rectangle 2.000 x 1.000 (X x Y before rotation)
    rotation 30 degrees (counter-clockwise)

one-oval-pad.PcbDoc
    one free pad, layer Top Layer only (SMD), designator "1", no hole, rotation 0
    centre (25.000, 15.000), Altium shape "Round" with 2.000 x 1.000, which Altium draws as an
    oval (a slot shape, 2.000 long along X, 1.000 wide)

one-bottom-track.PcbDoc
    one track, layer Bottom Layer
    start (10.000, 15.000), end (20.000, 15.000): 10.000 long, straight along +X
    width 0.500

one-arc.PcbDoc
    one arc (a curved track), layer Top Layer
    centre (25.000, 15.000), radius 5.000, width 0.500
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
The STEP model (box-1608.step) is my own: a box 1.600 x 0.800 x 0.450, centred on its origin in
X and Y, from z = 0 to z = 0.450, written by this repository's board-step crate (CC0 like the
board). Altium was given no rotation and no standoff; the script read back the body's extent:
x 24.2 to 25.8, y 14.6 to 15.4, height 0.450 above the top surface.

Scripted boards, fourth batch: one-rotated-resistor, one-bottom-resistor, one-net
----------------------------------------------------------------------------------
Made on 2026-11-24 by my own script, each from a file copy of a board above with exactly one
change; the script read every number back and logged it.

one-rotated-resistor.PcbDoc  (from one-resistor)
    R1 rotated to 90 degrees (counter-clockwise) about its centre, which stays at (25, 15);
    still on the top side. Its pads moved with it:
        pad 1 at (25.000, 14.200), pad 2 at (25.000, 15.800), both with rotation 90
    Its extruded box turned with it: x 24.6 to 25.4, y 14.2 to 15.8, still 0.450 high.
one-bottom-resistor.PcbDoc  (from one-resistor)
    R1 flipped to the bottom side, centre still (25, 15). Altium itself then shows R1 with
    rotation 180 (I set only the side). Its pads are on Bottom Layer and mirrored:
        pad 1 at (25.800, 15.000), pad 2 at (24.200, 15.000), both with rotation 180
    Its extruded box went to the bottom side too (Altium marks it as projecting from the bottom):
    x 24.2 to 25.8, y 14.6 to 15.4, standoff 0, height 0.450.
one-net.PcbDoc  (from one-track)
    the track (Top Layer, (10, 15) to (20, 15), width 0.500) now belongs to a net named "GND";
    nothing else has a net.

Scripted boards, fifth batch: an off-centre STEP model, rotated and flipped; a rotated fill;
TrueType text
-------------------------------------------------------------------------------------------
Made on 2026-11-24 by my own script; every number was read back and logged.

The STEP model box-offset.step is my own, written by this repository's board-step crate (CC0
like the boards): a box 1.600 x 0.800 x 0.450 that lies on the +X side of its origin, x 0 to
1.6, y -0.4 to 0.4, z 0 to 0.45. Being off-centre, it shows how Altium turns and flips models.

one-offset-step-resistor.PcbDoc  (from one-resistor)
    R1's extruded box replaced by a 3D body holding box-offset.step, its origin on R1's centre
    (25, 15), embedded in the board. Top side, rotation 0. The body spans x 25 to 26.6,
    y 14.6 to 15.4, height 0.450.
one-rotated-step-resistor.PcbDoc  (the same, then R1 rotated to 90 degrees)
    R1 at (25, 15), rotation 90; pads at (25, 14.2) and (25, 15.8). The body spans x 24.6 to
    25.4, y 15 to 16.6: the +X box turned onto +Y.
one-bottom-step-resistor.PcbDoc  (the same as one-offset-step-resistor, then R1 flipped)
    Altium turns a part about the middle of everything it holds, model included, so flipping
    R1 to the bottom moved its centre to (25.400, 15.000) and gave it rotation 180 (I set only
    the side). Pads on Bottom Layer at (26.2, 15) and (24.6, 15), rotation 180. The body spans
    x 23.8 to 25.4, y 14.6 to 15.4, height 0.450, projecting from the bottom.
one-rotated-fill.PcbDoc  (from empty-outline)
    the fill of one-fill (corners (20, 10) and (30, 20) before rotation) with rotation 30
    degrees, counter-clockwise about its centre (25, 15); its bounding box is x 18.170 to
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

Scripted boards, sixth batch: an off-centre extruded body, top and flipped
--------------------------------------------------------------------------
Made on 2026-11-24 by my own script, each from a file copy of one-resistor.PcbDoc with R1's
extruded box given a new outline; every number was read back and logged. The outline is
lopsided in X and in Y, so a flip shows its mirror axis: about R1's centre it is
(0, -0.4), (1.6, -0.4), (1.6, 0), (0, 0.4), from standoff 0 up to 0.450.

one-offset-extruded-resistor.PcbDoc
    R1 at (25, 15), rotation 0, top side. The body's outline is (26.6, 14.6), (26.6, 15),
    (25, 15.4), (25, 14.6); its box x 25 to 26.6, y 14.6 to 15.4, height 0.450.
one-bottom-extruded-resistor.PcbDoc  (the same, then R1 flipped to the bottom)
    Altium turns a part about the middle of everything it holds, so R1's centre moved to
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
    R2 got a copy of R1's 3D body: the same embedded box-1608.step, centred on R2 (body x 34.2
    to 35.8, y 14.6 to 15.4, height 0.450). The board holds the model once: one record in
    Models/Data and one stream Models/0, and both bodies name the same MODELID.
two-step-models.PcbDoc
    R2 got a new 3D body holding the embedded box-offset.step instead, its origin on R2's
    centre (body x 35 to 36.6, y 14.6 to 15.4, height 0.450). R1 keeps box-1608.step. The
    board holds two models: record 0 of Models/Data is box-offset.step, stored in Models/0 (x 0
    to 1.6 once unpacked), and record 1 is box-1608.step, stored in Models/1 (x -0.8 to 0.8).
    So the n-th record (from 0) is stored in Models/n; Altium put the newer model first.

Scripted boards, eighth batch: a STEP model turned on its part; bottom-side parts at 90
---------------------------------------------------------------------------------------
Made on 2026-11-24 by my own script; every number was read back and logged.

The first three are made from one-resistor. R1's extruded box was replaced by a 3D body holding
the embedded box-offset.step (fifth batch), its origin first put 0.7 above R1's centre. Then the
model was turned 30 degrees on the part: R1 was turned to 30 degrees with its body, the body
was taken off R1, R1 was turned back to 0 and the body put back. On the part the model's origin
is then at (-0.350, 0.606) from R1's centre (the point (0, 0.7) turned by 30 degrees), and the
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
    an arc on Top Overlay, centre (25, 15), radius 3.000, from 30 to 300 degrees
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
        corner arcs     centre (47, 3) from 270 to 360 degrees, centre (47, 27) from 0 to 90,
                        centre (3, 27) from 90 to 180, centre (3, 3) from 180 to 270
                        (counter-clockwise)
    One round board cutout: centre (40, 15), radius 2.000.
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
        at its mouth (convex)   centre (19, 29), radius 1.000, from 0 to 90 degrees
                                centre (31, 29), radius 1.000, from 90 to 180 degrees
        at its bottom (concave) centre (22, 22), radius 2.000, from 180 to 270 degrees
                                centre (28, 22), radius 2.000, from 270 to 360 degrees
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
        R1  centre (25, 15), rotation 0, top side (as on one-resistor-silkscreen)
        R2  centre (13, 22), rotation 0, top side
        R3  centre (37, 8), copied to (37, 8) and then flipped: rotation 180, bottom side;
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
    an arc on Top Overlay, centre (40, 18), radius 3.000, from 0 to 270 degrees
    (counter-clockwise), width 0.200, both over the top pour; a line on Bottom Overlay
    (10, 4) to (40, 4), width 0.150, over the bottom pour.

Sixteenth batch: solder mask and silkscreen colours
---------------------------------------------------
Made on 2026-11-24 by hand, each from a file copy of one-resistor.PcbDoc: in the 3D view, the
View Configuration panel, one board colour changed, then the board was saved. Altium keeps these
colours with the board in the 3DCONFIGURATION setting of /Board6/Data (they are how its own 3D
view draws the board). Colours below are red, green, blue from 0 to 255; nothing else changed.

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
Altium's own list, which filled in its material, colour and thickness; then the stack and the
board were saved. Altium shows the finish layers in the stack and counts them in its total
thickness; they are not among the stack's layers in the plain /Board6/Data keys.

one-enig-resistor.PcbDoc
    Top Surface Finish and Bottom Surface Finish: process ENIG, material "Nickel, Gold", colour
    #FFFFC400 (opaque; red 255, green 196, blue 0), thickness 0.004. Altium's total thickness:
    1.608.
one-hasl-resistor.PcbDoc
    Top Surface Finish and Bottom Surface Finish: process HASL, material "PbSn", colour
    #FFF2F2F2 (opaque; 242, 242, 242), thickness 0.020. Altium's total thickness: 1.640.

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
