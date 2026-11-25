Openly licensed board by others, not CC0. Licence: CC-BY-4.0, https://creativecommons.org/licenses/by/4.0/

GB-CARTPP-XC, an open-hardware board by gekkio, used here as a larger real-world test case. It is NOT CC0.

Licence: Creative Commons Attribution 4.0 International (CC BY 4.0), https://creativecommons.org/licenses/by/4.0/ . The full text is in LICENSE-CC-BY-4.0.txt, copied from the project.
Attribution (what CC BY 4.0 asks for): "GB-CARTPP-XC" by gekkio (https://gekkio.fi), from the project "Game Boy -related custom hardware by gekkio", licensed under CC BY 4.0. We changed the file only by saving it again in KiCad 9.0 (see below). The creator's name and website are still in the board file (title block and silkscreen text), as the project asks.

Source: https://github.com/Gekkio/gb-hardware , folder GB-CARTPP-XC, file GB-CARTPP-XC.kicad_pcb, commit b5ade559bd988fd6c4f5941615d4d6d79dec9098 (default branch on 2026-11-25; the project has no release tags). The board's own revision is v1.2 (title block), dated 2020-10-04.

How the file in this folder was made: the file at that commit was written by KiCad 6 (file version 20221018). We opened it in KiCad 9.0 (container kicad/kicad:9.0, pcbnew Python: LoadBoard, then SaveBoard) and saved it, so it is in the KiCad 9 file format (version 20241229). Nothing else was edited or removed. The project and settings files KiCad writes beside it are not included. The board stores no 3D model files; its footprints name models from KiCad's standard library (${KISYS3DMOD}/...) and the project's own library.

Privacy: the board file was scanned for paths, e-mail addresses, computer names and user names before it was committed: none. Its title block holds only the project's public name, revision, date and the creator's website (the attribution above).

Numbers, as KiCad 9.0 gives them for the file in this folder (pcbnew Python, not our reader):
- Outline (Edge.Cuts): 8 items, four lines and four rounded corners (arcs), one closed contour, no cutouts. Size on the line centres 80.000 x 55.000 mm (80.050 x 55.050 including the 0.05 line width). In KiCad's page coordinates the centre lines run from (50, 50) to (130, 105); Y points down.
- Thickness: 1.000 mm (general thickness; no stack-up table).
- Copper layers: 2 (F.Cu, B.Cu).
- Footprints: 29, all on the top side (0 on the bottom). For example U2 TQFP-44_10x10mm_P0.8mm, J3 GameBoy_Cartridge_AGS_1x32-1MP_P1.50mm_Socket_Horizontal, J1 USB_C_Receptacle_HRO_TYPE-C-31-M-12.
- Pads: 156. Tracks: 437. Vias: 144. Arc tracks: 0. Zones: 3. Nets: 46.
