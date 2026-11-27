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


def text(b, content, at, layer=pcbnew.F_SilkS, parent=None):
    t = pcbnew.PCB_TEXT(parent or b)
    t.SetText(content)
    t.SetLayer(layer)
    t.SetTextSize(VECTOR2I(FromMM(1.5), FromMM(1.5)))
    t.SetTextThickness(FromMM(0.2))
    t.SetHorizJustify(pcbnew.GR_TEXT_H_ALIGN_LEFT)
    if layer in (pcbnew.B_SilkS, pcbnew.B_Cu):
        t.SetMirrored(True)
    t.SetPosition(p(*at))
    (parent or b).Add(t)
    return t


# k10-text-vars (then add_text_vars.py, which writes the project's text variables into
# k10-text-vars.kicad_pro): a 100 x 80 mm rectangle with a title block, one board property and texts
# that use KiCad's text variables. Title block: title "Text Variables", revision "B2", company
# "Dutchman Test", date "2026-11-27", comment 1 "First comment", comment 2 "Second comment". Board
# property BOARD_NOTE = "board property". Board texts, left-justified, 1.5 mm, pen 0.2, at x = 5,
# y = 5, 8, 11, ... (3 mm apart) on F.SilkS in the order of BOARD_TEXTS, then one on F.Cu below them
# and one on B.SilkS (mirrored) at (45, 75). Footprint J1 (value CONN-2, field MPN = ABC-123) on the
# top at (70, 20) and J2 (value CONN-3, field MPN = XYZ-9) flipped to the bottom at (70, 50), each
# with the user texts of FP_TEXTS at (0, 4), (0, 7), ... from the footprint's origin. Their fields
# (as seen before J2 is flipped, from the footprint's origin): MPN shown on the silkscreen at
# (-6, 0), 1.0 mm, pen 0.15, turned 90 degrees, right-justified; BUILD = "${PCB_NUMBER}/${REFERENCE}"
# shown on the silkscreen at (6, 0), 1.2 mm, pen 0.15, left-justified, top-aligned; SECRET =
# "hidden" on the silkscreen, hidden; FAB_NOTE = "fab note" shown on the Fab layer at (0, -3).
BOARD_TEXTS = [
    "${REVISION}",
    "Rev ${REVISION}",
    "${TITLE} ${REVISION}",
    "${PCB_NUMBER}",
    "${BOARD_ID}",
    "${LAYER}",
    "${NOSUCHVAR}",
    "Rev ${REVISION",
    "${COMPANY}",
    "${ISSUE_DATE}",
    "${COMMENT1} / ${COMMENT2}",
    "${J1:VALUE}",
    "${J1:MPN}",
    "${BOARD_NOTE}",
    "${FILENAME}",
    "${PROJECTNAME}",
    "${revision}",
    "$${REVISION}",
]
FP_TEXTS = ["${REFERENCE}", "${VALUE}", "${MPN}", "${REFERENCE}-${MPN}", "${LAYER}", "${REVISION}"]
b = new_board()
rect(b, (0, 0), (100, 80))
tb = b.GetTitleBlock()
tb.SetTitle("Text Variables")
tb.SetRevision("B2")
tb.SetCompany("Dutchman Test")
tb.SetDate("2026-11-27")
tb.SetComment(0, "First comment")
tb.SetComment(1, "Second comment")
props = b.GetProperties()
props["BOARD_NOTE"] = "board property"
b.SetProperties(props)
for i, content in enumerate(BOARD_TEXTS):
    text(b, content, (5, 5 + 3 * i))
text(b, "${LAYER} ${REVISION}", (5, 5 + 3 * len(BOARD_TEXTS)), pcbnew.F_Cu)
text(b, "${LAYER} ${REVISION}", (45, 75), pcbnew.B_SilkS)
for ref, value, mpn, at, flip in [
    ("J1", "CONN-2", "ABC-123", (70, 20), False),
    ("J2", "CONN-3", "XYZ-9", (70, 50), True),
]:
    fp = pcbnew.FOOTPRINT(b)
    fp.SetReference(ref)
    fp.SetValue(value)
    fp.SetFPID(pcbnew.LIB_ID("test", "Conn"))
    fp.SetField("MPN", mpn)
    fp.SetField("BUILD", "${PCB_NUMBER}/${REFERENCE}")
    fp.SetField("SECRET", "hidden")
    fp.SetField("FAB_NOTE", "fab note")
    silk = pcbnew.F_SilkS
    for name, spot, size, angle, h, v in [
        ("MPN", (-6, 0), 1.0, 90.0, pcbnew.GR_TEXT_H_ALIGN_RIGHT, pcbnew.GR_TEXT_V_ALIGN_CENTER),
        ("BUILD", (6, 0), 1.2, 0.0, pcbnew.GR_TEXT_H_ALIGN_LEFT, pcbnew.GR_TEXT_V_ALIGN_TOP),
        ("SECRET", (0, -6), 1.0, 0.0, pcbnew.GR_TEXT_H_ALIGN_CENTER, pcbnew.GR_TEXT_V_ALIGN_CENTER),
    ]:
        f = fp.GetField(name)
        f.SetLayer(silk)
        f.SetPosition(VECTOR2I(FromMM(spot[0]), FromMM(spot[1])))
        f.SetTextSize(VECTOR2I(FromMM(size), FromMM(size)))
        f.SetTextThickness(FromMM(0.15))
        f.SetTextAngleDegrees(angle)
        f.SetHorizJustify(h)
        f.SetVertJustify(v)
    fp.GetField("SECRET").SetVisible(False)
    fab = fp.GetField("FAB_NOTE")
    fab.SetLayer(pcbnew.F_Fab)
    fab.SetPosition(VECTOR2I(0, FromMM(-3)))
    for i, content in enumerate(FP_TEXTS):
        t = text(b, content, (0, 0), parent=fp)
        t.SetPosition(VECTOR2I(0, FromMM(4 + 3 * i)))
    b.Add(fp)
    fp.SetPosition(p(*at))
    if flip:
        fp.Flip(fp.GetPosition(), False)
save(b, "k10-text-vars")
