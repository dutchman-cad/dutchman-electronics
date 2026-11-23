# Test boards

Each board here is one its author designed for testing and released under CC0, with a
`README.txt` beside it (see CONTRIBUTING.md). Before adding one, run:

```sh
cargo run -- privacy-check path/to/board.PcbDoc --watch "Your Name" --watch "Your Company"
```

It must say `clean`. CI also checks every board here for paths, e-mail addresses and computer
names.
