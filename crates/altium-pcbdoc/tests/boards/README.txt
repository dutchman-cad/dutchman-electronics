Original test board made for Dutchman Electronics. Released under CC0 1.0 (public domain): https://creativecommons.org/publicdomain/zero/1.0/

(This line covers every board in this folder: empty-outline, outline-with-hole, one-track and
one-resistor.)

Made in Altium Designer 26.12.2 (build 26.12.2.9), saved as "PCB Binary Files (*.PcbDoc)",
on 2026-11-23. Each board was drawn with typed or grid-snapped coordinates (snap grid 1 mm), so
every number below is exact.

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


Paths inside the files
----------------------
Altium writes a few paths from the PC that saved a board into the file. After the last save, I
blanked their values: each value is overwritten with spaces of exactly the same length, so no
record, stream or container size changed and no other byte moved. The fields are:
    Board6 FILENAME                    the board's own save path
    Board6 2DCONFIGFULLFILENAME        the 2D view settings file (it contains the Windows login
                                       and the Altium install ID)
    Components6 SOURCEFOOTPRINTLIBRARY the footprint library (one-resistor only)
These are notes Altium keeps for itself; none of them changes the board's geometry. A reader must
expect them to be blank. Altium Designer 26.12.2 opens the blanked files normally.
