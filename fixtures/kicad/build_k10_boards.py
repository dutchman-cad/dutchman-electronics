"""Builds the KiCad 10 only test boards, in the style of build_boards.py (one feature per board).

Run it with KiCad 10's own Python (for example `"C:\\Program Files\\KiCad\\10.0\\bin\\python.exe"
fixtures/kicad/build_k10_boards.py crates/kicad-pcb/tests/boards`, or inside
docker.io/kicad/kicad:10.0), then fill the zones with fill_zones.py as run.sh does. The script only
places objects at the numbers written below and saves the boards; the reader learns only from the
saved files and the numbers in each board's README.txt. KiCad's Y axis points down; the numbers
below are the coordinates as KiCad shows them (millimeters, relative to the board's corner at page
position X0, Y0).
"""
import os
import sys

import pcbnew
from pcbnew import FromMM, VECTOR2I

OUT = sys.argv[1] if len(sys.argv) > 1 else "."
X0, Y0 = 100.0, 100.0  # where the board's top-left corner sits on the page


def p(x, y):
    return VECTOR2I(FromMM(X0 + x), FromMM(Y0 + y))


def new_board(copper_layers=2, thickness=1.6):
    b = pcbnew.CreateEmptyBoard()
    b.SetCopperLayerCount(copper_layers)
    b.GetDesignSettings().SetBoardThickness(FromMM(thickness))
    return b


def shape(b, kind, layer=pcbnew.Edge_Cuts, width=0.1):
    s = pcbnew.PCB_SHAPE(b)
    s.SetShape(kind)
    s.SetLayer(layer)
    s.SetWidth(FromMM(width))
    b.Add(s)
    return s


def circle(b, center, radius, layer=pcbnew.Edge_Cuts, width=0.1):
    s = shape(b, pcbnew.SHAPE_T_CIRCLE, layer, width)
    s.SetCenter(p(*center))
    s.SetEnd(p(center[0] + radius, center[1]))
    return s


def rect(b, a, c, layer=pcbnew.Edge_Cuts, width=0.1):
    s = shape(b, pcbnew.SHAPE_T_RECT, layer, width)
    s.SetStart(p(*a))
    s.SetEnd(p(*c))
    return s


def net(b, name):
    n = pcbnew.NETINFO_ITEM(b, name)
    b.Add(n)
    return n


def save(b, name):
    b.Save(os.path.join(OUT, name + ".kicad_pcb"))


# k10-zone-cutout (then fill_zones.py): the 50 x 30 mm rectangle with a circle cutout of radius 3
# centered on (25, 15), and a GND zone on the top layer, (5, 5) to (45, 25), that the cutout lies
# inside. Zone minimum width 0.25, zone clearance 0.3; the board's copper-to-edge clearance 0.5.
b = new_board()
rect(b, (0, 0), (50, 30))
circle(b, (25, 15), 3.0)
b.GetDesignSettings().m_CopperEdgeClearance = FromMM(0.5)
gnd = net(b, "GND")
z = pcbnew.ZONE(b)
z.SetLayer(pcbnew.F_Cu)
z.SetNet(gnd)
z.SetMinThickness(FromMM(0.25))
z.SetLocalClearance(FromMM(0.3))
b.Add(z)
ol = z.Outline()
ol.NewOutline()
for q in [(5, 5), (45, 5), (45, 25), (5, 25)]:
    ol.Append(FromMM(X0 + q[0]), FromMM(Y0 + q[1]))
save(b, "k10-zone-cutout")
