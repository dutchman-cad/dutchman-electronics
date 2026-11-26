# Legal and provenance policy

Dutchman Electronics reads circuit-board files written by other programs. We build those readers clean-room: everything a reader knows about a format must trace back to files we are entitled to have and to public information. This page says which sources are allowed, which are not, and how to raise a concern. It applies to every contribution. If something you want to contribute does not fit it, don't contribute it yet: open an issue and ask.

This is a working policy written by engineers, not legal advice.

## Two separate declarations

We ask for two different statements, for two different things:

1. **Provenance of reader code and format specs** (this page). Every pull request that adds or changes a format reader or a format description carries the provenance sentence below.
2. **Test boards** (CONTRIBUTING.md). Every committed test board carries the CC0 declaration line in its README.txt. The one exception is the open-hardware boards made by others (`crates/kicad-pcb/tests/open-hardware`): each keeps its own accepted Creative Commons license (CC0, CC-BY 4.0 or CC-BY-SA 4.0), named in its README.txt with its source and attribution. A test enforces both.

## Allowed sources

A reader, or a written description of a format, may be derived only from:

- **Files you are legally entitled to possess**, above all boards you designed yourself, or files whose terms allow inspection and sharing what you learn from them.
- **Observing those files' bytes**: hex views, structure listings, and comparing files you made with one controlled change between them.
- **Public information**: open standards, published papers, and public vendor documentation with no restriction on use.
- **Open-source code under a permissive license** (MIT, BSD, Apache-2.0), with attribution: name the source in the pull request, and keep its license and copyright notice wherever code is reused.

### KiCad

The KiCad reader (`.kicad_pcb`) is derived only from KiCad's published file-format documentation (the "File Formats" section of KiCad's developer documentation) and from real board files: boards we make in KiCad ourselves, and openly licensed open-hardware boards. Never from KiCad's source code, which is GPL (see below).

## Forbidden sources

Reader code and format descriptions must not be derived from, and must not contain, any of these:

1. **Vendor SDKs and toolkits**: Altium's SDK, its scripting API documentation, headers, type libraries and sample code, and any other vendor's format toolkit. (Test-board builder scripts may use Altium's scripting; see below. Reader code may not learn from it.)
2. **Decompiled or disassembled vendor programs.** You may study the files a program writes, never the program that writes them.
3. **Confidential material**: anything under NDA, internal vendor documents, leaked specifications, or anything obtained by breaking an agreement or an access control.
4. **Copyleft code**: anything under GPL, LGPL or AGPL. This explicitly includes KiCad's Altium importer (`pcbnew/plugins/altium`, GPL-3), every other copyleft Altium parser, and all of KiCad's own source code (GPL). Do not read such code at all while working on reader code, not even as a hint.
5. **Files you may not possess or share**, including any employer's or customer's boards.
6. **Circumventing protection.** We do not decode content protected by encryption, license enforcement or other access controls, and we accept no keys, tools or format knowledge whose purpose is getting around such a measure.

## Conflict-of-knowledge rule

If what you know about a format could reasonably have come from a forbidden source (for example you are or were under NDA covering it, or you worked with the vendor's SDK for it), do not contribute reader code or format descriptions for that format. Working the same thing out again yourself does not fix this: each claim must trace back to an allowed source, not to memory. Every other part of the project (other formats, the board model, tooling, docs, test boards) is open to you.

## Provenance sentence

Every pull request that adds or changes a reader (a format crate's `src/`) or a format description (`docs/formats/`) must include this sentence in its description, ticked in the pull-request template, and only if it is true:

> I derived this contribution solely by analyzing files I am legally entitled to possess and from publicly available information. I did not use vendor SDKs, decompiled binaries, confidential or NDA-covered material, or any source listed as forbidden in LEGAL.md.

A CI check refuses such a pull request without it. If you cannot truthfully say it, the contribution cannot be accepted as it stands.

## Test-board builder scripts

Scripts that make test boards inside Altium may use Altium's built-in scripting: they only place objects at given coordinates and save the result. They never feed knowledge of the file format into the reader. The reader learns only from the saved files and the numbers each board was made with. The same holds for KiCad: scripts that make test boards may use KiCad's own board scripting (pcbnew's Python module) and its command-line exports (for example the VRML export, to measure where KiCad puts a 3D model). We run KiCad only as a program; we never read its source code, and the reader learns only from the saved files and from what those exports show. Builder scripts live apart from reader code (in a `fixtures/` folder, or outside this repository) and must be our own code, not copies of vendor examples.

## Trademarks

Altium, Altium Designer, KiCad, and the other product and format names mentioned in this repository are trademarks of their owners. We use them only to say which files a reader works with. Dutchman Electronics is not affiliated with, endorsed by or sponsored by any of them.

## Raising a concern or asking for a takedown

If you believe something here came from a forbidden source, infringes a right you hold, or breaks this policy:

- Open an issue for anything that can be discussed in public. For anything private, use this repository's private reporting: [Security tab → Report a vulnerability](https://github.com/dutchman-cad/dutchman-electronics/security/advisories/new).
- Name the files, commits or sections concerned and why.
- We review every report. If a concern holds up, we remove or revert the material while its provenance is checked, and re-derive it from allowed sources if needed.

Good-faith reports are welcome and will not be held against anyone.

---

Adapted from cadmpeg's [LEGAL.md](https://github.com/cadmpeg/cadmpeg/blob/main/LEGAL.md) (CC-BY-4.0).
