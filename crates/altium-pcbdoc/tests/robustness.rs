//! A hostile file must never crash the reader (AGENTS.md rule 5). This test takes every test
//! board, damages it in many reproducible ways and checks that `Altium::read` always returns,
//! either a board or an error, and never panics.
//!
//! Two kinds of damage:
//! - inside the container: one stream's bytes are changed (bytes flipped, a 4-byte field set to a
//!   huge value, the stream cut short or emptied) and the container is rebuilt, so the record
//!   parsers see the damage;
//! - the raw file: bytes flipped anywhere, or the file cut short, so the container handling sees
//!   it.
//!
//! The random generator has a fixed seed, so a failure names a board, a case and a seed that
//! reproduce it. `ROBUSTNESS_CASES` (default 40 per board and kind) runs more cases.

use std::io::{Cursor, Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use altium_pcbdoc::Altium;
use board_model::InputFormat;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// A small xorshift generator: deterministic, no dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// A number in `0..n` (0 when `n` is 0).
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            usize::try_from(self.next() % n as u64).unwrap_or(0)
        }
    }
}

fn boards() -> Result<Vec<(String, Vec<u8>)>> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/boards");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pcbdoc"))
        {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            found.push((name, std::fs::read(&path)?));
        }
    }
    found.sort();
    Ok(found)
}

fn cases() -> usize {
    std::env::var("ROBUSTNESS_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
}

/// Every stream of a container, as (path parts, bytes).
fn streams(bytes: &[u8]) -> Result<Vec<(Vec<String>, Vec<u8>)>> {
    let mut cf = cfb::CompoundFile::open(Cursor::new(bytes))?;
    let paths: Vec<PathBuf> = cf
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| e.path().to_path_buf())
        .collect();
    let mut out = Vec::new();
    for path in paths {
        let mut data = Vec::new();
        cf.open_stream(&path)?.read_to_end(&mut data)?;
        let parts = path
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(n) => Some(n.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect();
        out.push((parts, data));
    }
    Ok(out)
}

/// A new container holding these streams.
fn container(streams: &[(Vec<String>, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut cf = cfb::CompoundFile::create(Cursor::new(Vec::new()))?;
    for (parts, data) in streams {
        let mut path = PathBuf::from("/");
        for (i, part) in parts.iter().enumerate() {
            path.push(part);
            if i + 1 < parts.len() && !cf.exists(&path) {
                cf.create_storage(&path)?;
            }
        }
        cf.create_stream(&path)?.write_all(data)?;
    }
    cf.flush()?;
    Ok(cf.into_inner().into_inner())
}

/// Damages `data` in one of several ways.
fn damage(data: &mut Vec<u8>, rng: &mut Rng) {
    match rng.below(5) {
        0 => {
            for _ in 0..=rng.below(8) {
                let at = rng.below(data.len());
                if let Some(b) = data.get_mut(at) {
                    *b ^= 1 << rng.below(8);
                }
            }
        }
        1 => {
            // A length or count field set to a huge value.
            let at = rng.below(data.len());
            let value: [u8; 4] = match rng.below(3) {
                0 => [0xff, 0xff, 0xff, 0xff],
                1 => [0xff, 0xff, 0xff, 0x7f],
                _ => [0x00, 0x00, 0x00, 0x80],
            };
            for (i, v) in value.iter().enumerate() {
                if let Some(b) = data.get_mut(at + i) {
                    *b = *v;
                }
            }
        }
        2 => data.truncate(rng.below(data.len())),
        3 => data.clear(),
        _ => {
            // A run of bytes set to one value.
            let at = rng.below(data.len());
            let len = rng.below(64);
            let value = u8::try_from(rng.below(256)).unwrap_or(0);
            for b in data.iter_mut().skip(at).take(len) {
                *b = value;
            }
        }
    }
}

/// Reads `bytes`; `Err` with a message only when the reader panicked.
fn read_without_panic(bytes: &[u8]) -> std::result::Result<(), String> {
    catch_unwind(AssertUnwindSafe(|| {
        let _ = Altium.read(bytes);
    }))
    .map_err(|p| {
        p.downcast_ref::<String>()
            .cloned()
            .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "panic".into())
    })
}

#[test]
fn damaged_streams_never_crash_the_reader() -> Result<()> {
    let mut failures = Vec::new();
    for (board, bytes) in boards()? {
        let original = streams(&bytes)?;
        // Only streams with data are worth damaging.
        let targets: Vec<usize> = original
            .iter()
            .enumerate()
            .filter(|(_, (_, d))| !d.is_empty())
            .map(|(i, _)| i)
            .collect();
        for case in 0..cases() {
            let seed = 0x9e37_79b9_7f4a_7c15 ^ (case as u64 + 1);
            let mut rng = Rng(seed);
            let mut damaged = original.clone();
            let Some(&target) = targets.get(rng.below(targets.len())) else {
                continue;
            };
            let name = damaged
                .get(target)
                .map(|(p, _)| p.join("/"))
                .unwrap_or_default();
            if let Some((_, data)) = damaged.get_mut(target) {
                damage(data, &mut rng);
            }
            if let Err(why) = read_without_panic(&container(&damaged)?) {
                failures.push(format!("{board}, stream /{name}, seed {seed:#x}: {why}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "the reader panicked:\n{}",
        failures.join("\n")
    );
    Ok(())
}

#[test]
fn damaged_files_never_crash_the_reader() -> Result<()> {
    let mut failures = Vec::new();
    for (board, bytes) in boards()? {
        for case in 0..cases() {
            let seed = 0x2545_f491_4f6c_dd1d ^ (case as u64 + 1);
            let mut rng = Rng(seed);
            let mut damaged = bytes.clone();
            damage(&mut damaged, &mut rng);
            if let Err(why) = read_without_panic(&damaged) {
                failures.push(format!("{board}, raw file, seed {seed:#x}: {why}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "the reader panicked:\n{}",
        failures.join("\n")
    );
    Ok(())
}
