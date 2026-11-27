"""Turns the three-point polygons of a pad-arcs board into one arc each, through KiCad.

Our own code. KiCad's board scripting cannot put an arc inside a custom pad's polygon, but the file
format allows `(arc (start) (mid) (end))` among a polygon's points. This edits the saved text:
every `(pts (xy A) (xy B) (xy C))` becomes `(pts (arc (start A) (mid B) (end C)))`, then loads the
board in KiCad and saves it again, so KiCad itself writes the final file.

Usage (inside KiCad 9): add_pad_arcs.py board.kicad_pcb
"""
import re
import sys

import pcbnew

path = sys.argv[1]
text = open(path).read()
num = r"(-?[0-9.e+-]+)"
xy = r"\(xy " + num + " " + num + r"\)"
pattern = re.compile(r"\(pts\s*" + xy + r"\s*" + xy + r"\s*" + xy + r"\s*\)")
text, n = pattern.subn(
    lambda m: "(pts (arc (start {} {}) (mid {} {}) (end {} {})))".format(*m.groups()), text
)
assert n == 4, n
open(path, "w").write(text)
board = pcbnew.LoadBoard(path)
board.Save(path)
