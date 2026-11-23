//! `dutchman-electronics`: the command Dutchman's launcher finds on the PATH.
//! For now it has two sub-commands: `formats` and `streams <file>`.

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
        _ => {
            eprintln!("usage: dutchman-electronics formats | streams <board.PcbDoc>");
            ExitCode::from(2)
        }
    }
}
