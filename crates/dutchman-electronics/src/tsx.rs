//! A tscircuit source file (`.tsx`) into Circuit JSON, by running the tscircuit CLI:
//! `tsci export <file> -f circuit-json -o <tmp>`. The CLI is found on the PATH, or at the path
//! in `DUTCHMAN_TSCI`. Without it, a `.tsx` file cannot be read; a `.circuit.json` can.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The longest `tsci export` may run.
const TIME_LIMIT: Duration = Duration::from_secs(300);

/// A file in the temporary folder that is removed when dropped.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn temp(suffix: &str) -> TempFile {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or_default();
    TempFile(std::env::temp_dir().join(format!(
        "dutchman-electronics-{}-{nanos}{suffix}",
        std::process::id()
    )))
}

/// Compiles a `.tsx` board with the tscircuit CLI and returns its Circuit JSON.
pub fn compile(path: &Path) -> Result<Vec<u8>, String> {
    let tsci = std::env::var_os("DUTCHMAN_TSCI")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("tsci"));
    let out = temp(".circuit.json");
    let log = temp(".log");
    let log_file = std::fs::File::create(&log.0).map_err(|e| format!("cannot write a log: {e}"))?;
    let mut child = Command::new(&tsci)
        .arg("export")
        .arg(path)
        .args(["-f", "circuit-json", "-o"])
        .arg(&out.0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(log_file))
        .spawn()
        .map_err(|e| {
            format!(
                "cannot run the tscircuit CLI ({}): {e}. Install it (npm install -g @tscircuit/cli) \
                 or export a .circuit.json and read that",
                tsci.display()
            )
        })?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > TIME_LIMIT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "tsci export took longer than {} s and was stopped",
                    TIME_LIMIT.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(format!("tsci export: {e}")),
        }
    };
    if !status.success() {
        let said = std::fs::read_to_string(&log.0).unwrap_or_default();
        let tail: String = said
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!("tsci export failed ({status}):\n{tail}"));
    }
    crate::package::read_file(&out.0)
}
