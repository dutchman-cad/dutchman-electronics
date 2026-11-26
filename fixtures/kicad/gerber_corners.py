"""Lists the outlines of the polygon pads in a Gerber file written by KiCad.

Our own code, no KiCad code. KiCad's Gerber export draws a trapezoid ("Outline4P" macro) and a
chamfered or custom pad ("FreePoly" macro) as a polygon whose corners are given in the pad's own
frame, with a rotation, and flashes it at the pad's center. This script reads those and prints, for
every flash, the pad (reference and number), its center and the corners in board coordinates (Y up,
millimeters, the page coordinates KiCad shows with Y negated, the same frame as the reader's).

Usage: gerber_corners.py file.gbr > file.corners.txt
"""
import math
import re
import sys

text = open(sys.argv[1]).read()
macros = {}
for m in re.finditer(r"%AM(\w+)\*(.*?)%", text, re.S):
    body = [part.strip() for part in m.group(2).split("*") if part.strip()]
    for part in body:
        if part.startswith("4,1,"):
            macros[m.group(1)] = part
apertures = {}
for m in re.finditer(r"%ADD(\d+)(\w+),([^*]*)\*%", text):
    name = m.group(2)
    if name in macros:
        apertures[int(m.group(1))] = (name, [float(v) for v in m.group(3).split("X")])
current = None
pad = "?"
for line in text.splitlines():
    line = line.strip()
    t = re.match(r"%TO\.P,([^,]*),([^,*]*)", line)
    if t:
        pad = f"{t.group(1)} {t.group(2)}"
    d = re.fullmatch(r"D(\d+)\*", line)
    if d:
        current = int(d.group(1))
    f = re.match(r"X(-?\d+)Y(-?\d+)D03\*", line)
    if f and current in apertures:
        cx, cy = int(f.group(1)) / 1e6, int(f.group(2)) / 1e6
        name, params = apertures[current]
        parts = macros[name].rstrip("*").split(",")
        if name == "Outline4P":
            raw = params[:8]
            rot = params[8]
        else:
            raw = []
            for v in parts[3:]:
                if v.strip().startswith("$"):
                    break
                raw.append(float(v))
            rot = params[0]
        pts = list(zip(raw[0::2], raw[1::2]))
        if len(pts) > 1 and pts[0] == pts[-1]:
            pts.pop()
        s, c = math.sin(math.radians(rot)), math.cos(math.radians(rot))
        out = []
        for x, y in pts:
            q = (cx + x * c - y * s, cy + x * s + y * c)
            if not out or abs(out[-1][0] - q[0]) + abs(out[-1][1] - q[1]) > 1e-9:
                out.append(q)
        print(pad, "|", f"{cx:.6f} {cy:.6f}", "|", " ".join(f"{x:.6f},{y:.6f}" for x, y in out))
