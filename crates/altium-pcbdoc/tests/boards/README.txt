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

Paths inside the files
----------------------
Altium writes a few paths from the PC that saved a board into the file. After the last save, I
blanked their values: each value is overwritten with spaces of exactly the same length, so no
record or stream size changed and no other byte in the stream moved (one-ttf-text also lost
its embedded font, see above). The fields are:
    Board6 FILENAME                    the board's own save path
    Board6 2DCONFIGFULLFILENAME        the 2D view settings file (it contains the Windows login
                                       and the Altium install ID)
    Components6 SOURCEFOOTPRINTLIBRARY the footprint library (one-resistor only)
These are notes Altium keeps for itself; none of them changes the board's geometry. A reader must
expect them to be blank. Altium Designer 26.12.2 opens the blanked files normally.
