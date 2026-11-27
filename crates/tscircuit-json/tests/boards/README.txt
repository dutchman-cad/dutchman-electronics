Original test board made for Dutchman Electronics. Released under CC0 1.0 (public domain): https://creativecommons.org/publicdomain/zero/1.0/

(This line covers every board in this folder: each .tsx file and the .circuit.json made from it.)

Each board is a tscircuit source file (.tsx) we wrote for testing, and the Circuit JSON tscircuit
compiled from it with `tsci export <board>.tsx -f circuit-json`, using:

    @tscircuit/cli 0.1.2486, tscircuit 0.0.3288, circuit-json 0.0.560   (2026-11-23)

Lengths are millimetres, Y up, the board centred on (0, 0) unless an outline says otherwise.

outline-only       50 x 30 rectangle, thickness 1.6, two copper layers. Nothing else.
holes-cutout       50 x 30; a 3 mm hole at (-15, 0); a 10 x 10 cutout centred at (10, 0).
traces-vias        50 x 30; 0603 resistors R1 at (-12, 0) and R2 at (12, 0), joined by one top-layer
                   trace (width 0.15, from x -11.175 to 11.175 on y 0); a via at (0, 8),
                   hole 0.3, outer 0.6.
silkscreen         50 x 30; top silkscreen text "DUTCHMAN" at (0, 8), 2 mm high; a line from
                   (-20, -10) to (20, -10); a 6 x 4 rectangle at (-15, 0); a circle of radius 3 at
                   (15, 0).
components         60 x 40; R1 0603 at (-20, 10); C1 0805 at (-20, 0) turned 90 degrees; U1 SOIC-8
                   at (0, 5); J1 4-pin header (2.54 mm pitch, 1 mm plated holes) at (15, -10);
                   D1 0603 LED at (20, 10); R2 0603 on the bottom at (0, -12).
shaped-outline     an L shape: (0,0) (40,0) (40,15) (20,15) (20,30) (0,30); a 2 mm hole at (10, 10).
four-layer         50 x 30, four copper layers; R1 and R2 as in traces-vias, one trace.
