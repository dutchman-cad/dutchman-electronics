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
    let manifest = std::fs::read(out.join("manifest.json"));
    std::fs::remove_dir_all(&out).ok();
    let (ok, v) = result?;
    assert!(ok, "{v}");
    // The folder holds the same manifest the command prints.
    let on_disk: serde_json::Value = serde_json::from_slice(&manifest?)?;
    assert_eq!(on_disk, v);
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

/// A test board from the KiCad reader's own folder (CC0, numbers in its README.txt).
#[cfg(feature = "kicad")]
fn kicad_board(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../kicad-pcb/tests/boards")
        .join(name)
}

#[cfg(feature = "kicad")]
#[test]
fn info_lists_kicad_beside_altium() -> Result<(), Box<dyn std::error::Error>> {
    let (ok, v) = run(&["info", "--json"])?;
    assert!(ok);
    let ids: Vec<&str> = v["formats"]
        .as_array()
        .map(|a| a.iter().filter_map(|f| f["id"].as_str()).collect())
        .unwrap_or_default();
    assert!(ids.contains(&"kicad_pcb"), "{ids:?}");
    assert!(v["extensions"]
        .as_array()
        .is_some_and(|a| a.iter().any(|e| e == "kicad_pcb")));
    Ok(())
}

#[cfg(feature = "kicad")]
#[test]
fn detect_json_picks_the_kicad_reader() -> Result<(), Box<dyn std::error::Error>> {
    let board = kicad_board("one-resistor.kicad_pcb");
    let (ok, v) = run(&["detect", &board.to_string_lossy(), "--json"])?;
    assert!(ok);
    assert_eq!(v["detection"], "yes");
    assert_eq!(v["format"], "kicad_pcb");
    Ok(())
}

#[cfg(feature = "kicad")]
#[test]
fn read_writes_a_parts_package_for_a_kicad_board() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::temp_dir().join(format!("de-read-kicad-{}", std::process::id()));
    let board = kicad_board("outline-shapes.kicad_pcb");
    let result = run(&[
        "read",
        &board.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    let step = std::fs::read_to_string(out.join("board.step"));
    let _ = std::fs::remove_dir_all(&out);
    let (ok, v) = result?;
    assert!(ok, "{v}");
    assert_eq!(v["schema"], "dutchman.parts/1");
    assert_eq!(v["source"]["format"], "kicad_pcb");
    assert_eq!(v["name"], "outline-shapes");
    assert!(!step?.is_empty());
    Ok(())
}

#[cfg(feature = "kicad")]
#[test]
fn read_places_a_kicad_part_with_its_model_note() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::temp_dir().join(format!("de-read-kicad-part-{}", std::process::id()));
    let board = kicad_board("one-resistor.kicad_pcb");
    let result = run(&[
        "read",
        &board.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    let _ = std::fs::remove_dir_all(&out);
    let (ok, v) = result?;
    assert!(ok, "{v}");
    let notes = v["notes"].to_string();
    assert!(notes.contains("3D model file"), "{notes}");
    Ok(())
}

#[cfg(feature = "kicad")]
#[test]
fn read_says_why_an_older_kicad_file_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("de-read-old-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let board = dir.join("old.kicad_pcb");
    std::fs::write(&board, "(kicad_pcb (version 20211014) (generator pcbnew))")?;
    let result = run(&[
        "read",
        &board.to_string_lossy(),
        "--out",
        &dir.join("out").to_string_lossy(),
        "--json",
    ]);
    let _ = std::fs::remove_dir_all(&dir);
    let (ok, v) = result?;
    assert!(!ok);
    assert_eq!(v["ok"], false);
    assert!(v.to_string().contains("older than 9"), "{v}");
    Ok(())
}

#[cfg(feature = "kicad")]
#[test]
fn every_kicad_test_board_gives_a_package_with_a_board_solid(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = kicad_board("");
    let mut count = 0;
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("kicad_pcb") {
            continue;
        }
        let out = std::env::temp_dir().join(format!(
            "de-kicad-all-{}-{}",
            std::process::id(),
            path.file_stem().and_then(|s| s.to_str()).unwrap_or("x")
        ));
        let result = run(&[
            "read",
            &path.to_string_lossy(),
            "--out",
            &out.to_string_lossy(),
            "--json",
        ]);
        let solid = out.join("board.step").is_file();
        let _ = std::fs::remove_dir_all(&out);
        let (ok, v) = result?;
        assert!(ok && solid, "{}: {v}", path.display());
        assert_eq!(v["source"]["format"], "kicad_pcb");
        count += 1;
    }
    assert!(count >= 9);
    Ok(())
}

#[cfg(feature = "kicad")]
#[test]
fn read_survives_absurd_numbers_in_a_kicad_board() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("de-read-absurd-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let board = dir.join("big.kicad_pcb");
    std::fs::write(
        &board,
        "(kicad_pcb (version 20241229) (generator \"pcbnew\") (general (thickness 1.6))\n\
         (layers (0 \"F.Cu\" signal) (2 \"B.Cu\" signal) (5 \"F.SilkS\" user) (25 \"Edge.Cuts\" user))\n\
         (gr_rect (start 0 0) (end 50 30) (layer \"Edge.Cuts\") (stroke (width 0.1) (type default)))\n\
         (gr_rect (start 5 5) (end 1e300 1e300) (layer \"Edge.Cuts\") (stroke (width 0.1) (type default)))\n\
         (via (at 10 10) (size 1e300) (drill 1e300) (layers \"F.Cu\" \"B.Cu\"))\n\
         (via (at 1e300 1e300) (size 0.8) (drill 0.4) (layers \"F.Cu\" \"B.Cu\"))\n\
         (segment (start 1 1) (end 2 2) (width 1e300) (layer \"F.Cu\"))\n\
         (gr_text \"HI\" (at 10 10) (layer \"F.SilkS\") (effects (font (size 1e300 1e300) (thickness 1e300))))\n\
         (gr_circle (center 20 20) (end 1e300 20) (layer \"F.SilkS\") (stroke (width 0.1) (type default))))",
    )?;
    let out = dir.join("out");
    let result = run(&[
        "read",
        &board.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    let step = std::fs::read_to_string(out.join("board.step"));
    let manifest = std::fs::read_to_string(out.join("manifest.json"));
    let _ = std::fs::remove_dir_all(&dir);
    let (ok, v) = result?;
    assert!(ok, "{v}");
    let step = step?.to_ascii_lowercase();
    assert!(
        !step.contains("inf")
            && !step.contains("nan")
            && !step.contains("e+3")
            && !step.contains("e300")
    );
    let manifest = manifest?.to_ascii_lowercase();
    assert!(!manifest.contains("inf\"") && !manifest.contains("nan"));
    assert!(v["notes"].to_string().contains("beyond 100 m"), "{v}");
    Ok(())
}

#[cfg(feature = "kicad")]
#[test]
fn an_unrelated_dropped_note_is_not_added_to_another_failure(
) -> Result<(), Box<dyn std::error::Error>> {
    // The outline is there but flat, so the board cannot be made for that reason; the reader's
    // note about the dimension has nothing to do with it and must not be tacked on.
    let dir = std::env::temp_dir().join(format!("de-read-flat-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let board = dir.join("flat.kicad_pcb");
    std::fs::write(
        &board,
        "(kicad_pcb (version 20241229) (generator \"pcbnew\") (general (thickness 1.6))\n\
         (layers (0 \"F.Cu\" signal) (2 \"B.Cu\" signal) (25 \"Edge.Cuts\" user))\n\
         (dimension (type aligned))\n\
         (gr_rect (start 0 0) (end 0 5) (layer \"Edge.Cuts\") (stroke (width 0.1) (type default))))",
    )?;
    let result = run(&[
        "read",
        &board.to_string_lossy(),
        "--out",
        &dir.join("out").to_string_lossy(),
        "--json",
    ]);
    let _ = std::fs::remove_dir_all(&dir);
    let (ok, v) = result?;
    assert!(!ok, "{v}");
    assert!(!v.to_string().contains("dimension"), "{v}");
    Ok(())
}

/// The test project in the Altium reader's folder (CC0, see its README.txt).
#[cfg(feature = "altium")]
fn test_project() -> std::path::PathBuf {
    test_board("param-project/param-project.PrjPcb")
}

#[cfg(feature = "altium")]
#[test]
fn read_opens_a_projects_board_with_its_parameters() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::temp_dir().join(format!("de-read-prj-{}", std::process::id()));
    let result = run(&[
        "read",
        &test_project().to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
        "--board",
        "PARAM-BOARD-2",
        "--param",
        "BoardRevision=C",
    ]);
    std::fs::remove_dir_all(&out).ok();
    let (ok, v) = result?;
    assert!(ok, "{v}");
    assert_eq!(v["source"]["format"], "altium_pcbdoc");
    let notes: Vec<&str> = v["notes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| n["message"].as_str())
        .collect();
    assert!(
        notes.contains(
            &"2 text(s) refer to a project parameter or to .Designator, .Comment or .Layer_Name \
              and show its value, as Altium does"
        ),
        "{notes:?}"
    );
    Ok(())
}

#[cfg(feature = "altium")]
#[test]
fn a_project_with_two_boards_asks_which_one() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::temp_dir().join(format!("de-read-prj2-{}", std::process::id()));
    let (ok, v) = run(&[
        "read",
        &test_project().to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ])?;
    assert!(!ok);
    assert_eq!(v["ok"], false);
    let error = v["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("holds 2 boards (param-board.PcbDoc, param-board-2.PcbDoc)"),
        "{error}"
    );
    assert!(!out.exists());
    Ok(())
}

/// Notes of a read manifest, as text.
#[cfg(feature = "altium")]
fn messages(v: &serde_json::Value) -> Vec<String> {
    v["notes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| n["message"].as_str().map(str::to_string))
        .collect()
}

#[cfg(feature = "altium")]
#[test]
fn a_board_passed_on_its_own_can_use_a_projects_parameters(
) -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::temp_dir().join(format!("de-read-own-{}", std::process::id()));
    let board = test_board("param-project/param-board-2.PcbDoc");
    let result = run(&[
        "read",
        &board.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
        "--project",
        &test_project().to_string_lossy(),
    ]);
    std::fs::remove_dir_all(&out).ok();
    let (ok, v) = result?;
    assert!(ok, "{v}");
    let notes = messages(&v);
    assert!(
        notes
            .iter()
            .any(|m| m.starts_with("2 text(s) refer to a project parameter")),
        "{notes:?}"
    );
    Ok(())
}

#[cfg(feature = "altium")]
#[test]
fn a_project_link_out_of_its_folder_is_not_followed() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("de-prj-link-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let project = dir.join("p.PrjPcb");
    std::fs::write(
        &project,
        "[Design]\r\nVersion=1.0\r\n\r\n[Document1]\r\nDocumentPath=..\\outside.PcbDoc\r\n",
    )?;
    let out = dir.join("out");
    let result = run(&[
        "read",
        &project.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    std::fs::remove_dir_all(&dir).ok();
    let (ok, v) = result?;
    assert!(!ok);
    let error = v["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("leaves the project's folder, so it is not followed"),
        "{error}"
    );
    Ok(())
}

#[cfg(feature = "altium")]
#[test]
fn privacy_check_reads_a_project_file() -> Result<(), Box<dyn std::error::Error>> {
    let out = Command::new(env!("CARGO_BIN_EXE_dutchman-electronics"))
        .args(["privacy-check", &test_project().to_string_lossy()])
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{text}");
    assert!(text.starts_with("clean"), "{text}");
    Ok(())
}
