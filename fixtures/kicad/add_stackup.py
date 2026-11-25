"""Gives a board a stack-up table, then has KiCad rewrite the file.

KiCad's board scripting cannot write a stack-up, so the table (stackup-four-layer.sexp, written by
hand from the "stackup" section of KiCad's file-format documentation) is pasted into the board's
`setup` as text; KiCad then loads the board and saves it, which writes the file itself. Usage:
add_stackup.py <board.kicad_pcb> <stackup.sexp>
"""
import sys

import pcbnew

path, table = sys.argv[1], sys.argv[2]
text = open(path).read()
i = text.index("(setup")
j = text.index("\n", i)
text = text[: j + 1] + open(table).read() + text[j + 1 :]
open(path, "w").write(text)
b = pcbnew.LoadBoard(path)
b.Save(path)
