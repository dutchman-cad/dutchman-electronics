//! `dutchman-electronics`: the command Dutchman's launcher finds on the PATH.
//! For now it has three sub-commands: `formats`, `streams <file>` and
//! `privacy-check <file> [--watch WORD]... [--all-text]`.

use std::process::ExitCode;

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

const USAGE: &str = "usage: dutchman-electronics formats
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
    let report = match read_board_file(path).and_then(|b| check(&b, &options).map_err(|e| e.to_string())) {
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
        };
        println!("{}  @{}  {kind}: {}", f.stream, f.offset, f.text);
    }
    if report.not_listed > 0 {
        println!("... and {} more not listed", report.not_listed);
    }
    if report.is_clean() {
        println!("clean: no paths, e-mail addresses, computer names or watched words found");
        ExitCode::SUCCESS
    } else {
        println!("NOT clean: remove the items above before sharing this file");
        ExitCode::FAILURE
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg: Vec<&str> = args.iter().map(String::as_str).collect();
    match arg.as_slice() {
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
