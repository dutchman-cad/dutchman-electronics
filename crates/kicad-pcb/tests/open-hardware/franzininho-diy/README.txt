Openly licensed board by others, not CC0. License: CC-BY-SA-4.0, https://creativecommons.org/licenses/by-sa/4.0/

Franzininho DIY, an open-hardware board by the Franzininho project, used here as a larger real-world test case. It is NOT CC0.

License: Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0), https://creativecommons.org/licenses/by-sa/4.0/ . The full text is in LICENSE-CC-BY-SA-4.0.txt, copied from the project. The project is certified by OSHWA (UID BR000003), and the board's own silkscreen says "CC BY-SA 4.0".
Attribution (what CC BY-SA 4.0 asks for): "Franzininho DIY" (version V2RV3 on the silkscreen) by the Franzininho project (https://www.franzininho.com.br), licensed under CC BY-SA 4.0. We changed the file only by saving it again in KiCad 9.0 (see below). ShareAlike: this file, and any adapted copy of it, stays under CC BY-SA 4.0. The license covers this board file only; the Apache-2.0 license of the rest of the repository does not apply to it, and reading the file with our code does not make our code a derivative of it.

Source: https://github.com/Franzininho/franzininho-diy-board , file "Franzininho DIY/Franzininho.kicad_pcb", commit abdb4b48d867190d6b9794280be8b85b81aa1772 (default branch on 2026-11-25; the board is also in the project's tagged releases, v2.2 and earlier).

How the file in this folder was made: the file at that commit was written by KiCad 5.1 (file version 20171130). We opened it in KiCad 9.0 (container kicad/kicad:9.0, pcbnew Python: LoadBoard, then SaveBoard) and saved it, so it is in the KiCad 9 file format (version 20241229). Nothing else was edited or removed. KiCad warned that the legacy zone fill strategy is not supported and converted the zone fill; the zone has a fill in the saved file. The project and settings files KiCad writes beside it are not included. One footprint names an external 3D model (${KIPRJMOD}/library.pretty/USB-AM-S-F-B-TH.stp); that file is not in this folder.

Privacy: the board file was scanned for paths, e-mail addresses, computer names and user names before it was committed: none. It has no title block.

Numbers, as KiCad 9.0 gives them for the file in this folder (pcbnew Python, not our reader):
- Outline (Edge.Cuts): 4 lines (width 0.15), one closed contour, no cutouts. Size on the line centers 51.0675 x 30.0300 mm (51.2175 x 30.1800 including the line width). In KiCad's page coordinates the center lines run from (122.9673, 89.9886) to (174.0349, 120.0186); Y points down.
- Thickness: 1.600 mm (general thickness; no stack-up table).
- Copper layers: 2 (top and bottom).
- Footprints: 30, 28 on the top side and 2 on the bottom. For example X1 USB-A-H, IC1 DIP-8_W7.62mm_Socket_LongPads, RESET TAC-SWITCH-PTH, four mounting-hole footprints named @HOLE0 to @HOLE3 and several logo footprints.
- Pads: 62. Tracks: 157. Vias: 0. Arc tracks: 0. Zones: 1. Nets: 15.
