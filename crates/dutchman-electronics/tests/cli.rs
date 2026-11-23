//! The command as Dutchman runs it: one JSON object on stdout per call.

use std::process::Command;

fn run(args: &[&str]) -> Result<(bool, serde_json::Value), Box<dyn std::error::Error>> {
    let out = Command::new(env!("CARGO_BIN_EXE_dutchman-electronics"))
        .args(args)
        .output()?;
    let value = serde_json::from_slice(&out.stdout)?;
    Ok((out.status.success(), value))
}

#[test]
fn info_json_is_one_module_info_object() -> Result<(), Box<dyn std::error::Error>> {
    let (ok, v) = run(&["info", "--json"])?;
    assert!(ok);
    assert_eq!(v["schema"], "dutchman.module-info/1");
    assert_eq!(v["name"], "electronics");
    assert_eq!(v["interface"], 1);
    Ok(())
}

#[test]
fn detect_json_reads_a_file() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!("de-detect-{}.txt", std::process::id()));
    std::fs::write(&path, b"plain text")?;
    let result = run(&["detect", &path.to_string_lossy(), "--json"]);
    std::fs::remove_file(&path)?;
    let (ok, v) = result?;
    assert!(ok);
    assert_eq!(v["schema"], "dutchman.module-detect/1");
    assert_eq!(v["detection"], "no");
    Ok(())
}

#[test]
fn detect_json_reports_a_missing_file() -> Result<(), Box<dyn std::error::Error>> {
    let (ok, v) = run(&["detect", "/no/such/board.PcbDoc", "--json"])?;
    assert!(!ok);
    assert_eq!(v["ok"], false);
    Ok(())
}
