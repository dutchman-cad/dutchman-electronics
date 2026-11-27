"""Prints every text of a board (board texts, then each footprint's fields and texts) as a Markdown
table row: owner, layer, the stored string and what KiCad shows (GetShownText), with the board
loaded together with its project."""
import sys

import pcbnew

b = pcbnew.LoadBoard(sys.argv[1])


def row(owner, item):
    shown = item.GetShownText(True)
    layer = b.GetLayerName(item.GetLayer())
    print("| %s | %s | `%s` | `%s` |" % (owner, layer, item.GetText(), shown))


print("| owner | layer | stored | KiCad shows |")
print("|---|---|---|---|")
for d in b.GetDrawings():
    if isinstance(d, pcbnew.PCB_TEXT):
        row("board", d)
for fp in b.GetFootprints():
    for f in fp.GetFields():
        row(fp.GetReference() + " field " + f.GetName(), f)
    for d in fp.GraphicalItems():
        if isinstance(d, pcbnew.PCB_TEXT):
            row(fp.GetReference() + " text", d)
