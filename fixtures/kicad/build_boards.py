"""Builds the KiCad test boards (one feature per board) with KiCad's own board scripting.

Run it inside KiCad 9 (for example `podman run --rm -v "$PWD:/work" docker.io/kicad/kicad:9.0
python3 /work/fixtures/kicad/build_boards.py /work/crates/kicad-pcb/tests/boards`). The script
only places objects at the numbers written below and saves the boards. It is our own code and
never feeds format knowledge into the reader: the reader learns only from the saved files and the
numbers in each board's README.txt. KiCad's Y axis points down; the numbers below are the
coordinates as KiCad shows them (millimeters, relative to the board's own origin at the page
coordinates X0, Y0).
"""
import os
import sys

import pcbnew
from pcbnew import FromMM, VECTOR2I, VECTOR3D

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


def line(b, a, c, layer=pcbnew.Edge_Cuts, width=0.1):
    s = shape(b, pcbnew.SHAPE_T_SEGMENT, layer, width)
    s.SetStart(p(*a))
    s.SetEnd(p(*c))
    return s


def arc(b, start, mid, end, layer=pcbnew.Edge_Cuts, width=0.1):
    s = shape(b, pcbnew.SHAPE_T_ARC, layer, width)
    s.SetArcGeometry(p(*start), p(*mid), p(*end))
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


def poly(b, pts, layer=pcbnew.Edge_Cuts, width=0.1):
    s = shape(b, pcbnew.SHAPE_T_POLY, layer, width)
    s.SetPolyPoints([p(*q) for q in pts])
    return s


def outline_rect(b, w=50.0, h=30.0):
    rect(b, (0, 0), (w, h))


def net(b, name):
    n = pcbnew.NETINFO_ITEM(b, name)
    b.Add(n)
    return n


def resistor(b, ref, at, rot=0.0, flip=False, model=None, size=(0.9, 0.95), pitch=1.6):
    fp = pcbnew.FOOTPRINT(b)
    fp.SetReference(ref)
    fp.SetValue("10k")
    fp.SetFPID(pcbnew.LIB_ID("test", "R_0603"))
    for i, sx in enumerate((-1, 1)):
        pad = pcbnew.PAD(fp)
        pad.SetNumber(str(i + 1))
        pad.SetShape(pcbnew.PAD_SHAPE_RECT)
        pad.SetAttribute(pcbnew.PAD_ATTRIB_SMD)
        pad.SetSize(VECTOR2I(FromMM(size[0]), FromMM(size[1])))
        pad.SetLayerSet(pad.SMDMask())
        pad.SetPosition(VECTOR2I(FromMM(sx * pitch / 2), 0))
        fp.Add(pad)
    if model:
        m = pcbnew.FP_3DMODEL()
        m.m_Filename = model["file"]
        m.m_Offset = VECTOR3D(*model.get("offset", (0, 0, 0)))
        m.m_Scale = VECTOR3D(*model.get("scale", (1, 1, 1)))
        m.m_Rotation = VECTOR3D(*model.get("rotate", (0, 0, 0)))
        fp.Models().append(m)
    b.Add(fp)
    fp.SetPosition(p(*at))
    if rot:
        fp.SetOrientationDegrees(rot)
    if flip:
        fp.Flip(fp.GetPosition(), False)
    return fp


def save(b, name):
    b.Save(os.path.join(OUT, name + ".kicad_pcb"))


# 1. empty-outline: 50 x 30 mm rectangle, 1.6 mm, nothing else.
b = new_board()
outline_rect(b)
save(b, "empty-outline")

# 2. outline-shapes: an outline made of lines and one arc (a rounded corner), a circle cutout,
#    a rectangle cutout and a polygon cutout.
b = new_board()
line(b, (0, 0), (46, 0))
arc(b, (46, 0), (46 + 4 * 0.70710678, 4 - 4 * 0.70710678), (50, 4))
line(b, (50, 4), (50, 30))
line(b, (50, 30), (0, 30))
line(b, (0, 30), (0, 0))
circle(b, (10, 15), 1.5)
rect(b, (20, 10), (28, 14))
poly(b, [(35, 20), (42, 20), (38.5, 26)])
save(b, "outline-shapes")

# 3. four-layer: the rectangle on a four-layer stack-up, 1.2 mm thick.
b = new_board(4, 1.2)
outline_rect(b)
save(b, "four-layer")

# 4. one-track-arc-via: one top track, one bottom track, one top arc and one via.
b = new_board()
outline_rect(b)
gnd = net(b, "GND")
sig = net(b, "SIG")
t = pcbnew.PCB_TRACK(b)
t.SetStart(p(10, 10))
t.SetEnd(p(20, 10))
t.SetWidth(FromMM(0.5))
t.SetLayer(pcbnew.F_Cu)
t.SetNet(sig)
b.Add(t)
t = pcbnew.PCB_TRACK(b)
t.SetStart(p(10, 20))
t.SetEnd(p(30, 20))
t.SetWidth(FromMM(0.4))
t.SetLayer(pcbnew.B_Cu)
t.SetNet(gnd)
b.Add(t)
a = pcbnew.PCB_ARC(b)
a.SetStart(p(30, 10))
a.SetMid(p(30 + 5 * 0.70710678, 15 - 5 * 0.70710678))
a.SetEnd(p(35, 15))
a.SetWidth(FromMM(0.3))
a.SetLayer(pcbnew.F_Cu)
a.SetNet(sig)
b.Add(a)
v = pcbnew.PCB_VIA(b)
v.SetPosition(p(40, 20))
v.SetWidth(FromMM(0.8))
v.SetDrill(FromMM(0.4))
v.SetViaType(pcbnew.VIATYPE_THROUGH)
v.SetNet(gnd)
b.Add(v)
save(b, "one-track-arc-via")

# 5. one-resistor: R1, a 0603 with two pads, at (25, 15), no turn, with a model reference.
b = new_board()
outline_rect(b)
resistor(
    b,
    "R1",
    (25, 15),
    model={
        "file": "box-1608.step",
        "offset": (0.5, 0.25, 0.1),
        "scale": (1, 1, 1),
        "rotate": (0, 0, 30),
    },
)
save(b, "one-resistor")

# 6. one-bottom-resistor: R1 on the bottom side at (25, 15), turned 90 degrees.
b = new_board()
outline_rect(b)
resistor(
    b,
    "R1",
    (25, 15),
    rot=90.0,
    flip=True,
    model={"file": "box-1608.step", "offset": (0, 0, 0), "scale": (1, 1, 1), "rotate": (0, 0, 0)},
)
save(b, "one-bottom-resistor")

# 7. one-thru-hole-pads: a plated round pad with a 1.0 mm drill (1.8 mm round), a plated oval
#    pad (2.0 x 1.4 mm, 0.8 mm drill) and a non-plated 2.0 mm hole.
b = new_board()
outline_rect(b)
fp = pcbnew.FOOTPRINT(b)
fp.SetReference("J1")
fp.SetFPID(pcbnew.LIB_ID("test", "Header"))
specs = [
    ("1", pcbnew.PAD_SHAPE_CIRCLE, pcbnew.PAD_ATTRIB_PTH, (1.8, 1.8), 1.0, -5),
    ("2", pcbnew.PAD_SHAPE_OVAL, pcbnew.PAD_ATTRIB_PTH, (2.0, 1.4), 0.8, 0),
    ("", pcbnew.PAD_SHAPE_CIRCLE, pcbnew.PAD_ATTRIB_NPTH, (2.0, 2.0), 2.0, 5),
]
for num, shp, att, size, drill, dx in specs:
    pad = pcbnew.PAD(fp)
    pad.SetNumber(num)
    pad.SetShape(shp)
    pad.SetAttribute(att)
    pad.SetSize(VECTOR2I(FromMM(size[0]), FromMM(size[1])))
    pad.SetDrillSize(VECTOR2I(FromMM(drill), FromMM(drill)))
    if att == pcbnew.PAD_ATTRIB_PTH:
        pad.SetLayerSet(pad.PTHMask())
    else:
        pad.SetLayerSet(pad.UnplatedHoleMask())
    pad.SetPosition(VECTOR2I(FromMM(dx), 0))
    fp.Add(pad)
b.Add(fp)
fp.SetPosition(p(25, 15))
save(b, "one-thru-hole-pads")

# 8. silkscreen: on the top silkscreen a line, an arc, a circle, a polygon and a text.
b = new_board()
outline_rect(b)
S = pcbnew.F_SilkS
line(b, (5, 5), (15, 5), S, 0.15)
arc(b, (20, 5), (20 + 3 * 0.70710678, 8 - 3 * 0.70710678), (23, 8), S, 0.2)
circle(b, (35, 8), 2.0, S, 0.15)
poly(b, [(5, 20), (12, 20), (8.5, 26)], S, 0.15)
tx = pcbnew.PCB_TEXT(b)
tx.SetText("HELLO")
tx.SetLayer(S)
tx.SetPosition(p(30, 22))
tx.SetTextSize(VECTOR2I(FromMM(1.5), FromMM(1.5)))
tx.SetTextThickness(FromMM(0.2))
b.Add(tx)
save(b, "silkscreen")

# 9. one-zone: a filled GND zone on the top layer, (5, 5) to (45, 25), with a SIG via at (25, 15)
#    that the fill keeps clear of.
b = new_board()
outline_rect(b)
gnd = net(b, "GND")
sig = net(b, "SIG")
z = pcbnew.ZONE(b)
z.SetLayer(pcbnew.F_Cu)
z.SetNet(gnd)
z.SetMinThickness(FromMM(0.25))
b.Add(z)
ol = z.Outline()
ol.NewOutline()
for q in [(5, 5), (45, 5), (45, 25), (5, 25)]:
    ol.Append(FromMM(X0 + q[0]), FromMM(Y0 + q[1]))
v = pcbnew.PCB_VIA(b)
v.SetPosition(p(25, 15))
v.SetWidth(FromMM(0.8))
v.SetDrill(FromMM(0.4))
v.SetViaType(pcbnew.VIATYPE_THROUGH)
v.SetNet(sig)
b.Add(v)
# The fill is made afterwards by `fill_zones.py` (fill_zones.py).
save(b, "one-zone")


def part(b, ref, at, rot, flip, pads, model):
    """A footprint with the given pads [(number, x, y, w, h)] (rectangles, surface mount), turned
    by `rot` degrees, flipped to the bottom when `flip` is set, carrying one 3D model reference."""
    fp = pcbnew.FOOTPRINT(b)
    fp.SetReference(ref)
    fp.SetFPID(pcbnew.LIB_ID("test", "Part"))
    for num, x, y, w, h in pads:
        pad = pcbnew.PAD(fp)
        pad.SetNumber(num)
        pad.SetShape(pcbnew.PAD_SHAPE_RECT)
        pad.SetAttribute(pcbnew.PAD_ATTRIB_SMD)
        pad.SetSize(VECTOR2I(FromMM(w), FromMM(h)))
        pad.SetLayerSet(pad.SMDMask())
        pad.SetPosition(VECTOR2I(FromMM(x), FromMM(y)))
        fp.Add(pad)
    m = pcbnew.FP_3DMODEL()
    m.m_Filename = "box.step"
    m.m_Offset = VECTOR3D(*model["offset"])
    m.m_Scale = VECTOR3D(1, 1, 1)
    m.m_Rotation = VECTOR3D(*model["rotate"])
    fp.Models().append(m)
    b.Add(fp)
    fp.SetPosition(p(*at))
    if rot:
        fp.SetOrientationDegrees(rot)
    if flip:
        fp.Flip(fp.GetPosition(), False)
    return fp


# 10. model-placements: four parts with an off-center pad pair each and a model reference, to
#     check where a model lands: U1 and U2 on the bottom, U3 and U4 on the top.
b = new_board()
outline_rect(b)
asym = [("1", -2.0, 0.5, 1.0, 1.0), ("2", 1.0, -1.5, 1.5, 0.8)]
part(b, "U1", (12, 15), 90.0, True, asym, {"offset": (1.0, 2.0, 0.5), "rotate": (90.0, 0.0, 0.0)})
part(b, "U2", (25, 15), 270.0, True, asym, {"offset": (-1.0, 0.5, 0.25), "rotate": (0.0, 90.0, 0.0)})
part(b, "U3", (38, 15), 0.0, False, asym, {"offset": (0.0, 0.0, 0.0), "rotate": (90.0, 90.0, 0.0)})
part(b, "U4", (38, 25), 90.0, False, asym, {"offset": (1.5, -1.0, 0.3), "rotate": (90.0, 0.0, 45.0)})
save(b, "model-placements")


def sized_pad(fp, number, at, size, shape, attrib=pcbnew.PAD_ATTRIB_SMD, rot=0.0, drill=None):
    pad = pcbnew.PAD(fp)
    pad.SetNumber(number)
    pad.SetShape(shape)
    pad.SetAttribute(attrib)
    pad.SetSize(VECTOR2I(FromMM(size[0]), FromMM(size[1])))
    if attrib == pcbnew.PAD_ATTRIB_PTH:
        pad.SetLayerSet(pad.PTHMask())
        pad.SetDrillSize(VECTOR2I(FromMM(drill), FromMM(drill)))
    else:
        pad.SetLayerSet(pad.SMDMask())
    pad.SetPosition(VECTOR2I(FromMM(at[0]), FromMM(at[1])))
    if rot:
        pad.SetOrientationDegrees(rot)
    fp.Add(pad)
    return pad


# 11. pad-shapes: one footprint (U1, at (25, 15), not turned) with eight pads, left to right at
#     x = -21, -15, -9, -3, 3, 9, 15, 21 from the footprint's origin (y = 0):
#     1 roundrect 3 x 2, corner radius ratio 0.25; 2 trapezoid 3 x 2, delta (0, 1);
#     3 trapezoid 3 x 2, delta (1, 0); 4 trapezoid 3 x 2, delta (0.5, 1);
#     5 chamfered rectangle 3 x 2, chamfer ratio 0.25 on the top-left and bottom-right corners
#     (its other corners keep the rounding ratio 0.25); 6 custom, rectangle anchor 1 x 1 plus a
#     filled L-shaped polygon; 7 roundrect 3 x 2 turned 90 degrees; 8 through-hole custom, circle
#     anchor 1.6 plus a filled triangle, drill 0.8.
b = new_board()
outline_rect(b)
fp = pcbnew.FOOTPRINT(b)
fp.SetReference("U1")
fp.SetFPID(pcbnew.LIB_ID("test", "Shapes"))
pad = sized_pad(fp, "1", (-21, 0), (3, 2), pcbnew.PAD_SHAPE_ROUNDRECT)
pad.SetRoundRectRadiusRatio(0.25)
for number, x, delta in [("2", -15, (0, 1)), ("3", -9, (1, 0)), ("4", -3, (0.5, 1))]:
    pad = sized_pad(fp, number, (x, 0), (3, 2), pcbnew.PAD_SHAPE_TRAPEZOID)
    pad.SetDelta(VECTOR2I(FromMM(delta[0]), FromMM(delta[1])))
pad = sized_pad(fp, "5", (3, 0), (3, 2), pcbnew.PAD_SHAPE_CHAMFERED_RECT)
pad.SetRoundRectRadiusRatio(0.25)
pad.SetChamferRectRatio(0.25)
pad.SetChamferPositions(1 | 8)  # top-left and bottom-right
pad = sized_pad(fp, "6", (9, 0), (1, 1), pcbnew.PAD_SHAPE_CUSTOM)
pad.SetAnchorPadShape(pcbnew.F_Cu, pcbnew.PAD_SHAPE_RECT)
pad.AddPrimitivePoly(
    pcbnew.F_Cu,
    [VECTOR2I(FromMM(x), FromMM(y)) for x, y in [(0, 0), (2, 0), (2, 1), (1, 1), (1, 2), (0, 2)]],
    0,
    True,
)
pad = sized_pad(fp, "7", (15, 0), (3, 2), pcbnew.PAD_SHAPE_ROUNDRECT, rot=90.0)
pad.SetRoundRectRadiusRatio(0.25)
pad = sized_pad(
    fp, "8", (21, 0), (1.6, 1.6), pcbnew.PAD_SHAPE_CUSTOM, pcbnew.PAD_ATTRIB_PTH, drill=0.8
)
pad.SetAnchorPadShape(pcbnew.F_Cu, pcbnew.PAD_SHAPE_CIRCLE)
pad.AddPrimitivePoly(
    pcbnew.F_Cu,
    [VECTOR2I(FromMM(x), FromMM(y)) for x, y in [(0, 0), (2.4, 0), (0, 2.4)]],
    0,
    True,
)
b.Add(fp)
fp.SetPosition(p(25, 15))
save(b, "pad-shapes")

# 12. zone-thermal: a GND zone on the top layer, (5, 5) to (45, 25), minimum width 0.25,
#     clearance 0.5, thermal reliefs (gap 0.5, spokes 0.5 wide), round through-hole pad (1.6,
#     drill 0.8) on GND at (15, 15), and a SIG via (0.8, drill 0.4) at (30, 15).
b = new_board()
outline_rect(b)
gnd = net(b, "GND")
sig = net(b, "SIG")
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
for q in [(5, 5), (45, 5), (45, 25), (5, 25)]:
    ol.Append(FromMM(X0 + q[0]), FromMM(Y0 + q[1]))
fp = pcbnew.FOOTPRINT(b)
fp.SetReference("J1")
fp.SetFPID(pcbnew.LIB_ID("test", "Pin"))
pad = sized_pad(fp, "1", (0, 0), (1.6, 1.6), pcbnew.PAD_SHAPE_CIRCLE, pcbnew.PAD_ATTRIB_PTH, drill=0.8)
pad.SetNet(gnd)
b.Add(fp)
fp.SetPosition(p(15, 15))
v = pcbnew.PCB_VIA(b)
v.SetPosition(p(30, 15))
v.SetWidth(FromMM(0.8))
v.SetDrill(FromMM(0.4))
v.SetViaType(pcbnew.VIATYPE_THROUGH)
v.SetNet(sig)
b.Add(v)
save(b, "zone-thermal")

# 13. four-layer-stackup: the 50 x 30 mm rectangle, four copper layers, 1.6 mm. The stack-up table
#     is added afterwards by `add_stackup.py`.
b = new_board(4, 1.6)
outline_rect(b)
save(b, "four-layer-stackup")


def custom_l(fp, number, at, rot=0.0):
    """A custom pad: a 1 x 1 rectangle anchor and an L-shaped filled polygon (as on pad-shapes)."""
    pad = sized_pad(fp, number, at, (1, 1), pcbnew.PAD_SHAPE_CUSTOM, rot=rot)
    pad.SetAnchorPadShape(pcbnew.F_Cu, pcbnew.PAD_SHAPE_RECT)
    pad.AddPrimitivePoly(
        pcbnew.F_Cu,
        [VECTOR2I(FromMM(x), FromMM(y)) for x, y in [(0, 0), (2, 0), (2, 1), (1, 1), (1, 2), (0, 2)]],
        0,
        True,
    )
    return pad


def trapezoid(fp, number, at, rot=0.0):
    pad = sized_pad(fp, number, at, (3, 2), pcbnew.PAD_SHAPE_TRAPEZOID, rot=rot)
    pad.SetDelta(VECTOR2I(0, FromMM(1)))
    return pad


def chamfered(fp, number, at, rot=0.0):
    pad = sized_pad(fp, number, at, (3, 2), pcbnew.PAD_SHAPE_CHAMFERED_RECT, rot=rot)
    pad.SetRoundRectRadiusRatio(0.25)
    pad.SetChamferRectRatio(0.25)
    pad.SetChamferPositions(1 | 8)  # top-left and bottom-right
    return pad


# 14. pad-shapes-turned: the pad shapes of pad-shapes with their frames turned and flipped.
#     U1 (top, at (10, 8), not turned): pad 1 custom L turned 30 at (0, 0); pad 2 custom L turned
#     90 at (6, 0); pad 3 trapezoid (delta 0 1) turned 90 at (12, 0); pad 4 chamfered turned 30
#     at (18, 0). U2 (top, at (10, 20), turned 30): pad 1 custom L at (0, 0), pad 2 trapezoid at
#     (6, 0), pad 3 chamfered at (12, 0), all unturned within the footprint. U3 (bottom, at
#     (38, 8), not turned): pad 1 custom L at (-6, 0), pad 2 trapezoid at (0, 0), pad 3 chamfered
#     at (6, 0). U4 (bottom, at (38, 20), turned 90): pad 1 custom L at (-6, 0), pad 2 trapezoid
#     at (0, 0). Pad sizes and shapes are those of pad-shapes.
b = new_board()
outline_rect(b)


def footprint(ref, at, rot, flip, make):
    fp = pcbnew.FOOTPRINT(b)
    fp.SetReference(ref)
    fp.SetFPID(pcbnew.LIB_ID("test", "Turned"))
    make(fp)
    b.Add(fp)
    fp.SetPosition(p(*at))
    if rot:
        fp.SetOrientationDegrees(rot)
    if flip:
        fp.Flip(fp.GetPosition(), False)


def u1(fp):
    custom_l(fp, "1", (0, 0), 30.0)
    custom_l(fp, "2", (6, 0), 90.0)
    trapezoid(fp, "3", (12, 0), 90.0)
    chamfered(fp, "4", (18, 0), 30.0)


def u2(fp):
    custom_l(fp, "1", (0, 0))
    trapezoid(fp, "2", (6, 0))
    chamfered(fp, "3", (12, 0))


def u3(fp):
    custom_l(fp, "1", (-6, 0))
    trapezoid(fp, "2", (0, 0))
    chamfered(fp, "3", (6, 0))


def u4(fp):
    custom_l(fp, "1", (-6, 0))
    trapezoid(fp, "2", (0, 0))


footprint("U1", (10, 8), 0.0, False, u1)
footprint("U2", (10, 20), 30.0, False, u2)
footprint("U3", (38, 8), 0.0, True, u3)
footprint("U4", (38, 20), 90.0, True, u4)
save(b, "pad-shapes-turned")
