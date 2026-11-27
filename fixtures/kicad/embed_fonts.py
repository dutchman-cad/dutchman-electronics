"""Sets the font face of texts in a saved KiCad board and has KiCad embed the fonts.

Our own code. KiCad's board scripting cannot name a font, so this edits the saved text (adding a
`(face ...)` line to the `(font ...)` of each named text, and `(embedded_fonts yes)`), loads the
board and calls `EmbedFonts()`, then saves it again; KiCad itself writes the `embedded_files`.

Usage (inside KiCad 9): embed_fonts.py board.kicad_pcb "text content=Face Name" ...
"""
import re
import sys

import pcbnew

path = sys.argv[1]
text = open(path).read()
for spec in sys.argv[2:]:
    content, face = spec.rsplit("=", 1)
    pattern = re.compile(r'(\(gr_text "' + re.escape(content) + r'".*?\(font)', re.S)
    text, n = pattern.subn(lambda m: m.group(1) + '\n\t\t\t\t(face "' + face + '")', text, count=1)
    assert n == 1, content
text = text.replace("(embedded_fonts no)", "(embedded_fonts yes)")
open(path, "w").write(text)
board = pcbnew.LoadBoard(path)
board.EmbedFonts()
board.Save(path)
