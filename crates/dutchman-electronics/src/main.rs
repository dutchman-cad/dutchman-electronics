//! `dutchman-electronics`: the command Dutchman's launcher finds on the PATH.
//! Module interface for Dutchman: `info --json`, `detect <file> --json` and
//! `read <file> --out <dir> --json`.
//! For people: `formats`, `streams <file>` and
//! `privacy-check <file> [--watch WORD]... [--all-text]`.

use std::process::ExitCode;

use dutchman_electronics::module;

/// Reads a file, refusing one bigger than the model's limit before loading it.
#[cfg(feature = "altium")]
fn read_board_file(path: &str) -> Result<Vec<u8>, String> {
    let limit = dutchman_electronics::board_model::MAX_INPUT_BYTES;
    let len = std::fs::metadata(path)
        .map_err(|e| format!("cannot read {path}: {e}"))?
        .len();
    if len > u64::try_from(limit).unwrap_or(u64::MAX) {
        return Err(format!("{path} is too large ({len} bytes)"));
    }
    std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))
}

const USAGE: &str = "usage: dutchman-electronics info [--json]
       dutchman-electronics detect <file> [--json]
       dutchman-electronics read <file> --out <dir> [--json]
       dutchman-electronics formats
       dutchman-electronics streams <board.PcbDoc>
       dutchman-electronics privacy-check <board.PcbDoc> [--watch WORD]... [--all-text]";

/// Looks for paths, e-mail addresses, computer names and the given words in a board file.
/// Exit code 0 when clean, 1 when something was found, 2 for a usage mistake.
#[cfg(feature = "altium")]
fn privacy_check(path: &str, rest: &[&str]) -> ExitCode {
    use dutchman_electronics::altium_pcbdoc::privacy::{check, Kind, Options};
    let mut options = Options::default();
    let mut words = rest.iter();
    while let Some(word) = words.next() {
        match (*word, words.clone().next()) {
            ("--watch", Some(w)) => {
                options.watch.push((*w).to_string());
                words.next();
            }
            ("--all-text", _) => options.all_text = true,
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let report =
        match read_board_file(path).and_then(|b| check(&b, &options).map_err(|e| e.to_string())) {
            Ok(report) => report,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        };
    for f in &report.findings {
        let kind = match &f.kind {
            Kind::Path => "path".to_string(),
            Kind::Email => "e-mail".to_string(),
            Kind::ComputerName => "computer name".to_string(),
            Kind::Watched(w) => format!("watched \"{w}\""),
            Kind::Text => "text".to_string(),
            Kind::Unreadable => "not checked".to_string(),
        };
        println!("{}  @{}  {kind}: {}", f.stream, f.offset, f.text);
    }
    if report.not_listed > 0 {
        println!("... and {} more not listed", report.not_listed);
    }
    let unreadable = report
        .findings
        .iter()
        .filter(|f| f.kind == Kind::Unreadable)
        .count();
    let identifying = report
        .findings
        .iter()
        .any(|f| !matches!(f.kind, Kind::Text | Kind::Unreadable))
        || report.not_listed > 0;
    if identifying {
        println!("NOT clean: remove the items above before sharing this file");
    }
    if unreadable > 0 {
        println!(
            "NOT fully checked: {unreadable} packed value(s) above could not be unpacked, so what \
             they hold is unknown; re-save the board in Altium, or do not share it"
        );
    }
    if report.is_clean() {
        println!("clean: no paths, e-mail addresses, computer names or watched words found");
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// How many bytes `detect` reads from the start of a file.
const DETECT_HEAD_BYTES: u64 = 4096;

/// Prints one JSON reply on stdout.
fn print_json(value: &serde_json::Value) {
    println!("{value}");
}

/// A text field of a JSON reply, or `default`.
fn text<'a>(reply: &'a serde_json::Value, name: &str, default: &'a str) -> &'a str {
    reply.get(name).and_then(|v| v.as_str()).unwrap_or(default)
}

fn info(json: bool) -> ExitCode {
    let reply = module::info();
    if json {
        print_json(&reply);
    } else {
        println!(
            "{} {} (module interface {})",
            module::DISPLAY_NAME,
            env!("CARGO_PKG_VERSION"),
            module::INTERFACE
        );
        for f in dutchman_electronics::formats() {
            println!("  {}  .{}", f.name(), f.extensions().join(", ."));
        }
    }
    ExitCode::SUCCESS
}

fn detect(path: &str, json: bool) -> ExitCode {
    use std::io::Read;
    let mut head = Vec::new();
    let read =
        std::fs::File::open(path).and_then(|f| f.take(DETECT_HEAD_BYTES).read_to_end(&mut head));
    if let Err(e) = read {
        let message = format!("cannot read {path}: {e}");
        if json {
            print_json(&module::error(module::DETECT_SCHEMA, &message));
        } else {
            eprintln!("{message}");
        }
        return ExitCode::FAILURE;
    }
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str());
    let reply = module::detect(&head, extension);
    if json {
        print_json(&reply);
    } else {
        let format = text(&reply, "format", "none");
        let detection = text(&reply, "detection", "no");
        println!("{detection} ({format})");
    }
    ExitCode::SUCCESS
}

/// Reads a board file into a parts package in `out`; prints the manifest.
fn read(path: &str, out: &str, json: bool) -> ExitCode {
    use dutchman_electronics::package;
    let path = std::path::Path::new(path);
    let built = package::read_input(path).and_then(|bytes| package::build(path, &bytes));
    let written = built.and_then(|p| package::write(std::path::Path::new(out), &p).map(|()| p));
    match (written, json) {
        (Ok(p), true) => {
            print_json(&p.manifest);
            ExitCode::SUCCESS
        }
        (Ok(p), false) => {
            println!("wrote {} file(s) into {out}", p.files.len());
            let notes = p.manifest.get("notes").and_then(|n| n.as_array());
            for n in notes.into_iter().flatten() {
                println!("  {}: {}", text(n, "severity", ""), text(n, "message", ""));
            }
            ExitCode::SUCCESS
        }
        (Err(e), true) => {
            print_json(&package::error(&e));
            ExitCode::FAILURE
        }
        (Err(e), false) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg: Vec<&str> = args.iter().map(String::as_str).collect();
    match arg.as_slice() {
        ["info"] => info(false),
        ["info", "--json"] => info(true),
        ["detect", path] => detect(path, false),
        ["detect", path, "--json"] | ["detect", "--json", path] => detect(path, true),
        ["read", path, "--out", out] => read(path, out, false),
        ["read", path, "--out", out, "--json"] | ["read", path, "--json", "--out", out] => {
            read(path, out, true)
        }
        ["formats"] => {
            for f in dutchman_electronics::formats() {
                println!("{}\t{}\t{}", f.id(), f.extensions().join(","), f.name());
            }
            ExitCode::SUCCESS
        }
        #[cfg(feature = "altium")]
        ["streams", path] => {
            use dutchman_electronics::altium_pcbdoc::list_streams;
            let listed = read_board_file(path)
                .and_then(|bytes| list_streams(&bytes).map_err(|e| e.to_string()));
            match listed {
                Ok(streams) => {
                    for s in streams {
                        println!("{:>10}  {}", s.len, s.path);
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        #[cfg(feature = "altium")]
        ["privacy-check", path, rest @ ..] => privacy_check(path, rest),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
