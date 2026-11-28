Original test board made for Dutchman Electronics. Released under CC0 1.0 (public domain): https://creativecommons.org/publicdomain/zero/1.0/

(The line above covers everything in this folder: variants-project.PrjPcb,
variants-project.PrjPcbVariants, variants.SchDoc, variants.SchLib, variants.PcbLib and
variants-board.PcbDoc.)

variants-project: a project with two assembly variants
======================================================

Made on 2026-11-28 in Altium Designer 26.12.2. Altium only offers Project > Variants in a project
with a schematic, so this project has one, with parts from our own symbol and footprint
libraries (no Altium library part is used).

How it was made
---------------

1. File > New > Project, Local Projects, PCB, <Empty>; name variants-project. Altium made the
   folder and variants-project.PrjPcb in it.
2. Add New to Project > Schematic Library, saved as variants.SchLib. By my own script, its one
   symbol: LibReference and Design Item ID RES-TEST, description "Resistor, test symbol",
   designator R?, a rectangle from (-100, -40) to (100, 40) mil, pin 1 at (-100, 0) pointing left
   and pin 2 at (100, 0) pointing right, both 100 mil long and passive, name and designator
   hidden. Its footprint model is R0603-TEST (kind PCBLIB, no library named, so any library of
   the project).
3. variants.PcbLib is a file copy of our own footprint library, the one one-resistor's R1 came
   from; it holds the one footprint R0603-TEST (see ../README.txt, "one-resistor"). Added with
   Add Existing to Project, together with variants-board.PcbDoc, a file copy of
   ../empty-outline.PcbDoc (50 x 30, nothing on it).
4. Add New to Project > Schematic, saved as variants.SchDoc. RES-TEST placed twice from the SCH
   Library panel; then by my own script: R1 (comment 10k) at (3000, 5000) mil and R2 (comment
   4k7) at (3600, 5000) mil, three wires (2500 to 2800, 3200 to 3400 and 3800 to 4100 mil, all
   at y 5000) and two net labels, IN at (2550, 5000) and OUT at (3850, 5000). So IN - R1 -
   NetR1_2 - R2 - OUT. Compiling reports only that IN and OUT have one pin each.
5. Design > Update PCB Document variants-board.PcbDoc: Add R1, R2, the nets IN, NetR1_2 and OUT
   and the component class "variants" (no room). Executed, then by my own script R1 put at
   (20, 15) and R2 at (30, 15), top side, rotation 0. Read back:
       R1  10k  R0603-TEST  (20, 15)  pad 1 (19.2, 15) net IN,      pad 2 (20.8, 15) net NetR1_2
       R2  4k7  R0603-TEST  (30, 15)  pad 1 (29.2, 15) net NetR1_2, pad 2 (30.8, 15) net OUT
   Board saved.
6. Project > Variants: Add Variant "Lite", R2's Component Variation set to Not Fitted; Add Variant
   "Alt", R1's Comment set to 22k there (Altium shows the Comment as "Name" in that dialog). OK,
   File > Save All.

What the files store
--------------------

variants-project.PrjPcb (UTF-8 with a byte order mark, CRLF line ends): [Document1] to
[Document4] are variants.SchLib, variants.PcbLib, variants-board.PcbDoc and variants.SchDoc, in
the order they were added. Then one section per variant:

    [ProjectVariant1]
    UniqueId=177a57bd-9b44-434a-afb5-d2e0a93e3d17
    Description=Lite
    AllowFabrication=0
    ParameterCount=0
    VariationCount=1
    Variation1=Designator=R2|UniqueId=\JQYPWMZJ|Kind=1|AlternatePart=
    ParamVariationCount=0

    [ProjectVariant2]
    UniqueId=2643fd88-9b31-46d7-accc-dbeced531415
    Description=Alt
    AllowFabrication=0
    ParameterCount=0
    VariationCount=1
    Variation1=Designator=R1|UniqueId=\LYLREFOF|Kind=0|AlternatePart=22k
    ParamVariation1=ParameterName=Comment|VariantValue=22k
    ParamDesignator1=R1
    ParamVariationCount=1

- The variant's name is Description; UniqueId is a random version-4 GUID.
- Kind=1 is Not Fitted, Kind=0 is Fitted (here with a changed parameter). For a parameter change
  Altium also writes the new value into AlternatePart.
- A variation names its part twice: Designator, and UniqueId, which is "\" followed by the
  schematic part's UniqueID (R1 is LYLREFOF, R2 JQYPWMZJ in variants.SchDoc). The board's
  component has the same text in SOURCEUNIQUEID (Components6: R1 \LYLREFOF, R2 \JQYPWMZJ, both
  with SOURCEHIERARCHICALPATH=variants, SOURCELIBREFERENCE=RES-TEST and
  SOURCECOMPONENTLIBRARY=variants.SchLib), so a variant can be matched to board components by
  either.
- [Configuration1] has Variant=[No Variations], and the output groups name a variant per output
  in OutputVariantNameN (empty or [No Variations] here).

variants-board.PcbDoc did not change at all: the copy saved before Project > Variants and the
file after adding both variants and Save All are the same, byte for byte, and so is the
schematic (the two libraries were not saved again either). Which parts a variant leaves out, and
which values it changes, is only in the project file.

variants-project.PrjPcbVariants is new: Altium wrote it when the Variants dialog was first
opened. It is a compound file in Altium's schematic library format holding one empty component,
Component_1 (designator and comment "*", no graphics): a library for alternate parts, unused
here.

The schematic and libraries
---------------------------

variants.SchDoc and variants.SchLib are compound files whose records are text (|KEY=value|, as
in the board files); the pins are text records in the schematic (RECORD=2) but binary records
in the symbol library. The schematic's sheet parameters
(Author, CompanyName, DrawnBy, Engineer, Organization, Address1 to 4 and the other title block
fields) are all empty ("*"); its FileVersionInfo is base64 gzip text holding three of Altium's
own compatibility notes (with Altium's GUIDs for those features), nothing from this PC.

Privacy
-------

- The board: FILENAME, 2DCONFIGFULLFILENAME and the two components' SOURCEFOOTPRINTLIBRARY
  blanked as for the other boards (see ../README.txt, "Paths inside the files"); one folder
  path in it overwritten with a path of the same length. Altium embedded no font.
- variants.PcbLib: FILENAME and 2DCONFIGFULLFILENAME blanked in the same way, and the Windows
  user name, the Altium install ID and one folder path in it overwritten with neutral text of the
  same length.
- The project: Altium wrote the name of the PC's default printer into the ten TargetPrinter=
  lines of its output groups. Each was emptied (TargetPrinter= with no value).
- The schematic, the symbol library and variants-project.PrjPcbVariants needed no change.
- privacy-check reports all six files clean, also with personal watch words; a raw byte scan
  (ASCII and UTF-16) of every file and of the board's and footprint library's streams found no
  path, user name, printer, licence or e-mail. Altium's History, __Previews and Project Logs
  folders were not committed.
