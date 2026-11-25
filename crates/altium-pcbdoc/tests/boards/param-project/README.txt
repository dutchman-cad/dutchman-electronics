Original test board made for Dutchman Electronics. Released under CC0 1.0 (public domain): https://creativecommons.org/publicdomain/zero/1.0/

(The line above covers everything in this folder: param-project.PrjPcb, param-board.PcbDoc and
param-board-2.PcbDoc.)

param-project: a project with parameters, and texts that refer to them
=======================================================================

Made on 2026-11-25 in Altium Designer 26.12.2.

The project, by hand:
1. File > New > Project, Local Projects, PCB, <Empty>; name param-project.
2. In the same dialog, under Parameters, two parameters, both as text:
       PCBNumber      DN-0042
       BoardRevision  B
3. Create. Altium made the folder param-project and param-project.PrjPcb in it.
4. Two file copies of tests/boards/empty-outline.PcbDoc (50 x 30, nothing on it) put into that
   folder as param-board.PcbDoc and param-board-2.PcbDoc, and added in that order with
   Add Existing to Project; the project saved after each.

The texts, by my own script with the project open, each read back and logged together with the
string Altium shows for it. Every text is in the stroke font, height 1.500, stroke 0.150,
rotation 0. Position is the text's anchor.

param-board.PcbDoc

    Stored text                  Layer           At (x, y)    Mirrored  Altium shows
    .PCBNumber                   Top Overlay     2, 26        no        DN-0042
    .BoardRevision               Top Layer       2, 22.5      no        B
    Rev .BoardRevision           Top Overlay     2, 19        no        Rev B
    .pcbnumber                   Top Overlay     2, 15.5      no        DN-0042
    .NoSuchParameter             Top Overlay     2, 12        no        .NoSuchParameter
    X.PCBNumber                  Top Overlay     2, 8.5       no        XDN-0042
    .PCBNumber/.BoardRevision    Top Overlay     2, 5         no        .PCBNumber/.BoardRevision
    .PCBNumberX                  Top Overlay     2, 1.5       no        .PCBNumberX
    =PCBNumber                   Top Overlay     27, 26       no        =PCBNumber
    .BoardRevision.              Top Overlay     27, 22.5     no        .BoardRevision.
    ..PCBNumber                  Top Overlay     27, 19       no        ..PCBNumber
    .Layer_Name                  Top Overlay     27, 15.5     no        Top Overlay
    .PCBNumber                   Bottom Overlay  48, 12       yes       DN-0042
    .PCBNumber rev               Top Overlay     27, 8.5      no        .PCBNumber rev
    a.b.PCBNumber                Top Overlay     27, 1.5      no        a.bDN-0042
    a/.PCBNumber                 Bottom Overlay  48, 26       yes       a/DN-0042
    x..PCBNumber                 Bottom Overlay  48, 22.5     yes       x.DN-0042
    .x.PCBNumber                 Bottom Overlay  48, 19       yes       .x.PCBNumber

param-board-2.PcbDoc

    .PCBNumber                   Top Overlay     2, 26        no        DN-0042
    Board 2 rev .BoardRevision   Top Overlay     2, 22.5      no        Board 2 rev B

The board files keep each text as typed (the "Stored text" column); Altium shows a parameter's
value in its place when the board is open in the project. .Layer_Name is one of Altium's own
special strings (the text's layer), not a project parameter. All of the above follows one rule:
a text that starts with "." is replaced when everything after that dot names a parameter; any
other text is replaced when the part after its last "." names a parameter, keeping what comes
before that dot. Names match in any case.

The project file
----------------

param-project.PrjPcb is text: UTF-8 with a byte order mark and CRLF line ends, in sections.
[Document1] has DocumentPath=param-board.PcbDoc and [Document2] DocumentPath=param-board-2.PcbDoc,
relative to the project's folder. [Parameter1] is Name=BoardRevision, Value=B and [Parameter2]
Name=PCBNumber, Value=DN-0042, the other way round from how they were entered. The rest is
Altium's default project settings. Its one GUID, ContentTypeGUID=CB6F2064-E317-11DF-B822-
12313F0024A2, is a version-1 GUID whose time field dates it to 2010, years before this project
was made, so it did not come from this PC.

Privacy
-------

- The boards: FILENAME and 2DCONFIGFULLFILENAME blanked as for the other boards (see
  ../README.txt, "Paths inside the files"). Altium embedded no font. All GUIDs are version 4.
- The project: Altium wrote the name of the PC's default printer into the ten TargetPrinter=
  lines of its output groups. Each was emptied (TargetPrinter= with no value). Nothing else was
  changed.
- privacy-check reports both boards clean, also with personal watch words; the project file
  holds none of them.
