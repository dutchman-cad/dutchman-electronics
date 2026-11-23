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

/// A test board from the Altium reader's own folder (CC0, numbers in its README.txt).
fn test_board(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../altium-pcbdoc/tests/boards")
        .join(name)
}

#[cfg(feature = "altium")]
#[test]
fn read_writes_a_parts_package() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::temp_dir().join(format!("de-read-{}", std::process::id()));
    let board = test_board("outline-with-hole.PcbDoc");
    let result = run(&[
        "read",
        &board.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    let step = std::fs::read_to_string(out.join("board.step"));
    std::fs::remove_dir_all(&out).ok();
    let (ok, v) = result?;
    assert!(ok, "{v}");
    assert_eq!(v["schema"], "dutchman.parts/1");
    assert_eq!(v["units"], "mm");
    assert_eq!(v["source"]["format"], "altium_pcbdoc");
    assert_eq!(v["models"][0]["file"], "board.step");
    assert_eq!(v["parts"][0]["name"], "Board");
    assert_eq!(v["parts"][0]["model"], "board");
    let step = step?;
    assert!(step.starts_with("ISO-10303-21;"));
    // The board is 50 x 30 x 1.6 mm (README.txt), to 0.001 mm.
    let max = |axis: usize| {
        numbers_of(&step, "CARTESIAN_POINT('',(")
            .iter()
            .map(|p| p[axis])
            .fold(f64::MIN, f64::max)
    };
    assert!((max(0) - 50.0).abs() < 0.001, "x {}", max(0));
    assert!((max(1) - 30.0).abs() < 0.001, "y {}", max(1));
    assert!((max(2) - 1.6).abs() < 0.001, "z {}", max(2));
    // The 3 mm hole: two half cylinders of radius 1.5.
    let radii: Vec<f64> = step
        .lines()
        .filter(|l| l.contains("CYLINDRICAL_SURFACE("))
        .filter_map(|l| l.rsplit(',').next())
        .filter_map(|r| r.trim_end_matches(");").parse().ok())
        .collect();
    assert_eq!(radii.len(), 2, "hole");
    assert!(radii.iter().all(|r| (r - 1.5).abs() < 0.001), "{radii:?}");
    Ok(())
}

/// The numbers inside the parentheses after `prefix`, for every line that has it.
fn numbers_of(text: &str, prefix: &str) -> Vec<Vec<f64>> {
    text.lines()
        .filter_map(|l| l.split_once(prefix))
        .map(|(_, rest)| {
            rest.split(')')
                .next()
                .unwrap_or_default()
                .split(',')
                .filter_map(|n| n.parse().ok())
                .collect()
        })
        .collect()
}

#[test]
fn read_reports_a_file_it_cannot_read() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::temp_dir().join(format!("de-read-bad-{}", std::process::id()));
    let (ok, v) = run(&[
        "read",
        "/no/such/board.PcbDoc",
        "--out",
        &out.to_string_lossy(),
        "--json",
    ])?;
    assert!(!ok);
    assert_eq!(v["schema"], "dutchman.parts/1");
    assert_eq!(v["ok"], false);
    assert!(!out.exists());
    Ok(())
}
