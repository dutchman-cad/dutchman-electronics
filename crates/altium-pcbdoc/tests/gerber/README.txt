Altium's own Gerber output for our test boards (Altium Designer 26.12.2, File > Fabrication
Outputs > Gerber Files, millimeters, 4.4 format), to check what the reader draws against Altium.

plane-polygon-connect.GP1
    Layer 1 (the GND plane) of tests/boards/plane-polygon-connect.PcbDoc, exported on 2026-11-28
    from a copy of the committed board, which was closed without saving. Only Layer 1 was added
    to Altium's default layer choice; the other files were not kept. The file is negative
    (TF.FilePolarity,Negative): every flash is a hole in the plane copper. Its only identifier,
    the random TF.SameCoordinates GUID Altium writes, was zeroed (00000000-0000-0000-0000-
    000000000000, the same length), so nothing in it points back to the PC that made it; a privacy
    scan found nothing else.

    The three GND pads (round 1.600, plated hole 0.800) at y 25 and the rules on the board:
        R (6, 25)    plane connect Relief (the board rule: expansion 20mil, conductor 10mil,
                     4 entries): one flash of D16, aperture macro THD16, primitive 7 (thermal):
                     outer diameter 2.324, inner diameter 1.816, gap 0.254, rotation 45.
                     So the plane is cleared in a ring from radius 0.908 to 1.162 (0.254 wide),
                     interrupted by four copper spokes 0.254 wide at 45, 135, 225 and 315
                     degrees; the inner diameter is the hole plus twice the expansion
                     (0.800 + 2 x 0.508).
        D (12, 25)   plane connect Direct (rule PlaneConnect_D): no flash at all; the plane
                     copper runs into the hole, with no gap and no clearance.
        N (18, 25)   plane connect No Connect (rule PlaneConnect_N): one flash of D15, a circle
                     1.400 across: the hole plus twice the plane clearance (0.800 + 2 x 0.300),
                     no spokes.
    The rest of the file is four-layer-planes' own plane: the pull-back track D14 (1.016 wide)
    along the board edge, the GND vias' reliefs D19 (1.824 / 1.316, at (10, 10)), the GND pad's
    relief D17 (2.424 / 1.916, at (40, 10)) and clearances D18 (0.900) around the VCC via at
    (10, 20) and the free via at (35, 20).
