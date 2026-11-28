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


def line(b, a, c, layer=pcbnew.Edge_Cuts, width=0.1):
    s = shape(b, pcbnew.SHAPE_T_SEGMENT, layer, width)
    s.SetStart(p(*a))
    s.SetEnd(p(*c))
    return s


def arc(b, start, mid, end, layer=pcbnew.Edge_Cuts, width=0.1):
    s = shape(b, pcbnew.SHAPE_T_ARC, layer, width)
    s.SetArcGeometry(p(*start), p(*mid), p(*end))
    return s


def poly(b, pts, layer=pcbnew.Edge_Cuts, width=0.1):
    s = shape(b, pcbnew.SHAPE_T_POLY, layer, width)
    s.SetPolyPoints([p(*q) for q in pts])
    return s


def mm(x, y):
    return VECTOR2I(FromMM(x), FromMM(y))


def pad(fp, number, x, size, kind, attrib=pcbnew.PAD_ATTRIB_SMD, drill=None):
    d = pcbnew.PAD(fp)
    d.SetNumber(number)
    d.SetShape(kind)
    d.SetAttribute(attrib)
    d.SetSize(mm(*size))
    if attrib == pcbnew.PAD_ATTRIB_PTH:
        d.SetLayerSet(d.PTHMask())
        d.SetDrillSize(mm(drill, drill))
    elif attrib == pcbnew.PAD_ATTRIB_NPTH:
        d.SetLayerSet(d.UnplatedHoleMask())
        d.SetDrillSize(mm(drill, drill))
    else:
        d.SetLayerSet(d.SMDMask())
    d.SetPosition(mm(x, 0))
    fp.Add(d)
    return d


def via(b, at, net_, size=0.8, drill=0.4, span=None, tenting=None, micro=False):
    v = pcbnew.PCB_VIA(b)
    v.SetPosition(p(*at))
    v.SetWidth(FromMM(size))
    v.SetDrill(FromMM(drill))
    if span is None:
        v.SetViaType(pcbnew.VIATYPE_THROUGH)
    else:
        v.SetViaType(pcbnew.VIATYPE_MICROVIA if micro else pcbnew.VIATYPE_BLIND)
        v.SetLayerPair(*span)
    if tenting is not None:
        v.SetFrontTentingMode(tenting[0])
        v.SetBackTentingMode(tenting[1])
    v.SetNet(net_)
    b.Add(v)
    return v


# k10-all-features (then fill_zones.py, add_stackup.py with stackup-all-features.sexp,
# add_pad_arcs.py with 1 arc, embed_fonts.py for "TrueType 789", and add_text_vars.py with
# PCB_NUMBER = DN-0100): the KiCad twin of the Altium all-features board, one of everything the
# reader handles. k10-all-features.README.txt gives every number; in short:
# - a 100 x 70 mm outline with corners rounded (radius 3), a circle cutout (radius 2 at (90, 60))
#   and a rectangle cutout (80, 6) to (86, 12); four copper layers, 1.6 mm, a stack-up with ENIG;
# - U1 at (30, 8): ten pads 4 mm apart from x = -18, one of each shape (circle, rect, oval,
#   roundrect, chamfered, trapezoid, custom polygon, custom with an arc, plated hole, plain hole);
# - a GND zone on F.Cu (5, 18) to (45, 38) with a thermal-relief pad J1 at (15, 28) and a SIG via
#   at (30, 28);
# - tracks and an arc on F.Cu and B.Cu along y = 45, joined by a via at (30, 45), and six vias
#   along y = 50 (tenting, blind, buried, micro);
# - silkscreen shapes and texts (stroke, TrueType, text variables, one on the bottom);
# - R1 on the top at (15, 58) and R2 on the bottom at (35, 58), turned 90, with models and a
#   shown MPN field;
# - a filled copper trapezoid on F.Cu and a footprint U3 at (80, 50) holding a copper zone.
R2 = 0.70710678
b = new_board(4, 1.6)
line(b, (3, 0), (97, 0))
arc(b, (97, 0), (97 + 3 * R2, 3 - 3 * R2), (100, 3))
line(b, (100, 3), (100, 67))
arc(b, (100, 67), (97 + 3 * R2, 67 + 3 * R2), (97, 70))
line(b, (97, 70), (3, 70))
arc(b, (3, 70), (3 - 3 * R2, 67 + 3 * R2), (0, 67))
line(b, (0, 67), (0, 3))
arc(b, (0, 3), (3 - 3 * R2, 3 - 3 * R2), (3, 0))
circle(b, (90, 60), 2.0)
rect(b, (80, 6), (86, 12))
tb = b.GetTitleBlock()
tb.SetTitle("All Features")
tb.SetRevision("C1")
tb.SetCompany("Dutchman Test")
gnd = net(b, "GND")
sig = net(b, "SIG")

fp = pcbnew.FOOTPRINT(b)
fp.SetReference("U1")
fp.SetValue("SHAPES")
fp.SetFPID(pcbnew.LIB_ID("test", "Shapes"))
pad(fp, "1", -18, (1.5, 1.5), pcbnew.PAD_SHAPE_CIRCLE)
pad(fp, "2", -14, (2.0, 1.5), pcbnew.PAD_SHAPE_RECT)
pad(fp, "3", -10, (2.5, 1.5), pcbnew.PAD_SHAPE_OVAL)
d = pad(fp, "4", -6, (2.0, 1.5), pcbnew.PAD_SHAPE_ROUNDRECT)
d.SetRoundRectRadiusRatio(0.25)
d = pad(fp, "5", -2, (2.0, 1.5), pcbnew.PAD_SHAPE_CHAMFERED_RECT)
d.SetChamferRectRatio(0.25)
d.SetChamferPositions(1 | 8)  # top-left and bottom-right
d = pad(fp, "6", 2, (2.0, 1.5), pcbnew.PAD_SHAPE_TRAPEZOID)
d.SetDelta(mm(0, 0.5))
d = pad(fp, "7", 6, (0.8, 0.8), pcbnew.PAD_SHAPE_CUSTOM)
d.SetAnchorPadShape(pcbnew.F_Cu, pcbnew.PAD_SHAPE_RECT)
L_SHAPE = [(0, 0), (1.5, 0), (1.5, 0.6), (0.6, 0.6), (0.6, 1.5), (0, 1.5)]
d.AddPrimitivePoly(pcbnew.F_Cu, [mm(x, y) for x, y in L_SHAPE], 0, True)
d = pad(fp, "8", 10, (1.0, 1.0), pcbnew.PAD_SHAPE_CUSTOM)
d.SetAnchorPadShape(pcbnew.F_Cu, pcbnew.PAD_SHAPE_CIRCLE)
# A D: add_pad_arcs.py turns these three points into one arc through (1.2, 0).
d.AddPrimitivePoly(pcbnew.F_Cu, [mm(0, -1.2), mm(1.2, 0), mm(0, 1.2)], 0, True)
pad(fp, "9", 14, (1.6, 1.6), pcbnew.PAD_SHAPE_CIRCLE, pcbnew.PAD_ATTRIB_PTH, 0.8)
pad(fp, "", 18, (1.0, 1.0), pcbnew.PAD_SHAPE_CIRCLE, pcbnew.PAD_ATTRIB_NPTH, 1.0)
b.Add(fp)
fp.SetPosition(p(30, 8))

z = pcbnew.ZONE(b)
z.SetLayer(pcbnew.F_Cu)
z.SetNet(gnd)
z.SetMinThickness(FromMM(0.25))
z.SetLocalClearance(FromMM(0.5))
z.SetThermalReliefGap(FromMM(0.5))
z.SetThermalReliefSpokeWidth(FromMM(0.5))
z.SetPadConnection(pcbnew.ZONE_CONNECTION_THERMAL)
b.Add(z)
ol = z.Outline()
ol.NewOutline()
for q in [(5, 18), (45, 18), (45, 38), (5, 38)]:
    ol.Append(FromMM(X0 + q[0]), FromMM(Y0 + q[1]))
fp = pcbnew.FOOTPRINT(b)
fp.SetReference("J1")
fp.SetValue("PIN")
fp.SetFPID(pcbnew.LIB_ID("test", "Pin"))
pad(fp, "1", 0, (1.6, 1.6), pcbnew.PAD_SHAPE_CIRCLE, pcbnew.PAD_ATTRIB_PTH, 0.8).SetNet(gnd)
b.Add(fp)
fp.SetPosition(p(15, 28))
via(b, (30, 28), sig)

for a, c, layer, w in [
    ((5, 45), (20, 45), pcbnew.F_Cu, 0.25),
    ((30, 45), (40, 45), pcbnew.B_Cu, 0.3),
]:
    t = pcbnew.PCB_TRACK(b)
    t.SetStart(p(*a))
    t.SetEnd(p(*c))
    t.SetLayer(layer)
    t.SetWidth(FromMM(w))
    t.SetNet(sig)
    b.Add(t)
t = pcbnew.PCB_ARC(b)
t.SetStart(p(20, 45))
t.SetMid(p(25, 40))
t.SetEnd(p(30, 45))
t.SetLayer(pcbnew.F_Cu)
t.SetWidth(FromMM(0.25))
t.SetNet(sig)
b.Add(t)
via(b, (30, 45), sig)
T, N = pcbnew.TENTING_MODE_TENTED, pcbnew.TENTING_MODE_NOT_TENTED
via(b, (5, 50), sig)
via(b, (10, 50), sig, tenting=(T, T))
via(b, (15, 50), sig, tenting=(N, N))
via(b, (20, 50), sig, span=(pcbnew.F_Cu, pcbnew.In1_Cu))
via(b, (25, 50), sig, span=(pcbnew.In1_Cu, pcbnew.In2_Cu))
via(b, (30, 50), sig, size=0.5, drill=0.2, span=(pcbnew.F_Cu, pcbnew.In1_Cu), micro=True)

SILK = pcbnew.F_SilkS
line(b, (50, 20), (60, 20), SILK, 0.15)
arc(b, (65, 20), (65 + 3 * R2, 23 - 3 * R2), (68, 23), SILK, 0.2)
circle(b, (75, 22), 2.0, SILK, 0.15)
rect(b, (80, 19), (85, 24), SILK, 0.15)
poly(b, [(88, 19), (94, 19), (93, 24), (89, 24)], SILK, 0.15)
for content, at, layer, size, h in [
    ("STROKE", (55, 30), SILK, 1.5, pcbnew.GR_TEXT_H_ALIGN_CENTER),
    ("TrueType 789", (65, 30), SILK, 2.0, pcbnew.GR_TEXT_H_ALIGN_LEFT),
    ("Rev ${REVISION}", (55, 35), SILK, 1.5, pcbnew.GR_TEXT_H_ALIGN_LEFT),
    ("${PCB_NUMBER}", (75, 35), SILK, 1.5, pcbnew.GR_TEXT_H_ALIGN_LEFT),
    ("BOTTOM", (60, 40), pcbnew.B_SilkS, 1.5, pcbnew.GR_TEXT_H_ALIGN_CENTER),
]:
    tx = pcbnew.PCB_TEXT(b)
    tx.SetText(content)
    tx.SetLayer(layer)
    tx.SetPosition(p(*at))
    tx.SetTextSize(mm(size, size))
    tx.SetTextThickness(FromMM(0.2))
    tx.SetHorizJustify(h)
    tx.SetMirrored(layer == pcbnew.B_SilkS)
    b.Add(tx)

for ref, value, at, rot, flip, mpn in [
    ("R1", "10k", (15, 58), 0.0, False, "RC0603-10K"),
    ("R2", "1k", (35, 58), 90.0, True, "RC0603-1K"),
]:
    fp = pcbnew.FOOTPRINT(b)
    fp.SetReference(ref)
    fp.SetValue(value)
    fp.SetFPID(pcbnew.LIB_ID("test", "R_0603"))
    for n, x in (("1", -0.8), ("2", 0.8)):
        pad(fp, n, x, (0.9, 0.95), pcbnew.PAD_SHAPE_RECT)
    m = pcbnew.FP_3DMODEL()
    m.m_Filename = "R_0603.step"
    m.m_Offset = pcbnew.VECTOR3D(0.1, 0.2, 0.0)
    m.m_Scale = pcbnew.VECTOR3D(1, 1, 1)
    m.m_Rotation = pcbnew.VECTOR3D(0, 0, 90)
    fp.Models().append(m)
    fp.SetField("MPN", mpn)
    f = fp.GetField("MPN")
    f.SetLayer(SILK)
    f.SetPosition(mm(0, -2))
    f.SetTextSize(mm(1.0, 1.0))
    f.SetTextThickness(FromMM(0.15))
    fp.GetField("Reference").SetPosition(mm(0, 2))
    b.Add(fp)
    fp.SetPosition(p(*at))
    if rot:
        fp.SetOrientationDegrees(rot)
    if flip:
        fp.Flip(fp.GetPosition(), False)

# A filled copper shape (issue #1 of the KiCad gap survey): a trapezoid on F.Cu, (50,45) (58,45)
# (56,52) (52,52), filled, stroke 0.2 (four corners: add_pad_arcs.py turns any three-point
# polygon into an arc).
s = poly(b, [(50, 45), (58, 45), (56, 52), (52, 52)], pcbnew.F_Cu, 0.2)
s.SetFilled(True)
# A copper zone inside a footprint (#10): U3 on the top at (80,50), a plain hole of 1.0 at its
# origin and a zone on F.Cu, outline (-5,-4) (5,-4) (5,4) (-5,4) from its origin, no net, minimum
# width 0.25, clearance 0.5 (filled by fill_zones.py with the board's own).
fp = pcbnew.FOOTPRINT(b)
fp.SetReference("U3")
fp.SetValue("ZONE")
fp.SetFPID(pcbnew.LIB_ID("test", "Zone"))
pad(fp, "", 0, (1.0, 1.0), pcbnew.PAD_SHAPE_CIRCLE, pcbnew.PAD_ATTRIB_NPTH, 1.0)
z = pcbnew.ZONE(fp)
z.SetLayer(pcbnew.F_Cu)
z.SetMinThickness(FromMM(0.25))
z.SetLocalClearance(FromMM(0.5))
ol = z.Outline()
ol.NewOutline()
for q in [(-5, -4), (5, -4), (5, 4), (-5, 4)]:
    ol.Append(FromMM(q[0]), FromMM(q[1]))
fp.Add(z)
b.Add(fp)
fp.SetPosition(p(80, 50))
save(b, "k10-all-features")


# k10-dnp (issue #184): five resistors like k10-one-resistor's (test:R_0603, value 10k, two
# 0.9 x 0.95 pads 0.8 either side of the origin, model box-1608.step), not turned, on the top,
# each with one of KiCad's footprint flags so they can be told apart: R1 at (10,10) none; R2 at
# (20,10) Do not populate; R3 at (30,10) Exclude from position files; R4 at (40,10) Exclude from
# bill of materials; R5 at (10,20) Not in schematic (board only). The board has a variant LITE in
# which R1 is not populated and shows the value 1k, and R2 is populated.
b = new_board()
rect(b, (0, 0), (50, 30))
b.AddVariant("LITE")
for ref, at, flag in [
    ("R1", (10, 10), 0),
    ("R2", (20, 10), pcbnew.FP_DNP),
    ("R3", (30, 10), pcbnew.FP_EXCLUDE_FROM_POS_FILES),
    ("R4", (40, 10), pcbnew.FP_EXCLUDE_FROM_BOM),
    ("R5", (10, 20), pcbnew.FP_BOARD_ONLY),
]:
    fp = pcbnew.FOOTPRINT(b)
    fp.SetReference(ref)
    fp.SetValue("10k")
    fp.SetFPID(pcbnew.LIB_ID("test", "R_0603"))
    for n, x in (("1", -0.8), ("2", 0.8)):
        pad(fp, n, x, (0.9, 0.95), pcbnew.PAD_SHAPE_RECT)
    m = pcbnew.FP_3DMODEL()
    m.m_Filename = "box-1608.step"
    fp.Models().append(m)
    fp.SetAttributes(pcbnew.FP_SMD | flag)
    if ref == "R1":
        v = fp.AddVariant("LITE")
        v.SetDNP(True)
        v.SetFieldValue("Value", "1k")
    elif ref == "R2":
        fp.AddVariant("LITE").SetDNP(False)
    b.Add(fp)
    fp.SetPosition(p(*at))
save(b, "k10-dnp")
