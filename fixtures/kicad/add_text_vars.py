"""Writes a project's text variables into its .kicad_pro (KiCad's file if it saved one beside the
board, otherwise a new one holding only these).

Usage: add_text_vars.py board.kicad_pro [NAME=VALUE ...]; without NAME=VALUE, the variables of
k10-text-vars."""
import json
import os
import sys

path = sys.argv[1]
pro = {"meta": {"filename": os.path.basename(path), "version": 3}}
if os.path.exists(path):
    with open(path, encoding="utf-8") as f:
        pro = json.load(f)
given = dict(arg.split("=", 1) for arg in sys.argv[2:])
pro["text_variables"] = given or {
    "PCB_NUMBER": "DN-0042",
    "BOARD_ID": "${PCB_NUMBER}-${REVISION}",
    "FINISH": "ENIG",
}
with open(path, "w", encoding="utf-8", newline="\n") as f:
    json.dump(pro, f, indent=2)
    f.write("\n")
