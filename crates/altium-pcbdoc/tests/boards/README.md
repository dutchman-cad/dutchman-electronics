# Test boards

Each board here is an original test board released under CC0. Each has a README.txt that starts with the declaration in
CONTRIBUTING.md; a test refuses a board without it.

One exception: the `one-dejavu-*` boards embed the font files of DejaVu Sans Mono, as Altium does with every TrueType
font a board uses. Those font files keep their own licence (Bitstream Vera, with the DejaVu changes in the public
domain), which allows passing them on with their notices. LICENSE-DejaVu.txt holds the licence; README.txt names
the boards in its second line and has the details. Everything else in those boards is CC0.

Before adding a board, run:

```sh
cargo run -- privacy-check path/to/board.PcbDoc --watch "Your Name" --watch "Your Company"
```

It must say `clean`. CI also checks every board here for paths, e-mail addresses and computer
names.

`param-project/` is a test project: a `.PrjPcb` with two boards and two parameters, with its own README.txt.
Altium writes the PC's default printer into a project file (`TargetPrinter=`) and may link its documents by absolute
paths. Run `privacy-check` on the `.PrjPcb` too: it reports both, and CI checks every project here.
