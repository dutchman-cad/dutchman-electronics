"""Writes the project text variables of k10-text-vars into its .kicad_pro (KiCad's file if it saved
one beside the board, otherwise a new one holding only these)."""
import json
import os
import sys

path = sys.argv[1]
pro = {"meta": {"filename": os.path.basename(path), "version": 3}}
if os.path.exists(path):
    with open(path, encoding="utf-8") as f:
        pro = json.load(f)
pro["text_variables"] = {
    "PCB_NUMBER": "DN-0042",
    "BOARD_ID": "${PCB_NUMBER}-${REVISION}",
    "FINISH": "ENIG",
}
with open(path, "w", encoding="utf-8", newline="\n") as f:
    json.dump(pro, f, indent=2)
    f.write("\n")
