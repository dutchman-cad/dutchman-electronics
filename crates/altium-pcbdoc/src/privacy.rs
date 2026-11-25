//! Privacy check: finds text inside a board file that could point back to a person, a company
//! or a computer, before the file is shared.
//!
//! It knows nothing about what any Altium stream means. It reads every stream in the container
//! and looks for text, both as single bytes (ASCII / UTF-8) and as two-byte UTF-16 (little end
//! first), then reports:
//! - file paths (`C:\...`, `\\server\...`, `\Users\`, `/home/`),
//! - e-mail addresses,
//! - default Windows computer names (`DESKTOP-...`, `LAPTOP-...`),
//! - every word the caller asks to watch for (a name, a company, a licence number), matched
//!   without regard to upper or lower case, in either text encoding.
//!
//! The watched words come from the person running the check and are never stored anywhere.
//!
//! One stream is read with knowledge of its layout: `/EmbeddedFonts6/Data` holds font files
//! packed with zlib, whose bytes look like random text and would hide real text. It is checked
//! as its font names plus each font file unpacked (see `fonts.rs`). A TrueType file's text is in
//! its `name` table (the OpenType specification), so those strings are checked for paths,
//! e-mail addresses and computer names, and the whole file for watched words; the rest of a
//! font file is binary that would only give false alarms. A stream that cannot be split into
//! fonts, or a font that is not a TrueType file, is checked byte for byte like any other stream.

use std::io::{Cursor, Read};

use board_model::{ReadError, MAX_INPUT_BYTES};

/// The most findings reported for one file; the rest are counted, not listed.
pub const MAX_FINDINGS: usize = 1_000;
/// Text runs shorter than this are not reported (binary noise).
const MIN_RUN: usize = 4;
/// Longest text kept per finding, in characters.
const MAX_SHOWN: usize = 200;
/// Watched words shorter than this are refused: they would match everywhere.
pub const MIN_WATCH_LEN: usize = 3;
/// The most entries a container may hold before it is refused as hostile.
const MAX_ENTRIES: usize = 100_000;

/// What kind of identifying text was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A file or folder path.
    Path,
    /// An e-mail address.
    Email,
    /// A default Windows computer name.
    ComputerName,
    /// One of the caller's watched words (the word itself, as given).
    Watched(String),
    /// Any text at all (only with [`Options::all_text`]), for looking through by eye.
    Text,
}

/// One place in the file with identifying text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// The stream it is in, e.g. "/FileHeader".
    pub stream: String,
    /// Byte offset inside that stream.
    pub offset: usize,
    pub kind: Kind,
    /// The text around it, shortened.
    pub text: String,
}

/// What to look for.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Words to search for (a name, a company, a licence number), any case.
    pub watch: Vec<String>,
    /// Also list every text run, so a person can read through it.
    pub all_text: bool,
}

/// The result of a check.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub findings: Vec<Finding>,
    /// Findings past [`MAX_FINDINGS`] that were counted but not listed.
    pub not_listed: usize,
}

impl Report {
    fn push(&mut self, finding: Finding) {
        if self.findings.len() < MAX_FINDINGS {
            self.findings.push(finding);
        } else {
            self.not_listed = self.not_listed.saturating_add(1);
        }
    }

    /// True when nothing identifying was found (plain text listed by `all_text` does not count).
    pub fn is_clean(&self) -> bool {
        self.not_listed == 0 && self.findings.iter().all(|f| f.kind == Kind::Text)
    }
}

/// Checks every stream of an OLE container (a `.PcbDoc`).
pub fn check(bytes: &[u8], options: &Options) -> Result<Report, ReadError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(ReadError::TooLarge { bytes: bytes.len() });
    }
    for word in &options.watch {
        if word.trim().chars().count() < MIN_WATCH_LEN {
            return Err(ReadError::Invalid(format!(
                "a watched word needs at least {MIN_WATCH_LEN} characters"
            )));
        }
    }
    let mut container = cfb::CompoundFile::open(Cursor::new(bytes))
        .map_err(|e| ReadError::Invalid(e.to_string()))?;
    let mut paths = Vec::new();
    for (seen, entry) in container.walk().enumerate() {
        if seen >= MAX_ENTRIES {
            return Err(ReadError::Invalid("too many entries".into()));
        }
        if entry.is_stream() {
            paths.push(entry.path().to_path_buf());
        }
    }
    paths.sort();
    let mut report = Report::default();
    // The streams together can never be bigger than the file, which is already bounded.
    for path in paths {
        let name = crate::container_path(&path);
        let mut data = Vec::new();
        container
            .open_stream(&path)
            .and_then(|s| s.take(MAX_INPUT_BYTES as u64).read_to_end(&mut data))
            .map_err(|e| ReadError::Invalid(format!("{name}: {e}")))?;
        if name == FONTS_STREAM {
            check_fonts(&name, &data, options, &mut report);
        } else {
            check_stream(&name, &data, options, &mut report);
        }
    }
    Ok(report)
}

/// The stream that holds embedded font files, packed.
const FONTS_STREAM: &str = "/EmbeddedFonts6/Data";

/// Checks each embedded font's names and its unpacked file, or the raw stream when it cannot be
/// split into fonts or a font cannot be unpacked. A font is unpacked to at most the input limit.
fn check_fonts(stream: &str, data: &[u8], options: &Options, report: &mut Report) {
    let Ok(records) = crate::fonts::records(data) else {
        check_stream(stream, data, options, report);
        return;
    };
    let mut files = Vec::with_capacity(records.len());
    for r in &records {
        match crate::fonts::unpack(r, MAX_INPUT_BYTES) {
            Ok(file) => files.push(file),
            Err(_) => {
                check_stream(stream, data, options, report);
                return;
            }
        }
    }
    for (i, (r, file)) in records.iter().zip(&files).enumerate() {
        check_stream(
            &format!("{stream} font {i} names"),
            r.names,
            options,
            report,
        );
        let name = format!("{stream} font {i} unpacked");
        match truetype_names(file) {
            Some(texts) => {
                check_texts(&name, texts, options, report);
                check_watched(&name, file, options, report);
            }
            None => check_stream(&name, file, options, report),
        }
    }
}

/// The strings of a TrueType file's `name` table, as (offset in the file, text), or `None` when
/// the file is not a TrueType file with a readable `name` table. Windows and Unicode strings are
/// UTF-16 with the high byte first; Macintosh ones are read one byte per character.
fn truetype_names(file: &[u8]) -> Option<Vec<(usize, String)>> {
    let u16_at = |at: usize| -> Option<usize> {
        let b = file.get(at..at.checked_add(2)?)?;
        Some(usize::from(u16::from_be_bytes(b.try_into().ok()?)))
    };
    let u32_at = |at: usize| -> Option<usize> {
        let b = file.get(at..at.checked_add(4)?)?;
        usize::try_from(u32::from_be_bytes(b.try_into().ok()?)).ok()
    };
    if !matches!(file.get(..4), Some([0, 1, 0, 0] | b"true" | b"OTTO")) {
        return None;
    }
    let tables = u16_at(4)?;
    let table = (0..tables).find_map(|i| {
        let at = i.checked_mul(16)?.checked_add(12)?;
        (file.get(at..at.checked_add(4)?)? == b"name").then_some(at)
    })?;
    let name_at = u32_at(table.checked_add(8)?)?;
    let count = u16_at(name_at.checked_add(2)?)?;
    let strings_at = name_at.checked_add(u16_at(name_at.checked_add(4)?)?)?;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let rec = name_at.checked_add(6)?.checked_add(i.checked_mul(12)?)?;
        let platform = u16_at(rec)?;
        let len = u16_at(rec.checked_add(8)?)?;
        let start = strings_at.checked_add(u16_at(rec.checked_add(10)?)?)?;
        let bytes = file.get(start..start.checked_add(len)?)?;
        let text = if platform == 1 {
            bytes.iter().map(|&b| char::from(b)).collect()
        } else {
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&pair| u16::from_be_bytes(pair))
                .collect();
            String::from_utf16_lossy(&units)
        };
        out.push((start, text));
    }
    Some(out)
}

/// Checks the bytes of one stream.
pub fn check_stream(stream: &str, data: &[u8], options: &Options, report: &mut Report) {
    check_texts(stream, text_runs(data), options, report);
    check_watched(stream, data, options, report);
}

/// Reports what each (offset, text) contains, and lists the text with [`Options::all_text`].
fn check_texts(stream: &str, texts: Vec<(usize, String)>, options: &Options, report: &mut Report) {
    for (offset, text) in texts {
        for kind in classify(&text) {
            report.push(Finding {
                stream: stream.to_string(),
                offset,
                kind,
                text: shorten(&text),
            });
        }
        if options.all_text {
            report.push(Finding {
                stream: stream.to_string(),
                offset,
                kind: Kind::Text,
                text: shorten(&text),
            });
        }
    }
}

/// Reports every watched word in the bytes, in either text encoding.
fn check_watched(stream: &str, data: &[u8], options: &Options, report: &mut Report) {
    if options.watch.is_empty() {
        return;
    }
    let folded = data.to_ascii_lowercase();
    for word in &options.watch {
        let word = word.trim();
        let utf8 = word.to_lowercase().into_bytes();
        let utf16: Vec<u8> = word
            .to_lowercase()
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        for pattern in [utf8, utf16] {
            for offset in find_all(&folded, &pattern) {
                let end = offset.saturating_add(pattern.len()).saturating_add(40);
                let context = data
                    .get(offset.saturating_sub(40)..end.min(data.len()))
                    .map(readable)
                    .unwrap_or_default();
                report.push(Finding {
                    stream: stream.to_string(),
                    offset,
                    kind: Kind::Watched(word.to_string()),
                    text: shorten(&context),
                });
            }
        }
    }
}

/// Every start offset of `needle` in `haystack`. An empty needle matches nothing.
fn find_all(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() {
        return Vec::new();
    }
    haystack
        .windows(needle.len())
        .enumerate()
        .filter(|(_, w)| *w == needle)
        .map(|(i, _)| i)
        .collect()
}

/// Bytes as text a person can read: printable characters kept, the rest shown as dots.
fn readable(bytes: &[u8]) -> String {
    bytes
        .iter()
        .filter(|b| **b != 0)
        .map(|b| {
            if b.is_ascii_graphic() || *b == b' ' {
                char::from(*b)
            } else {
                '.'
            }
        })
        .collect()
}

fn shorten(text: &str) -> String {
    if text.chars().count() <= MAX_SHOWN {
        text.to_string()
    } else {
        let mut s: String = text.chars().take(MAX_SHOWN).collect();
        s.push('…');
        s
    }
}

/// A byte that can be part of single-byte text: printable ASCII, or any byte of a UTF-8
/// character beyond ASCII (names with accents).
fn is_text_byte(b: u8) -> bool {
    (0x20..0x7f).contains(&b) || b >= 0x80
}

/// Runs of text in the data, as (offset, text): single-byte runs, then UTF-16 runs at both
/// even and odd starts.
pub fn text_runs(data: &[u8]) -> Vec<(usize, String)> {
    let mut runs = Vec::new();
    // Single-byte text.
    let mut start = None;
    for (i, b) in data.iter().enumerate() {
        match (is_text_byte(*b), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                push_run(&mut runs, s, data.get(s..i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        push_run(&mut runs, s, data.get(s..));
    }
    // Two-byte text, low byte first.
    for align in 0..2usize {
        let Some(tail) = data.get(align..) else {
            continue;
        };
        let mut chars = String::new();
        let mut run_start = 0usize;
        for (k, pair) in tail.as_chunks::<2>().0.iter().enumerate() {
            let unit = u16::from_le_bytes(*pair);
            let c = char::from_u32(u32::from(unit)).filter(|c| !c.is_control());
            match c {
                Some(c) => {
                    if chars.is_empty() {
                        run_start = align.saturating_add(k.saturating_mul(2));
                    }
                    chars.push(c);
                }
                None => {
                    take_utf16_run(&mut runs, run_start, &mut chars);
                }
            }
        }
        take_utf16_run(&mut runs, run_start, &mut chars);
    }
    runs
}

fn push_run(runs: &mut Vec<(usize, String)>, start: usize, bytes: Option<&[u8]>) {
    if let Some(bytes) = bytes {
        if bytes.len() >= MIN_RUN {
            runs.push((start, String::from_utf8_lossy(bytes).into_owned()));
        }
    }
}

/// Keeps a UTF-16 run only when it is mostly ASCII-range characters (real text, not two
/// single-byte characters read as one).
fn take_utf16_run(runs: &mut Vec<(usize, String)>, start: usize, chars: &mut String) {
    let count = chars.chars().count();
    let ascii = chars.chars().filter(char::is_ascii).count();
    if count >= MIN_RUN && ascii.saturating_mul(2) >= count {
        runs.push((start, std::mem::take(chars)));
    } else {
        chars.clear();
    }
}

/// What identifying kinds a piece of text contains.
pub fn classify(text: &str) -> Vec<Kind> {
    let mut kinds = Vec::new();
    if looks_like_path(text) {
        kinds.push(Kind::Path);
    }
    if text
        .split(|c: char| c.is_whitespace() || "<>()[]{};,\"'|=".contains(c))
        .any(looks_like_email)
    {
        kinds.push(Kind::Email);
    }
    let upper = text.to_ascii_uppercase();
    if ["DESKTOP-", "LAPTOP-"].iter().any(|p| upper.contains(p)) {
        kinds.push(Kind::ComputerName);
    }
    kinds
}

fn looks_like_path(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let drive = text
        .as_bytes()
        .windows(3)
        .any(|w| matches!(w, [d, b':', b'\\'] if d.is_ascii_alphabetic()));
    drive
        || text.contains("\\\\")
        || lower.contains("\\users\\")
        || lower.contains("/users/")
        || lower.contains("/home/")
}

fn looks_like_email(token: &str) -> bool {
    let Some((local, domain)) = token.split_once('@') else {
        return false;
    };
    let domain = domain.trim_end_matches('.');
    !local.is_empty()
        && local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._%+-".contains(c))
        && domain.contains('.')
        && !domain.starts_with('.')
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    /// A container with a path, an e-mail, a computer name and a name hidden in UTF-16.
    fn leaky() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut cf = cfb::CompoundFile::create(Cursor::new(Vec::new()))?;
        cf.create_storage("/Board6")?;
        let mut header = b"\x01\x02SAVED=C:\\Work\\Boards\\x.PcbDoc\x00\x00".to_vec();
        header.extend_from_slice(b"\x00by test.person@example.com\x00");
        cf.create_stream("/FileHeader")?.write_all(&header)?;
        let mut board = vec![0u8, 1, 2];
        board.extend(utf16("OWNER=Jane Testerson|HOST=DESKTOP-AB12CD"));
        board.extend([0u8, 0, 7, 7]);
        cf.create_stream("/Board6/Data")?.write_all(&board)?;
        cf.flush()?;
        Ok(cf.into_inner().into_inner())
    }

    fn clean() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut cf = cfb::CompoundFile::create(Cursor::new(Vec::new()))?;
        cf.create_stream("/FileHeader")?
            .write_all(b"PCB 6.0 Binary File\x00LAYER=TOP|WIDTH=10mil")?;
        cf.flush()?;
        Ok(cf.into_inner().into_inner())
    }

    #[test]
    fn finds_path_email_and_computer_name() -> Result<(), Box<dyn std::error::Error>> {
        let report = check(&leaky()?, &Options::default())?;
        let kinds: Vec<(&str, &Kind)> = report
            .findings
            .iter()
            .map(|f| (f.stream.as_str(), &f.kind))
            .collect();
        assert!(kinds.contains(&("/FileHeader", &Kind::Path)), "{kinds:?}");
        assert!(kinds.contains(&("/FileHeader", &Kind::Email)), "{kinds:?}");
        assert!(
            kinds.contains(&("/Board6/Data", &Kind::ComputerName)),
            "{kinds:?}"
        );
        assert!(!report.is_clean());
        Ok(())
    }

    #[test]
    fn finds_watched_words_in_utf16_any_case() -> Result<(), Box<dyn std::error::Error>> {
        let options = Options {
            watch: vec!["testerson".into()],
            all_text: false,
        };
        let report = check(&leaky()?, &options)?;
        let watched: Vec<&Finding> = report
            .findings
            .iter()
            .filter(|f| f.kind == Kind::Watched("testerson".into()))
            .collect();
        assert_eq!(watched.len(), 1, "{:?}", report.findings);
        assert_eq!(watched[0].stream, "/Board6/Data");
        assert!(watched[0].text.contains("Testerson"));
        Ok(())
    }

    #[test]
    fn a_clean_file_is_clean() -> Result<(), Box<dyn std::error::Error>> {
        let options = Options {
            watch: vec!["Testerson".into()],
            all_text: false,
        };
        assert!(check(&clean()?, &options)?.is_clean());
        Ok(())
    }

    #[test]
    fn all_text_lists_text_but_stays_clean() -> Result<(), Box<dyn std::error::Error>> {
        let options = Options {
            watch: vec![],
            all_text: true,
        };
        let report = check(&clean()?, &options)?;
        assert!(report
            .findings
            .iter()
            .any(|f| f.text.contains("WIDTH=10mil")));
        assert!(report.is_clean());
        Ok(())
    }

    #[test]
    fn short_watch_words_and_garbage_are_errors() -> Result<(), Box<dyn std::error::Error>> {
        let options = Options {
            watch: vec!["ab".into()],
            all_text: false,
        };
        assert!(check(&clean()?, &options).is_err());
        assert!(check(b"not a container", &Options::default()).is_err());
        assert!(check(&[], &Options::default()).is_err());
        Ok(())
    }

    #[test]
    fn classify_examples() {
        assert_eq!(classify("\\\\fileserver\\share"), vec![Kind::Path]);
        assert_eq!(classify("/home/someone/board"), vec![Kind::Path]);
        assert_eq!(classify("a@b.co"), vec![Kind::Email]);
        assert!(classify("WIDTH=10mil|LAYER=TOP").is_empty());
        assert!(classify("x@y").is_empty());
    }

    #[test]
    fn text_runs_handles_odd_lengths_and_empty() {
        assert!(text_runs(&[]).is_empty());
        assert!(text_runs(&[0x41]).is_empty());
        let runs = text_runs(b"\x00ABCDE\x00");
        assert!(runs.contains(&(1, "ABCDE".to_string())));
    }

    /// A TrueType file with one Windows `name` string, followed by binary that looks like a
    /// network path.
    fn tiny_font(name: &str) -> Vec<u8> {
        let text: Vec<u8> = name.encode_utf16().flat_map(u16::to_be_bytes).collect();
        let len = u16::try_from(text.len()).unwrap_or(0);
        let mut f = vec![0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        f.extend(b"name");
        f.extend([0; 4]);
        f.extend(28u32.to_be_bytes());
        f.extend(u32::from(len).saturating_add(18).to_be_bytes());
        f.extend([0, 0, 0, 1, 0, 18]);
        f.extend([0, 3, 0, 1, 4, 9, 0, 0]);
        f.extend(len.to_be_bytes());
        f.extend([0, 0]);
        f.extend(text);
        f.extend(b"\x01\x02\\\\ab\\\\cd\x03");
        f
    }

    /// `/EmbeddedFonts6/Data` holding `file`, packed.
    fn fonts_stream(file: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for s in ["Test Sans", "Test Sans", "Book"] {
            let bytes: Vec<u8> = s
                .encode_utf16()
                .chain([0])
                .flat_map(u16::to_le_bytes)
                .collect();
            out.extend(u32::try_from(bytes.len()).unwrap_or(0).to_le_bytes());
            out.extend(bytes);
        }
        let packed = miniz_oxide::deflate::compress_to_vec_zlib(file, 6);
        out.extend([0, 0, 1]);
        out.extend(u32::try_from(packed.len()).unwrap_or(0).to_le_bytes());
        out.extend(packed);
        out
    }

    fn fonts_report(file: &[u8], watch: &[&str]) -> Report {
        let options = Options {
            watch: watch.iter().map(|w| w.to_string()).collect(),
            ..Options::default()
        };
        let mut report = Report::default();
        check_fonts(FONTS_STREAM, &fonts_stream(file), &options, &mut report);
        report
    }

    #[test]
    fn an_embedded_font_is_checked_by_its_name_strings() {
        let clean = fonts_report(&tiny_font("Copyright Test Foundry"), &[]);
        assert!(clean.is_clean(), "{:?}", clean.findings);
        let leaky = fonts_report(&tiny_font("Made in C:\\Users\\someone"), &[]);
        let kinds: Vec<_> = leaky.findings.iter().map(|f| &f.kind).collect();
        assert_eq!(kinds, [&Kind::Path]);
        assert!(leaky
            .findings
            .iter()
            .all(|f| f.stream == "/EmbeddedFonts6/Data font 0 unpacked"));
    }

    #[test]
    fn watched_words_are_found_anywhere_in_an_embedded_font() {
        let mut file = tiny_font("Copyright Test Foundry");
        file.extend(b"\x00\x00hidden-word\x00");
        let report = fonts_report(&file, &["hidden-word"]);
        let kinds: Vec<_> = report.findings.iter().map(|f| &f.kind).collect();
        assert_eq!(kinds, [&Kind::Watched("hidden-word".into())]);
    }

    #[test]
    fn a_font_that_is_not_truetype_is_checked_byte_for_byte() {
        let report = fonts_report(b"not a font, C:\\Users\\someone", &[]);
        assert!(report.findings.iter().any(|f| f.kind == Kind::Path));
        let mut raw = Report::default();
        check_fonts(
            FONTS_STREAM,
            b"\x05\x00\x00\x00C:\\x\\y",
            &Options::default(),
            &mut raw,
        );
        assert!(raw.findings.iter().any(|f| f.stream == FONTS_STREAM));
    }
}
