Original test board made for Dutchman Electronics. Released under CC0 1.0 (public domain): https://creativecommons.org/publicdomain/zero/1.0/

(The line above covers everything in this folder: one-linked-step-resistor.PcbDoc and
box-1608.step.)

linked-model: a 3D model the board links to instead of holding
===============================================================

Made on 2026-11-25 in Altium Designer 26.12.2, by hand.

box-1608.step is the same file as the one embedded in ../one-step-resistor.PcbDoc: my own box
1.600 x 0.800 x 0.450, centred on its origin in X and Y, from z = 0 to z = 0.450, written by this
repository's board-step crate (6493 bytes, LF line ends).

The board:
1. A file copy of ../one-step-resistor.PcbDoc (R1 at (25, 15), rotation 0, top side, with its
   3D body holding the embedded box-1608.step), put into an empty folder with a copy of
   box-1608.step.
2. Opened in Altium. R1 selected; in the Properties panel, Primitives unlocked (the lock next
   to "Primitives"), so its 3D body can be selected on its own.
3. R1's 3D body selected. Properties > Source offers three buttons: Server, Embed Model and
   Link to Model. Link to Model pressed; the Path field became empty.
4. Choose... next to Path, and box-1608.step in the board's folder chosen. The body's
   Identifier became box-1608, and Path showed the full path of the file.
5. Saved, and closed in Altium. The paths Altium wrote into the board (FILENAME and
   2DCONFIGFULLFILENAME) were then blanked, as for every board here.

What the file keeps
-------------------
/Models/Data is empty: the model is not in the file. R1's body record (ComponentBodies6) has:

    MODEL.EMBED=FALSE              (TRUE on one-step-resistor)
    MODEL.MODELSOURCE=File         (Undefined on one-step-resistor)
    MODEL.NAME=box-1608.step       the bare file name
    MODEL.CHECKSUM=1259363632      the same as for the embedded copy of the same file
    MODELID={A80E16CD-AB39-46A1-AD50-5C02186E37B5}
                                   a new ID
    IDENTIFIER=98,111,120,45,49,54,48,56
                                   the body's Identifier "box-1608", as character codes

and the same placement as on one-step-resistor: MODEL.2D.X/Y at R1's centre, ROTX, ROTY and
ROTZ 0, DZ 0. A stream of its own, /ModelsNoEmbed/Data (empty on the boards with embedded
models), lists the linked model the way /Models/Data lists an embedded one, one record of a
u32 length and text:

    EMBED=FALSE|MODELSOURCE=File|ID={A80E16CD-AB39-46A1-AD50-5C02186E37B5}|ROTX=0.000|
    ROTY=0.000|ROTZ=0.000|DZ=0|CHECKSUM=1259363632|NAME=box-1608.step

with no stream holding the file. The folder of the file is not stored anywhere in the board: a search of the whole
file, as single bytes and as UTF-16, finds it only in the FILENAME fields Altium writes on every
save (now blanked). So a reader gets the file's name and checksum, and has to look for the file
itself.

The checksum
------------
MODEL.CHECKSUM is the sum of every byte of the file times its offset in the file, the first
byte (offset 0) counted once, kept to 32 bits. Every STEP file starts with "ISO-10303-21;", so
this cannot tell "the first byte counted once" from "plus 73 (the letter I)"; both give the
same number. Three files confirm it:

    box-1608.step    6493 bytes, LF          Altium 1259363632   (this board, one-step-resistor)
    box-offset.step  6477 bytes, LF          Altium 1259260868   (two-step-models)
    box-big.step     490672 bytes, CR LF     Altium 1234716065

box-big.step was box-1608.step with CR LF line ends and 4000 comment lines after HEADER;,
each "/* padding so the file is large: " followed by the line's number (0 to 3999, six digits
and a space) twelve times, then "*/". It was linked on a probe board, not kept here; the
reader's tests build it again from box-1608.step. Its sum is about 6.9e12, so the checksum
wraps at 32 bits, and a CR is counted like any other byte. Plain CRC-32 (and its usual
variants), Adler-32, Fletcher-32, FNV-1a and plain byte sums give neither of the first two
values.

What Altium does with it
------------------------
After closing the board and opening it again, with box-1608.step still next to it, Altium
reported in Messages:

    Model (ID = {00000000-0000-0000-7300-740065007000}) wasn't loaded

and it did the same with the file moved into a Models folder next to the board. Preferences >
PCB Editor > Models lists one Model Search Path, .\Models, "Relative Paths Are Relative To
Project Path"; this board was opened on its own, without a project. (The ID in the message is
not the board's MODELID: 7300 7400 6500 7000 are the letters "step" in UTF-16.) So Altium
itself relies on its search paths for a linked model, and a file next to a free board is not
enough for it.
