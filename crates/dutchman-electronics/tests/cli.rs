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
fn version_is_one_line_for_the_launcher() -> Result<(), Box<dyn std::error::Error>> {
    let out = Command::new(env!("CARGO_BIN_EXE_dutchman-electronics"))
        .arg("--version")
        .output()?;
    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    assert_eq!(
        String::from_utf8(out.stdout)?,
        format!("dutchman-electronics {}\n", env!("CARGO_PKG_VERSION"))
    );
    Ok(())
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
fn a_project_with_one_board_needs_no_board_option() -> Result<(), Box<dyn std::error::Error>> {
    // all-features.PrjPcb links only all-features.PcbDoc; its four parameter texts (two of
    // them quoted), U1's .Comment and R7's '.Designator' get their values.
    let out = std::env::temp_dir().join(format!("de-read-prj1-{}", std::process::id()));
    let result = run(&[
        "read",
        &test_board("all-features.PrjPcb").to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    std::fs::remove_dir_all(&out).ok();
    let (ok, v) = result?;
    assert!(ok, "{v}");
    assert_eq!(v["name"], "all-features");
    let notes: Vec<&str> = v["notes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| n["message"].as_str())
        .collect();
    assert!(
        notes.contains(
            &"6 text(s) refer to a project parameter or to .Designator, .Comment or .Layer_Name \
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

/// A project in a new temporary folder `name` holding `a\b.PcbDoc` and `c\b.PcbDoc`, both
/// copies of the test project's first board, with the project's own parameters.
#[cfg(feature = "altium")]
fn same_named_boards(name: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    let board = std::fs::read(test_board("param-project/param-board.PcbDoc"))?;
    for sub in ["a", "c"] {
        std::fs::create_dir_all(dir.join(sub))?;
        std::fs::write(dir.join(sub).join("b.PcbDoc"), &board)?;
    }
    std::fs::write(
        dir.join("p.PrjPcb"),
        "[Design]\r\nVersion=1.0\r\n\r\n[Document1]\r\nDocumentPath=a\\b.PcbDoc\r\n\r\n\
         [Document2]\r\nDocumentPath=c\\b.PcbDoc\r\n",
    )?;
    Ok(dir)
}

#[cfg(feature = "altium")]
#[test]
fn two_boards_of_one_name_are_told_apart_by_their_folder() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = same_named_boards("de-prj-same")?;
    let project = dir.join("p.PrjPcb");
    let out = dir.join("out");
    let read = |board: &str| {
        run(&[
            "read",
            &project.to_string_lossy(),
            "--out",
            &out.to_string_lossy(),
            "--json",
            "--board",
            board,
        ])
    };
    let by_name = read("b");
    let by_link = read("C/B.PcbDoc");
    let by_backslash = read("a\\b.PcbDoc");
    std::fs::remove_dir_all(&dir).ok();
    let (ok, v) = by_name?;
    assert!(!ok);
    let error = v["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("b is ambiguous") && error.contains("(a\\b.PcbDoc, c\\b.PcbDoc)"),
        "{error}"
    );
    for (ok, v) in [by_link?, by_backslash?] {
        assert!(ok, "{v}");
    }
    Ok(())
}

/// A symbolic link inside the project's folder that leads out of it is not followed.
#[cfg(all(feature = "altium", unix))]
#[test]
fn a_project_board_behind_a_symbolic_link_out_of_the_folder_is_not_read(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("de-prj-symlink-{}", std::process::id()));
    let inside = dir.join("project");
    std::fs::create_dir_all(&inside)?;
    let outside = dir.join("outside.PcbDoc");
    std::fs::copy(test_board("param-project/param-board.PcbDoc"), &outside)?;
    std::os::unix::fs::symlink(&outside, inside.join("b.PcbDoc"))?;
    let project = inside.join("p.PrjPcb");
    std::fs::write(
        &project,
        "[Design]\r\nVersion=1.0\r\n\r\n[Document1]\r\nDocumentPath=b.PcbDoc\r\n",
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
    assert!(error.contains("through a symbolic link"), "{error}");
    Ok(())
}

/// Notes of a read manifest, as text.
#[cfg(any(feature = "altium", feature = "kicad"))]
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

/// The open-hardware boards by others, as files under `../kicad-pcb/tests/open-hardware` (each
/// folder's README.txt gives the numbers KiCad itself reports).
#[cfg(feature = "kicad")]
const OPEN_BOARDS: [&str; 2] = [
    "gb-cartpp-xc/GB-CARTPP-XC.kicad_pcb",
    "franzininho-diy/Franzininho-DIY.kicad_pcb",
];

/// Reads an open-hardware board and returns its manifest and the files of its package.
#[cfg(feature = "kicad")]
fn read_open_board(
    tag: &str,
    file: &str,
) -> Result<(serde_json::Value, std::path::PathBuf), Box<dyn std::error::Error>> {
    let board = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../kicad-pcb/tests/open-hardware")
        .join(file);
    let out = std::env::temp_dir().join(format!(
        "de-open-{}-{tag}-{}",
        std::process::id(),
        board.file_stem().and_then(|s| s.to_str()).unwrap_or("x")
    ));
    let (ok, v) = run(&[
        "read",
        &board.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ])?;
    assert!(ok, "{file}: {v}");
    Ok((v, out))
}

#[cfg(feature = "kicad")]
#[test]
fn every_open_hardware_board_gives_a_package_with_a_board_solid_and_a_board_part(
) -> Result<(), Box<dyn std::error::Error>> {
    for file in OPEN_BOARDS {
        let (v, out) = read_open_board("package", file)?;
        let step = std::fs::read_to_string(out.join("board.step"));
        let manifest = std::fs::read_to_string(out.join("manifest.json"));
        let _ = std::fs::remove_dir_all(&out);
        assert!(step?.contains("MANIFOLD_SOLID_BREP"), "{file}");
        assert!(!manifest?.is_empty());
        assert_eq!(v["schema"], "dutchman.parts/1", "{file}");
        assert_eq!(v["source"]["format"], "kicad_pcb", "{file}");
        let first = &v["parts"][0];
        assert_eq!(first["name"], "Board", "{file}");
        assert_eq!(first["model"], "board", "{file}");
    }
    Ok(())
}

#[cfg(feature = "kicad")]
#[test]
fn the_open_hardware_board_packages_are_named_after_their_boards_and_keep_their_notes_honest(
) -> Result<(), Box<dyn std::error::Error>> {
    let (v, out) = read_open_board("gb", "gb-cartpp-xc/GB-CARTPP-XC.kicad_pcb")?;
    let _ = std::fs::remove_dir_all(&out);
    assert_eq!(v["name"], "GB-CARTPP-XC");
    // The board's 3D models are not found, so its parts get standard bodies where their pads
    // are a known package (default `--bodies file`); the board is first and the plating last.
    let names: Vec<&str> = v["parts"]
        .as_array()
        .map(|a| a.iter().filter_map(|p| p["name"].as_str()).collect())
        .unwrap_or_default();
    assert_eq!(names.first(), Some(&"Board"), "{names:?}");
    assert_eq!(names.last(), Some(&"Plating"), "{names:?}");
    // The board names 3D models from KiCad's own library: said, not guessed.
    assert!(
        v["notes"].to_string().contains("3D model file"),
        "{}",
        v["notes"]
    );

    let (v, out) = read_open_board("fz", "franzininho-diy/Franzininho-DIY.kicad_pcb")?;
    let _ = std::fs::remove_dir_all(&out);
    assert_eq!(v["name"], "Franzininho-DIY");
    // The one external 3D model it names is said to be missing, not guessed.
    assert!(
        v["notes"].to_string().contains("3D model file"),
        "{}",
        v["notes"]
    );
    Ok(())
}

/// The same privacy check the Altium boards get (`privacy-check`), run over the text of each
/// open-hardware board: no path, e-mail address or computer name. (The command itself reads only
/// Altium files, so this test feeds it the KiCad file's text.)
#[cfg(all(feature = "altium", feature = "kicad"))]
#[test]
fn the_open_hardware_boards_pass_the_privacy_check() -> Result<(), Box<dyn std::error::Error>> {
    use dutchman_electronics::altium_pcbdoc::privacy::{check_stream, Options, Report};
    for file in OPEN_BOARDS {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../kicad-pcb/tests/open-hardware")
            .join(file);
        let mut report = Report::default();
        check_stream(
            "board",
            &std::fs::read(&path)?,
            &Options::default(),
            &mut report,
        );
        assert!(report.is_clean(), "{file}: {:?}", report.findings);
    }
    Ok(())
}

/// Reads `board` into a fresh folder, with extra arguments; returns the result and the files
/// written.
#[cfg(feature = "altium")]
fn read_into(
    name: &str,
    board: &std::path::Path,
    extra: &[&str],
) -> Result<(bool, serde_json::Value, Vec<String>), Box<dyn std::error::Error>> {
    let out = std::env::temp_dir().join(format!("de-{name}-{}", std::process::id()));
    let board = board.to_string_lossy();
    let out_text = out.to_string_lossy();
    let mut args = vec!["read", board.as_ref(), "--out", out_text.as_ref(), "--json"];
    args.extend_from_slice(extra);
    let result = run(&args);
    let mut files: Vec<String> = std::fs::read_dir(&out)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    std::fs::remove_dir_all(&out).ok();
    let (ok, v) = result?;
    Ok((ok, v, files))
}

/// A copy of the linked-model board alone in a fresh folder, without its STEP file.
#[cfg(feature = "altium")]
fn lone_linked_board(name: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("de-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir)?;
    let board = dir.join("one-linked-step-resistor.PcbDoc");
    std::fs::copy(
        test_board("linked-model/one-linked-step-resistor.PcbDoc"),
        &board,
    )?;
    Ok(board)
}

#[cfg(feature = "altium")]
#[test]
fn a_linked_model_next_to_the_board_is_used() -> Result<(), Box<dyn std::error::Error>> {
    let board = test_board("linked-model/one-linked-step-resistor.PcbDoc");
    let (ok, v, files) = read_into("linked", &board, &[])?;
    assert!(ok, "{v}");
    let r1 = v["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["name"] == "R1")
        .ok_or("R1 is not in the package")?;
    let file = v["models"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|m| m["id"] == r1["model"])
        .and_then(|m| m["file"].as_str())
        .ok_or("R1's model is not listed")?;
    assert!(files.iter().any(|f| f == file), "{files:?}");
    let notes = messages(&v);
    assert!(
        notes.iter().any(|m| m
            == "1 part(s) link to the model file box-1608.step, which was found with the \
                board's checksum and is used"),
        "{notes:?}"
    );
    Ok(())
}

#[cfg(feature = "altium")]
#[test]
fn a_linked_model_is_found_in_a_models_folder_given() -> Result<(), Box<dyn std::error::Error>> {
    let board = lone_linked_board("linked-lone")?;
    let library = test_board("linked-model");
    let (ok, v, _) = read_into("linked-lone-out", &board, &[])?;
    assert!(ok, "{v}");
    // Not found: R1 gets the standard body of its 0603 pads, a body and its leads.
    let named = |v: &serde_json::Value, n: &str| {
        v["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|p| p["name"] == n)
    };
    assert!(named(&v, "R1") && named(&v, "R1 leads"), "{v}");
    assert!(
        messages(&v).iter().any(|m| m.contains(
            "box-1608.step, which is not in the board's folder, its Models folder or a folder \
             given with --models"
        )),
        "{v}"
    );
    let (ok, v, _) = read_into(
        "linked-lone-models",
        &board,
        &["--models", &library.to_string_lossy()],
    )?;
    std::fs::remove_dir_all(board.parent().ok_or("no folder")?).ok();
    assert!(ok, "{v}");
    // Found: R1 uses the linked model, one part, no leads part.
    assert!(named(&v, "R1") && !named(&v, "R1 leads"), "{v}");
    Ok(())
}

#[cfg(feature = "altium")]
#[test]
fn a_linked_model_of_the_same_name_but_another_checksum_is_not_used(
) -> Result<(), Box<dyn std::error::Error>> {
    let board = lone_linked_board("linked-other")?;
    let dir = board.parent().ok_or("no folder")?.to_path_buf();
    let mut step = std::fs::read(test_board("linked-model/box-1608.step"))?;
    // The same box, one coordinate changed: a different file of the same name.
    let text = String::from_utf8(step)?.replacen("(-0.8,-0.4,0.)", "(-0.9,-0.4,0.)", 1);
    step = text.into_bytes();
    std::fs::create_dir_all(dir.join("Models"))?;
    std::fs::write(dir.join("Models").join("box-1608.step"), step)?;
    let (ok, v, _) = read_into("linked-other-out", &board, &[])?;
    std::fs::remove_dir_all(&dir).ok();
    assert!(ok, "{v}");
    // The file is refused (said below), so R1 gets the standard body of its pads.
    let named = |n: &str| {
        v["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|p| p["name"] == n)
    };
    assert!(named("R1") && named("R1 leads"), "{v}");
    assert!(
        messages(&v)
            .iter()
            .any(|m| m.contains("its checksum is not the board's")),
        "{v}"
    );
    Ok(())
}

#[cfg(feature = "altium")]
#[test]
fn read_refuses_an_option_it_does_not_know() -> Result<(), Box<dyn std::error::Error>> {
    let out = Command::new(env!("CARGO_BIN_EXE_dutchman-electronics"))
        .args(["read", "x.PcbDoc", "--out", "o", "--model", "lib"])
        .output()?;
    assert_eq!(out.status.code(), Some(2));
    let out = Command::new(env!("CARGO_BIN_EXE_dutchman-electronics"))
        .args(["read", "x.PcbDoc", "--json"])
        .output()?;
    assert_eq!(out.status.code(), Some(2));
    Ok(())
}

/// KiCad keeps no checksum: a KiCad board's linked file is used by its name alone. The KiCad
/// one-resistor links `box-1608.step`, the CC0 box beside the Altium linked-model board.
#[cfg(all(feature = "altium", feature = "kicad"))]
#[test]
fn a_kicad_boards_linked_model_is_used_by_its_name() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("de-kicad-linked-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir)?;
    let board = dir.join("one-resistor.kicad_pcb");
    std::fs::copy(kicad_board("one-resistor.kicad_pcb"), &board)?;
    std::fs::copy(
        test_board("linked-model/box-1608.step"),
        dir.join("box-1608.step"),
    )?;
    let result = read_into("kicad-linked", &board, &[]);
    std::fs::remove_dir_all(&dir).ok();
    let (ok, v, files) = result?;
    assert!(ok, "{v}");
    let r1 = v["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["name"] == "R1")
        .ok_or("R1 is not in the package")?;
    let file = v["models"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|m| m["id"] == r1["model"])
        .and_then(|m| m["file"].as_str())
        .ok_or("R1's model is not listed")?;
    assert!(files.iter().any(|f| f == file), "{files:?}");
    let notes = messages(&v);
    assert!(
        notes.iter().any(|m| m
            == "1 part(s) link to the model file box-1608.step; box-1608.step was found by its \
                name only (the board keeps no checksum to check it by) and is used"),
        "{notes:?}"
    );
    Ok(())
}

/// A new temporary folder `name` holding a copy of all-features.PcbDoc.
#[cfg(feature = "altium")]
fn folder_with_all_features(name: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    std::fs::copy(
        test_board("all-features.PcbDoc"),
        dir.join("all-features.PcbDoc"),
    )?;
    Ok(dir)
}

/// A project file text linking `link` and holding the parameter PCBNumber = `value`.
#[cfg(feature = "altium")]
fn project_text(link: &str, value: &str) -> String {
    format!(
        "\u{feff}[Design]\r\nVersion=1.0\r\n\r\n[Document1]\r\nDocumentPath={link}\r\n\r\n\
         [Parameter1]\r\nName=PCBNumber\r\nValue={value}\r\n"
    )
}

/// The files of a package: (name, contents), sorted.
#[cfg(any(feature = "altium", feature = "kicad"))]
type Files = Vec<(String, Vec<u8>)>;

/// Reads `board` into a new folder under `dir`; the manifest and every file written.
#[cfg(any(feature = "altium", feature = "kicad"))]
fn read_files(
    board: &std::path::Path,
    dir: &std::path::Path,
    extra: &[&str],
) -> Result<(serde_json::Value, Files), Box<dyn std::error::Error>> {
    let out = dir.join(format!("out{}", extra.len()));
    let mut args = vec![
        "read".to_string(),
        board.to_string_lossy().into_owned(),
        "--out".into(),
        out.to_string_lossy().into_owned(),
        "--json".into(),
    ];
    args.extend(extra.iter().map(|a| (*a).to_string()));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (ok, v) = run(&args)?;
    assert!(ok, "{v}");
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&out)? {
        let entry = entry?;
        // The manifest differs by the note on where the parameters came from.
        if entry.file_name() == "manifest.json" {
            continue;
        }
        files.push((
            entry.file_name().to_string_lossy().into_owned(),
            std::fs::read(entry.path())?,
        ));
    }
    files.sort();
    Ok((v, files))
}

#[cfg(feature = "altium")]
#[test]
fn a_board_alone_takes_the_parameters_of_the_project_beside_it(
) -> Result<(), Box<dyn std::error::Error>> {
    // all-features.PcbDoc's ".PCBNumber" and "Rev .BoardRevision" show DN-0001 and Rev A
    // (README.txt, step 10) without --project: the drawing is the same as with it.
    let board = test_board("all-features.PcbDoc");
    let dir = std::env::temp_dir().join(format!("de-read-beside-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let alone = read_files(&board, &dir, &[]);
    let with = read_files(
        &board,
        &dir,
        &[
            "--project",
            &test_board("all-features.PrjPcb").to_string_lossy(),
        ],
    );
    std::fs::remove_dir_all(&dir).ok();
    let ((alone_v, alone_files), (with_v, with_files)) = (alone?, with?);
    let notes = messages(&alone_v);
    assert!(
        notes.contains(&"parameters taken from all-features.PrjPcb next to the board".to_string()),
        "{notes:?}"
    );
    assert!(
        notes
            .iter()
            .any(|m| m.starts_with("6 text(s) refer to a project parameter")),
        "{notes:?}"
    );
    assert!(!alone_files.is_empty());
    assert_eq!(alone_files, with_files);
    // The manifests differ only by the note naming the project.
    let mut alone_notes = notes;
    alone_notes.retain(|m| !m.starts_with("parameters taken from"));
    assert_eq!(alone_notes, messages(&with_v));
    Ok(())
}

#[cfg(feature = "altium")]
#[test]
fn two_projects_linking_the_board_give_a_warning_and_no_parameters(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = folder_with_all_features("de-read-two")?;
    std::fs::write(
        dir.join("a.PrjPcb"),
        project_text("all-features.PcbDoc", "A-1"),
    )?;
    std::fs::write(
        dir.join("b.prjpcb"),
        project_text(".\\ALL-FEATURES.pcbdoc", "B-2"),
    )?;
    let result = read_files(&dir.join("all-features.PcbDoc"), &dir, &[]);
    std::fs::remove_dir_all(&dir).ok();
    let notes = messages(&result?.0);
    assert!(
        notes.iter().any(|m| m
            .starts_with("2 projects next to the board link it (a.PrjPcb, b.prjpcb)")
            && m.contains("--project")),
        "{notes:?}"
    );
    assert!(
        !notes.iter().any(|m| m.starts_with("parameters taken from")),
        "{notes:?}"
    );
    Ok(())
}

#[cfg(feature = "altium")]
#[test]
fn a_project_that_does_not_link_the_board_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
    let dir = folder_with_all_features("de-read-other")?;
    std::fs::write(
        dir.join("other.PrjPcb"),
        project_text("other.PcbDoc", "X-9"),
    )?;
    std::fs::write(
        dir.join("sub.PrjPcb"),
        project_text("sub\\all-features.PcbDoc", "X-8"),
    )?;
    let result = read_files(&dir.join("all-features.PcbDoc"), &dir, &[]);
    std::fs::remove_dir_all(&dir).ok();
    let notes = messages(&result?.0);
    assert!(
        !notes
            .iter()
            .any(|m| m.contains("next to the board") || m.contains("PrjPcb")),
        "{notes:?}"
    );
    Ok(())
}

#[cfg(feature = "altium")]
#[test]
fn a_huge_or_broken_project_beside_the_board_is_skipped_with_a_note(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = folder_with_all_features("de-read-huge")?;
    // A sparse file one byte over the limit on what is read; a file that is not a project.
    let huge = std::fs::File::create(dir.join("huge.PrjPcb"))?;
    huge.set_len((256 << 20) + 1)?;
    std::fs::write(dir.join("broken.PrjPcb"), "not a project")?;
    std::fs::write(
        dir.join("good.PrjPcb"),
        project_text("all-features.PcbDoc", "G-1"),
    )?;
    let result = read_files(&dir.join("all-features.PcbDoc"), &dir, &[]);
    std::fs::remove_dir_all(&dir).ok();
    let notes = messages(&result?.0);
    assert!(
        notes.iter().any(|m| m.starts_with(
            "the project huge.PrjPcb next to the board was not used: it is larger than"
        )),
        "{notes:?}"
    );
    assert!(
        notes
            .iter()
            .any(|m| m.starts_with("the project broken.PrjPcb next to the board was not used")),
        "{notes:?}"
    );
    assert!(
        notes.contains(&"parameters taken from good.PrjPcb next to the board".to_string()),
        "{notes:?}"
    );
    Ok(())
}

/// A new temporary folder `name` holding a copy of k10-text-vars.kicad_pcb, and its project
/// when `with_project`.
#[cfg(feature = "kicad")]
fn folder_with_text_vars(
    name: &str,
    with_project: bool,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    std::fs::copy(
        kicad_board("k10-text-vars.kicad_pcb"),
        dir.join("k10-text-vars.kicad_pcb"),
    )?;
    if with_project {
        std::fs::copy(
            kicad_board("k10-text-vars.kicad_pro"),
            dir.join("k10-text-vars.kicad_pro"),
        )?;
    }
    Ok(dir)
}

#[cfg(feature = "kicad")]
#[test]
fn a_kicad_board_takes_the_text_variables_of_its_project_beside_it(
) -> Result<(), Box<dyn std::error::Error>> {
    // k10-text-vars.README.txt: ${PCB_NUMBER} and ${BOARD_ID} come from the .kicad_pro of the
    // board's own name. Read alone, with --project naming it, and from a folder without it.
    let board = kicad_board("k10-text-vars.kicad_pcb");
    let bare = folder_with_text_vars("de-read-kicad-vars", false)?;
    let alone = read_files(&board, &bare, &[]);
    let with = read_files(
        &board,
        &bare,
        &[
            "--project",
            &kicad_board("k10-text-vars.kicad_pro").to_string_lossy(),
        ],
    );
    let without = read_files(&bare.join("k10-text-vars.kicad_pcb"), &bare, &["x"; 0]);
    let given = read_files(
        &board,
        &bare,
        &["--param", "PCB_NUMBER=X-1", "--param", "pcb_number=lower"],
    );
    std::fs::remove_dir_all(&bare).ok();
    let ((alone_v, alone_files), (with_v, with_files)) = (alone?, with?);
    let (without_v, without_files) = without?;
    let (given_v, given_files) = given?;
    let notes = messages(&alone_v);
    assert!(
        notes.contains(
            &"text variables taken from k10-text-vars.kicad_pro next to the board".to_string()
        ),
        "{notes:?}"
    );
    assert!(
        !notes.iter().any(|m| m.contains("${PCB_NUMBER}")),
        "{notes:?}"
    );
    // The texts are drawn in the manifest's overlays.
    assert_eq!(alone_files, with_files);
    assert_eq!(alone_v["overlays"], with_v["overlays"]);
    let mut alone_notes = notes;
    alone_notes.retain(|m| !m.starts_with("text variables taken from"));
    assert_eq!(alone_notes, messages(&with_v));
    // Without the project, the two variables it defines stay as typed.
    let notes = messages(&without_v);
    // PCB_NUMBER is in one board text and the two BUILD fields.
    for (name, n) in [("PCB_NUMBER", 3), ("BOARD_ID", 1)] {
        assert!(
            notes.iter().any(|m| m.starts_with(&format!(
                "{n} text(s) use the text variable ${{{name}}}, which neither"
            ))),
            "{notes:?}"
        );
    }
    assert_eq!(without_files, alone_files);
    assert_ne!(without_v["overlays"], alone_v["overlays"]);
    // --param wins over the project; names are case-sensitive, so pcb_number is a new one.
    assert_eq!(given_files, alone_files);
    assert_ne!(given_v["overlays"], alone_v["overlays"]);
    assert_eq!(messages(&given_v), messages(&alone_v));
    Ok(())
}

#[cfg(feature = "kicad")]
#[test]
fn a_kicad_project_beside_the_board_that_is_not_json_gives_a_warning(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = folder_with_text_vars("de-read-kicad-badpro", false)?;
    std::fs::write(dir.join("k10-text-vars.kicad_pro"), "(not json")?;
    let result = read_files(&dir.join("k10-text-vars.kicad_pcb"), &dir, &[]);
    std::fs::remove_dir_all(&dir).ok();
    let notes = messages(&result?.0);
    assert!(
        notes.iter().any(|m| m.starts_with(
            "the project k10-text-vars.kicad_pro next to the board was not used: not a KiCad project file"
        )),
        "{notes:?}"
    );
    Ok(())
}

/// body-arc-outline: R1's extruded body is a rounded slot with ends of radius 0.4, so its STEP
/// file has two exact half cylinders of that radius, not facets.
#[cfg(feature = "altium")]
#[test]
fn read_writes_an_extruded_body_with_arcs_as_exact_cylinders(
) -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::temp_dir().join(format!("de-arc-body-{}", std::process::id()));
    let board = test_board("body-arc-outline.PcbDoc");
    let result = run(&[
        "read",
        &board.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    let step = std::fs::read_to_string(out.join("model-1.step"));
    std::fs::remove_dir_all(&out).ok();
    let (ok, v) = result?;
    assert!(ok, "{v}");
    let r1 = v["parts"]
        .as_array()
        .and_then(|p| p.iter().find(|p| p["name"] == "R1"))
        .ok_or("no R1")?;
    assert_eq!(r1["model"], "model-1");
    let step = step?;
    let radii: Vec<f64> = step
        .lines()
        .filter(|l| l.contains("CYLINDRICAL_SURFACE("))
        .filter_map(|l| l.rsplit(',').next())
        .filter_map(|r| r.trim_end_matches(");").parse().ok())
        .collect();
    assert_eq!(radii.len(), 2, "{radii:?}");
    assert!(radii.iter().all(|r| (r - 0.4).abs() < 0.001), "{radii:?}");
    Ok(())
}

/// pour-modes (tests/boards/README.txt): the Top copper of each 10 x 10 pour, y 10 to 20. Solid
/// (x 2 to 12) is the whole 100.000 mm2. The 90 degree hatch (x 14 to 24) of 0.300 tracks on a
/// 1.000 grid leaves 100 open squares of 0.700, 49.000 mm2. The None pour (x 38 to 48) is only
/// its outline: one ring of 0.300 tracks around a single opening of 9.700 x 9.700, 94.090 mm2,
/// so nothing inside it is copper. The ring's outer edge has round corners, drawn within the
/// package's 0.02 mm, so its area is checked to 0.005 mm2 (10.3 x 10.3 less (4 - pi) 0.15^2).
#[cfg(feature = "altium")]
#[test]
fn read_draws_hatched_and_outline_pours_as_altium_pours_them(
) -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::temp_dir().join(format!("de-pours-{}", std::process::id()));
    let board = test_board("pour-modes.PcbDoc");
    let result = run(&[
        "read",
        &board.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    std::fs::remove_dir_all(&out).ok();
    let (ok, v) = result?;
    assert!(ok, "{v}");
    let ring = |r: &serde_json::Value| -> Vec<(f64, f64)> {
        r.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?)))
                    .collect()
            })
            .unwrap_or_default()
    };
    let area = |r: &[(f64, f64)]| {
        let n = r.len();
        (0..n)
            .map(|i| {
                let (a, b) = (r[i], r[(i + 1) % n]);
                a.0 * b.1 - b.0 * a.1
            })
            .sum::<f64>()
            .abs()
            / 2.0
    };
    let copper = v["overlays"]
        .as_array()
        .and_then(|o| o.iter().find(|o| o["name"] == "Copper top"))
        .ok_or("no top copper")?;
    let areas = copper["areas"].as_array().ok_or("no areas")?;
    assert_eq!(areas.len(), 4, "one copper area per pour");
    // (left x, outer area, tolerance, hole count, holes' area) by pour.
    let mut found: Vec<(f64, f64, usize, f64)> = areas
        .iter()
        .map(|a| {
            let outer = ring(&a["outer"]);
            let holes: Vec<Vec<(f64, f64)>> = a["holes"]
                .as_array()
                .map(|h| h.iter().map(ring).collect())
                .unwrap_or_default();
            let left = outer.iter().map(|p| p.0).fold(f64::MAX, f64::min);
            (
                left,
                area(&outer),
                holes.len(),
                holes.iter().map(|h| area(h)).sum(),
            )
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    let [solid, h90, _h45, none] = found.as_slice() else {
        return Err(format!("{found:?}").into());
    };
    assert!((solid.1 - 100.0).abs() < 0.001 && solid.2 == 0, "{solid:?}");
    assert!(h90.2 == 100 && (h90.3 - 49.0).abs() < 0.001, "{h90:?}");
    let outer = 10.3 * 10.3 - (4.0 - std::f64::consts::PI) * 0.15 * 0.15;
    assert!((none.1 - outer).abs() < 0.005, "{none:?}");
    assert!(
        none.2 == 1 && (none.3 - 9.7 * 9.7).abs() < 0.001,
        "{none:?}"
    );
    Ok(())
}

/// teardrops: the seven teardrops join their pads, vias and tracks into one area of top copper,
/// from pad "1" (round 1.600 at (10, 15)) to the track's end at (40, 22) and its round cap.
#[cfg(feature = "altium")]
#[test]
fn read_unites_teardrops_with_their_copper() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::temp_dir().join(format!("de-teardrops-{}", std::process::id()));
    let board = test_board("teardrops.PcbDoc");
    let result = run(&[
        "read",
        &board.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    std::fs::remove_dir_all(&out).ok();
    let (ok, v) = result?;
    assert!(ok, "{v}");
    let copper = v["overlays"]
        .as_array()
        .and_then(|o| o.iter().find(|o| o["name"] == "Copper top"))
        .ok_or("no top copper")?;
    let areas = copper["areas"].as_array().ok_or("no areas")?;
    assert_eq!(areas.len(), 1, "one united area");
    let outer: Vec<(f64, f64)> = areas[0]["outer"]
        .as_array()
        .ok_or("no outer ring")?
        .iter()
        .filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?)))
        .collect();
    let lo = |f: fn(&(f64, f64)) -> f64| outer.iter().map(f).fold(f64::MAX, f64::min);
    let hi = |f: fn(&(f64, f64)) -> f64| outer.iter().map(f).fold(f64::MIN, f64::max);
    let b = [lo(|p| p.0), hi(|p| p.0), lo(|p| p.1), hi(|p| p.1)];
    // x from the pad's left edge 9.2 to the track's right edge 40.127 (the via ring 40.3), y from
    // the pad's bottom 14.2 to the cap at 22.127.
    let want = [9.2, 40.3, 14.2, 22.127];
    assert!(
        b.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.001),
        "{b:?}"
    );
    Ok(())
}

/// pad-shapes-custom: the finish and the mask opening of each pad follow its real outline, not
/// the 1 mil square pads 2 and 3 store: each opening is the pad grown by the board's 0.1 mask
/// expansion on every side.
#[cfg(feature = "altium")]
#[test]
fn read_opens_the_mask_round_chamfered_and_custom_pads() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::temp_dir().join(format!("de-custom-pads-{}", std::process::id()));
    let board = test_board("pad-shapes-custom.PcbDoc");
    let result = run(&[
        "read",
        &board.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    std::fs::remove_dir_all(&out).ok();
    let (ok, v) = result?;
    assert!(ok, "{v}");
    let ring_box = |r: &serde_json::Value| -> Option<[f64; 4]> {
        let pts: Vec<(f64, f64)> = r
            .as_array()?
            .iter()
            .filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?)))
            .collect();
        Some([
            pts.iter().map(|p| p.0).fold(f64::MAX, f64::min),
            pts.iter().map(|p| p.0).fold(f64::MIN, f64::max),
            pts.iter().map(|p| p.1).fold(f64::MAX, f64::min),
            pts.iter().map(|p| p.1).fold(f64::MIN, f64::max),
        ])
    };
    let overlay = |name: &str| {
        v["overlays"]
            .as_array()
            .and_then(|o| o.iter().find(|o| o["name"] == name))
            .cloned()
    };
    let near = |a: &[f64; 4], b: &[f64; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.001);
    let mut openings: Vec<[f64; 4]> = overlay("Solder mask top").ok_or("no top mask")?["areas"][0]
        ["holes"]
        .as_array()
        .ok_or("no openings")?
        .iter()
        .filter_map(ring_box)
        .collect();
    openings.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let want = [
        [8.9, 11.1, 14.4, 15.6],
        [18.9, 21.1, 14.4, 15.6],
        [29.9, 33.1, 13.9, 17.1],
    ];
    assert_eq!(openings.len(), 3, "{openings:?}");
    for (o, w) in openings.iter().zip(&want) {
        assert!(near(o, w), "{o:?}, want {w:?}");
    }
    let mut finish: Vec<[f64; 4]> = overlay("Finish top").ok_or("no top finish")?["areas"]
        .as_array()
        .ok_or("no finish")?
        .iter()
        .filter_map(|a| ring_box(&a["outer"]))
        .collect();
    finish.sort_by(|a, b| a[0].total_cmp(&b[0]));
    assert_eq!(finish.len(), 3, "{finish:?}");
    assert!(near(&finish[1], &[19.0, 21.0, 14.5, 15.5]), "{finish:?}");
    assert!(near(&finish[2], &[30.0, 33.0, 14.0, 17.0]), "{finish:?}");
    Ok(())
}

/// Each model file of a package: its id and its text.
#[cfg(feature = "altium")]
type ModelFiles = Vec<(String, String)>;

/// Reads an Altium test board into a package and returns the manifest and each model file's text.
#[cfg(feature = "altium")]
fn package_of(name: &str) -> Result<(serde_json::Value, ModelFiles), Box<dyn std::error::Error>> {
    let out = std::env::temp_dir().join(format!(
        "de-{}-{}",
        name.replace('.', "-"),
        std::process::id()
    ));
    let board = test_board(name);
    let result = run(&[
        "read",
        &board.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    let mut files = Vec::new();
    if let Ok((_, v)) = &result {
        for m in v["models"].as_array().into_iter().flatten() {
            let (Some(id), Some(file)) = (m["id"].as_str(), m["file"].as_str()) else {
                continue;
            };
            if let Ok(text) = std::fs::read_to_string(out.join(file)) {
                files.push((id.to_string(), text));
            }
        }
    }
    std::fs::remove_dir_all(&out).ok();
    let (ok, v) = result?;
    assert!(ok, "{v}");
    Ok((v, files))
}

/// The radii of a STEP file's surfaces of one kind (`SPHERICAL_SURFACE(`, `CYLINDRICAL_SURFACE(`).
#[cfg(feature = "altium")]
fn surface_radii(step: &str, kind: &str) -> Vec<f64> {
    step.lines()
        .filter(|l| l.contains(kind))
        .filter_map(|l| l.rsplit(',').next())
        .filter_map(|r| r.trim_end_matches(");").parse().ok())
        .collect()
}

/// body-sphere, body-two-extruded and arc-region: one model per body. R1's sphere is one exact
/// sphere of radius 0.5; R1's two boxes are two parts with their own models and colors; the
/// free rounded slot is "Free body 1" in "Free bodies", with two exact half cylinders of radius
/// 2.
#[cfg(feature = "altium")]
#[test]
fn read_writes_spheres_further_bodies_and_free_bodies() -> Result<(), Box<dyn std::error::Error>> {
    let part = |v: &serde_json::Value, name: &str| -> Option<serde_json::Value> {
        v["parts"]
            .as_array()?
            .iter()
            .find(|p| p["name"] == name)
            .cloned()
    };
    let file = |files: &[(String, String)], id: &serde_json::Value| -> Option<String> {
        files
            .iter()
            .find(|(i, _)| Some(i.as_str()) == id.as_str())
            .map(|(_, t)| t.clone())
    };

    let (v, files) = package_of("body-sphere.PcbDoc")?;
    let r1 = part(&v, "R1").ok_or("no R1")?;
    let step = file(&files, &r1["model"]).ok_or("no sphere file")?;
    let radii = surface_radii(&step, "SPHERICAL_SURFACE(");
    assert!(
        radii.len() == 1 && (radii[0] - 0.5).abs() < 0.001,
        "{radii:?}"
    );

    let (v, files) = package_of("body-two-extruded.PcbDoc")?;
    let main = part(&v, "R1").ok_or("no R1")?;
    let second = part(&v, "R1 body 2").ok_or("no R1 body 2")?;
    assert_ne!(main["model"], second["model"], "one model per body");
    assert_eq!(
        (main["color"].as_str(), second["color"].as_str()),
        (Some("#808080"), Some("#00a0e0"))
    );
    assert!(file(&files, &main["model"]).is_some() && file(&files, &second["model"]).is_some());

    let (v, files) = package_of("arc-region.PcbDoc")?;
    let free = part(&v, "Free body 1").ok_or("no free body")?;
    assert_eq!(free["group"], "Free bodies");
    let step = file(&files, &free["model"]).ok_or("no free body file")?;
    let radii = surface_radii(&step, "CYLINDRICAL_SURFACE(");
    assert_eq!(radii.len(), 2, "{radii:?}");
    assert!(radii.iter().all(|r| (r - 2.0).abs() < 0.001), "{radii:?}");
    Ok(())
}
