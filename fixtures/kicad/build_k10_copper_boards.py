"""Builds the KiCad 10 copper-shape test boards, in the style of build_boards.py (one feature per
board).

Run it with KiCad 10's own Python (for example `"C:\\Program Files\\KiCad\\10.0\\bin\\python.exe"
fixtures/kicad/build_k10_copper_boards.py crates/kicad-pcb/tests/boards`, or inside
docker.io/kicad/kicad:10.0), then fill the zones with fill_zones.py as run.sh does. The script
only places objects at the numbers written below and saves the boards; the reader learns only
from the saved files and the numbers in each board's README.txt. KiCad's Y axis points down; the
numbers below are the coordinates as KiCad shows them (millimeters, relative to the board's corner
at page position X0, Y0).
"""
import os
import sys

import pcbnew
from pcbnew import FromMM, VECTOR2I

OUT = sys.argv[1] if len(sys.argv) > 1 else "."
X0, Y0 = 100.0, 100.0  # where the board's top-left corner sits on the page


def p(x, y):
    return VECTOR2I(FromMM(X0 + x), FromMM(Y0 + y))


def mm(x, y):
    return VECTOR2I(FromMM(x), FromMM(y))


def new_board():
    b = pcbnew.CreateEmptyBoard()
    b.SetCopperLayerCount(2)
    b.GetDesignSettings().SetBoardThickness(FromMM(1.6))
    return b


def shape(parent, kind, layer, width, filled=False):
    s = pcbnew.PCB_SHAPE(parent)
    s.SetShape(kind)
    s.SetLayer(layer)
    s.SetWidth(FromMM(width))
    s.SetFilled(filled)
    parent.Add(s)
    return s


def outline(b):
    s = shape(b, pcbnew.SHAPE_T_RECT, pcbnew.Edge_Cuts, 0.1)
    s.SetStart(p(0, 0))
    s.SetEnd(p(50, 30))


def footprint(b, ref, at, rot, flip, make):
    fp = pcbnew.FOOTPRINT(b)
    fp.SetReference(ref)
    fp.SetFPID(pcbnew.LIB_ID("test", "Shapes"))
    make(fp)
    b.Add(fp)
    fp.SetPosition(p(*at))
    if rot:
        fp.SetOrientationDegrees(rot)
    if flip:
        fp.Flip(fp.GetPosition(), False)
    return fp


def save(b, name):
    b.Save(os.path.join(OUT, name + ".kicad_pcb"))


# k10-copper-shapes: shapes drawn on copper, on the 50 x 30 mm rectangle (two layers, 1.6 mm).
# Filled on the board: an L polygon on F.Cu (5,5) (15,5) (15,8) (8,8) (8,12) (5,12), no stroke; a
# rectangle on F.Cu (20,5) to (28,12) with a 0.2 stroke; a circle on F.Cu about (36,8.5), radius 3,
# no stroke; a triangle on B.Cu (42,4) (47,4) (44.5,10) with a 0.3 stroke. Not filled: a rectangle
# on F.Cu (5,16) to (12,22) with a 0.25 stroke. In footprints: U1 on the top at (20,22), not
# turned, with a filled circle of radius 1.5 about its origin and a filled rectangle (2,-1.5) to
# (5,1.5), both on F.Cu, no stroke; U2 flipped to the bottom at (38,22), turned 30, with a filled
# triangle (0,-2) (3,1) (-3,1) on its copper (B.Cu once flipped), no stroke.
b = new_board()
outline(b)
F, B = pcbnew.F_Cu, pcbnew.B_Cu
s = shape(b, pcbnew.SHAPE_T_POLY, F, 0, True)
s.SetPolyPoints([p(5, 5), p(15, 5), p(15, 8), p(8, 8), p(8, 12), p(5, 12)])
s = shape(b, pcbnew.SHAPE_T_RECT, F, 0.2, True)
s.SetStart(p(20, 5))
s.SetEnd(p(28, 12))
s = shape(b, pcbnew.SHAPE_T_CIRCLE, F, 0, True)
s.SetCenter(p(36, 8.5))
s.SetEnd(p(39, 8.5))
s = shape(b, pcbnew.SHAPE_T_POLY, B, 0.3, True)
s.SetPolyPoints([p(42, 4), p(47, 4), p(44.5, 10)])
s = shape(b, pcbnew.SHAPE_T_RECT, F, 0.25, False)
s.SetStart(p(5, 16))
s.SetEnd(p(12, 22))


def u1(fp):
    c = shape(fp, pcbnew.SHAPE_T_CIRCLE, F, 0, True)
    c.SetCenter(mm(0, 0))
    c.SetEnd(mm(1.5, 0))
    r = shape(fp, pcbnew.SHAPE_T_RECT, F, 0, True)
    r.SetStart(mm(2, -1.5))
    r.SetEnd(mm(5, 1.5))


def u2(fp):
    t = shape(fp, pcbnew.SHAPE_T_POLY, F, 0, True)
    t.SetPolyPoints([mm(0, -2), mm(3, 1), mm(-3, 1)])


footprint(b, "U1", (20, 22), 0.0, False, u1)
footprint(b, "U2", (38, 22), 30.0, True, u2)
save(b, "k10-copper-shapes")


def fp_zone(fp, corners, layer=pcbnew.F_Cu, rule_area=False):
    z = pcbnew.ZONE(fp)
    z.SetLayer(layer)
    if rule_area:
        z.SetIsRuleArea(True)
        z.SetDoNotAllowTracks(True)
        z.SetDoNotAllowVias(True)
    else:
        z.SetMinThickness(FromMM(0.25))
        z.SetLocalClearance(FromMM(0.5))
    o = z.Outline()
    o.NewOutline()
    for x, y in corners:
        o.Append(FromMM(x), FromMM(y))
    fp.Add(z)
    return z


# k10-footprint-zone (then fill_zones.py, which fills the footprints' zones too): copper zones
# inside footprints, on the 50 x 30 mm rectangle (two layers, 1.6 mm). Each of U1 and U2 holds a
# zone on its copper layer, outline (-6,-5) (6,-5) (6,5) (-6,5) from its origin, no net, minimum
# width 0.25, clearance 0.5, and a plain hole of 1.0 at its origin that the fill keeps clear of.
# U2 also holds a keep-out rule area (no tracks, no vias) (-3,6) (3,6) (3,8) (-3,8). U1 is on the
# top at (13,15), not turned; U2 is flipped to the bottom at (36,15), turned 30.
b = new_board()
outline(b)


def zone_part(fp, keepout):
    d = pcbnew.PAD(fp)
    d.SetNumber("")
    d.SetShape(pcbnew.PAD_SHAPE_CIRCLE)
    d.SetAttribute(pcbnew.PAD_ATTRIB_NPTH)
    d.SetSize(mm(1.0, 1.0))
    d.SetDrillSize(mm(1.0, 1.0))
    d.SetLayerSet(d.UnplatedHoleMask())
    d.SetPosition(mm(0, 0))
    fp.Add(d)
    fp_zone(fp, [(-6, -5), (6, -5), (6, 5), (-6, 5)])
    if keepout:
        fp_zone(fp, [(-3, 6), (3, 6), (3, 8), (-3, 8)], rule_area=True)


footprint(b, "U1", (13, 15), 0.0, False, lambda fp: zone_part(fp, False))
footprint(b, "U2", (36, 15), 30.0, True, lambda fp: zone_part(fp, True))
save(b, "k10-footprint-zone")
