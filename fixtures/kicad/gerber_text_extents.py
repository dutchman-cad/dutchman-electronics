"""Lists the ink of each text in a silkscreen Gerber file written by KiCad.

Our own code, no KiCad code. KiCad's Gerber export draws a stroke text as pen strokes (line
draws with a round aperture). This script collects every drawn stroke, groups the strokes that lie
within 3 mm of each other into one text, and prints for each text the box of the pen's center
line plus the pen width, in board coordinates (Y up, millimeters, the page coordinates KiCad shows
with Y negated, the same frame as the reader's): "min_x min_y max_x max_y pen".

Usage: gerber_text_extents.py file.gbr > file.extents.txt
"""
import re
import sys

text = open(sys.argv[1]).read()
fs = re.search(r"%FSLAX(\d)(\d)Y(\d)(\d)\*%", text)
scale = 10.0 ** -int(fs.group(2))
apertures = {}
for m in re.finditer(r"%ADD(\d+)C,([\d.]+)\*%", text):
    apertures[int(m.group(1))] = float(m.group(2))
pen = 0.0
x = y = 0.0
segs = []  # (pen, [points])
drawing = None
for line in text.splitlines():
    line = line.strip()
    d = re.fullmatch(r"D(\d+)\*", line)
    if d and int(d.group(1)) >= 10:
        pen = apertures.get(int(d.group(1)), 0.0)
        continue
    m = re.match(r"(?:X(-?\d+))?(?:Y(-?\d+))?(?:I-?\d+)?(?:J-?\d+)?D0([123])\*", line)
    if not m:
        continue
    if m.group(1) is not None:
        x = int(m.group(1)) * scale
    if m.group(2) is not None:
        y = int(m.group(2)) * scale
    op = m.group(3)
    if op == "2":
        drawing = [(x, y)]
        segs.append((pen, drawing))
    elif op == "1" and drawing is not None:
        drawing.append((x, y))
    # a flash (D03) is not a stroke of a text
boxes = []
for pen, pts in segs:
    if len(pts) < 2:
        continue
    xs, ys = [q[0] for q in pts], [q[1] for q in pts]
    boxes.append([min(xs), min(ys), max(xs), max(ys), pen])
GAP = 3.0
changed = True
while changed:
    changed = False
    out = []
    for b in boxes:
        for o in out:
            if (b[0] <= o[2] + GAP and o[0] <= b[2] + GAP
                    and b[1] <= o[3] + GAP and o[1] <= b[3] + GAP):
                o[0], o[1] = min(o[0], b[0]), min(o[1], b[1])
                o[2], o[3] = max(o[2], b[2]), max(o[3], b[3])
                o[4] = max(o[4], b[4])
                changed = True
                break
        else:
            out.append(list(b))
    boxes = out
for b in sorted(boxes, key=lambda b: (-round(b[1] / 10), b[0])):
    print("%.4f %.4f %.4f %.4f %.4f" % tuple(b))
