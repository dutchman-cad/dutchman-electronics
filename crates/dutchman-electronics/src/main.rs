//! `dutchman-electronics`: the command Dutchman's launcher finds on the PATH.
//! For now it has two sub-commands: `formats` and `streams <file>`.

use std::process::ExitCode;

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
        ["streams", path] => match std::fs::read(path) {
            Ok(bytes) => match dutchman_electronics::altium::list_streams(&bytes) {
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
            },
            Err(e) => {
                eprintln!("cannot read {path}: {e}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("usage: dutchman-electronics formats | streams <board.PcbDoc>");
            ExitCode::from(2)
        }
    }
}
