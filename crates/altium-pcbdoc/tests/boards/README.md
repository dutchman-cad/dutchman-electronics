# Test boards

Each board here is an original test board released under CC0. Each has a README.txt that starts with the declaration in
CONTRIBUTING.md; a test refuses a board without it. Before adding one, run:

```sh
cargo run -- privacy-check path/to/board.PcbDoc --watch "Your Name" --watch "Your Company"
```

It must say `clean`. CI also checks every board here for paths, e-mail addresses and computer
names.
