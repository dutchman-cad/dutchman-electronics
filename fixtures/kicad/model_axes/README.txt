How we measured the way KiCad places a 3D model (offset, rotate) on a top and a bottom footprint.

1. python3 mkbox.py box.step      writes a STEP box, x 0..4, y 0..1, z 0..0.5 mm (our own code)
2. mkboards.py (inside KiCad 9)   writes boards with one footprint carrying that box, with offsets
   and rotations, on the top and flipped to the bottom
3. kicad-cli pcb export vrml      writes each board's VRML; the box's Transform holds the
   rotation (axis and angle) and translation KiCad applies
4. rot.py compares the axis/angle in the VRML with turns about X, Y, Z in each order

Result: the offset (x, y, z) is x right, y up, z out of the board face the part sits on
(bottom: z points down, and the file's footprint angle of 180 after flipping is taken in).
The rotation (rx, ry, rz) is a turn of -rx about X, then -ry about Y, then -rz about Z.
