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
//!
//! One value is unpacked too: `V9_STACKCUSTOMDATA`, the layer stack as zlib-compressed XML in
//! base64 (see `appearance.rs`), whose packing hides its text. Each such value in any stream, its
//! key written in single bytes or in UTF-16 right after a `|`, is unpacked and its XML checked
//! like a stream of its own, once more with XML character references (`&#xFC;`, `&amp;`)
//! decoded when it has any. A value that cannot be unpacked in full has its readable start
//! checked and is reported as [`Kind::Unreadable`]: the check cannot say what the rest holds.
//!
//! The fonts and the stack values of a file share one unpacking budget, the input limit.
//!
//! An Altium project file (`.PrjPcb`) is text, not a container: [`check_project`] checks it like
//! one stream, and also reports each link to a document that is absolute or leaves the project's
//! folder (`project.rs`) and each printer name Altium wrote into it (`TargetPrinter=`, which on
//! `param-project` was the PC's default printer).

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
    /// Packed data that could not be unpacked in full, so not all of its text was checked.
    Unreadable,
    /// A printer's name, which Altium writes into a project file.
    Printer,
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
    // What all packed data in the file may unpack to, together.
    let mut budget = MAX_INPUT_BYTES;
    // The streams together can never be bigger than the file, which is already bounded.
    for path in paths {
        let name = crate::container_path(&path);
        let mut data = Vec::new();
        container
            .open_stream(&path)
            .and_then(|s| s.take(MAX_INPUT_BYTES as u64).read_to_end(&mut data))
            .map_err(|e| ReadError::Invalid(format!("{name}: {e}")))?;
        if name == FONTS_STREAM {
            check_fonts(&name, &data, options, &mut budget, &mut report);
        } else {
            check_stream(&name, &data, options, &mut report);
            check_stacks(&name, &data, options, &mut budget, &mut report);
        }
    }
    Ok(report)
}

/// The name a project file's findings are reported under.
pub const PROJECT_STREAM: &str = "project file";

/// Checks an Altium project file (`.PrjPcb`): its text like a stream, plus every `DocumentPath=`
/// whose link is absolute or leaves the project's folder, and every `TargetPrinter=` with a
/// printer's name in it.
pub fn check_project(bytes: &[u8], options: &Options) -> Result<Report, ReadError> {
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
    let mut report = Report::default();
    check_stream(PROJECT_STREAM, bytes, options, &mut report);
    let mut offset = 0usize;
    for line in bytes.split(|&b| b == b'\n') {
        let text = String::from_utf8_lossy(line);
        let text = text.trim_end_matches('\r');
        if let Some(link) = text.strip_prefix("DocumentPath=") {
            if let Some(why) = crate::project::link_problem(link) {
                report.push(Finding {
                    stream: PROJECT_STREAM.to_string(),
                    offset,
                    kind: Kind::Path,
                    text: shorten(&format!("a document link that {why}: {link}")),
                });
            }
        } else if let Some(printer) = text.strip_prefix("TargetPrinter=") {
            if !printer.trim().is_empty() {
                report.push(Finding {
                    stream: PROJECT_STREAM.to_string(),
                    offset,
                    kind: Kind::Printer,
                    text: shorten(text),
                });
            }
        }
        offset = offset.saturating_add(line.len()).saturating_add(1);
    }
    Ok(report)
}

/// The stream that holds embedded font files, packed.
const FONTS_STREAM: &str = "/EmbeddedFonts6/Data";

/// The most bytes of `name` table strings read from one font file before the file is checked
/// byte for byte instead. A real font holds a few kilobytes of names; the records of a hostile
/// one could all point at the same long span.
const MAX_NAME_TABLE_TEXT: usize = 1 << 20;

/// Checks each embedded font's names and its unpacked file, or the raw stream when it cannot be
/// split into fonts or a font cannot be unpacked. The fonts are unpacked one at a time, each
/// checked and dropped before the next, from `budget`: what is left of the file's unpacking
/// budget.
fn check_fonts(
    stream: &str,
    data: &[u8],
    options: &Options,
    budget: &mut usize,
    report: &mut Report,
) {
    let Ok(records) = crate::fonts::records(data) else {
        check_stream(stream, data, options, report);
        return;
    };
    for (i, r) in records.iter().enumerate() {
        let file = match crate::fonts::unpack(r, *budget) {
            Ok(file) => file,
            Err(e) => {
                *budget = budget.saturating_sub(e.spent);
                check_stream(stream, data, options, report);
                return;
            }
        };
        *budget = budget.saturating_sub(file.len());
        check_stream(
            &format!("{stream} font {i} names"),
            r.names,
            options,
            report,
        );
        let name = format!("{stream} font {i} unpacked");
        match truetype_names(&file) {
            Some(texts) => {
                check_texts(&name, texts, options, report);
                check_watched(&name, &file, options, report);
            }
            None => check_stream(&name, &file, options, report),
        }
    }
}

/// The strings of a TrueType file's `name` table, as (offset in the file, text), or `None` when
/// the file is not a TrueType file with a readable `name` table, or when its strings add up to
/// more than [`MAX_NAME_TABLE_TEXT`] bytes. Windows and Unicode strings are UTF-16 with the high
/// byte first; Macintosh ones are read one byte per character.
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
    let mut text_bytes = 0usize;
    for i in 0..count {
        let rec = name_at.checked_add(6)?.checked_add(i.checked_mul(12)?)?;
        let platform = u16_at(rec)?;
        let len = u16_at(rec.checked_add(8)?)?;
        let start = strings_at.checked_add(u16_at(rec.checked_add(10)?)?)?;
        let bytes = file.get(start..start.checked_add(len)?)?;
        text_bytes = text_bytes.checked_add(len)?;
        if text_bytes > MAX_NAME_TABLE_TEXT {
            return None;
        }
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

/// The start offsets of `needle` in `haystack`, at most one more than the report lists (so a
/// report that is full still counts more). An empty needle matches nothing.
fn find_all(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() {
        return Vec::new();
    }
    haystack
        .windows(needle.len())
        .enumerate()
        .filter(|(_, w)| *w == needle)
        .map(|(i, _)| i)
        .take(MAX_FINDINGS.saturating_add(1))
        .collect()
}

/// The key of the packed layer stack, with the `|` that starts every key in a record.
const STACK_KEY: &str = "|V9_STACKCUSTOMDATA=";

/// Unpacks each `V9_STACKCUSTOMDATA` value in the stream and checks its XML as
/// "`<stream>` V9_STACKCUSTOMDATA `<n>`" (offsets in the XML). The key is found in single bytes
/// and in UTF-16; a value ends at `|`, at a 0 or at the end of the stream. Each value takes what
/// it unpacked to from `budget`, what is left of the file's unpacking budget.
fn check_stacks(
    stream: &str,
    data: &[u8],
    options: &Options,
    budget: &mut usize,
    report: &mut Report,
) {
    let mut values = Vec::new();
    for at in find_all(data, STACK_KEY.as_bytes()) {
        let start = at.saturating_add(STACK_KEY.len());
        let rest = data.get(start..).unwrap_or_default();
        let len = rest
            .iter()
            .position(|&b| b == b'|' || b == 0)
            .unwrap_or(rest.len());
        values.push((start, rest.get(..len).unwrap_or_default().to_vec()));
    }
    let wide_key: Vec<u8> = STACK_KEY
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    for at in find_all(data, &wide_key) {
        let start = at.saturating_add(wide_key.len());
        let units: Vec<u16> = data
            .get(start..)
            .unwrap_or_default()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| u16::from_le_bytes(pair))
            .take_while(|&u| u != 0 && u != u16::from(b'|'))
            .collect();
        values.push((start, String::from_utf16_lossy(&units).into_bytes()));
    }
    for (n, (offset, value)) in values.iter().enumerate() {
        let name = format!("{stream} V9_STACKCUSTOMDATA {n}");
        match crate::appearance::unpack_stack(value, *budget) {
            Ok(xml) => {
                *budget = budget.saturating_sub(xml.len());
                check_xml(&name, &xml, options, report);
            }
            Err(partial) => {
                *budget = budget.saturating_sub(partial.bytes.len());
                check_xml(&name, &partial.bytes, options, report);
                report.push(Finding {
                    stream: stream.to_string(),
                    offset: *offset,
                    kind: Kind::Unreadable,
                    text: format!(
                        "V9_STACKCUSTOMDATA {n} could not be unpacked in full ({}); only its first \
                         {} bytes were checked",
                        partial.why,
                        partial.bytes.len()
                    ),
                });
            }
        }
    }
}

/// Checks XML like a stream, and once more with its character references decoded when it has
/// any, as "`<name>` decoded" (offsets in the decoded text).
fn check_xml(name: &str, xml: &[u8], options: &Options, report: &mut Report) {
    check_stream(name, xml, options, report);
    if let Some(decoded) = decode_references(&String::from_utf8_lossy(xml)) {
        check_stream(
            &format!("{name} decoded"),
            decoded.as_bytes(),
            options,
            report,
        );
    }
}

/// The text with XML's character references (`&#252;`, `&#xFC;`) and its five named ones
/// (`&amp;` and so on) replaced by their characters, or `None` when it holds none. A reference
/// that is not well formed is kept as written.
fn decode_references(text: &str) -> Option<String> {
    if !text.contains('&') {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        let (before, from_amp) = rest.split_at(amp);
        out.push_str(before);
        let decoded = from_amp
            .get(1..)
            .and_then(|r| r.split_once(';'))
            .filter(|(body, _)| body.len() <= 10)
            .and_then(|(body, after)| reference(body).map(|c| (c, after)));
        match decoded {
            Some((c, after)) => {
                out.push(c);
                rest = after;
                changed = true;
            }
            None => {
                out.push('&');
                rest = from_amp.get(1..).unwrap_or_default();
            }
        }
    }
    out.push_str(rest);
    changed.then_some(out)
}

/// The character a reference body (between `&` and `;`) stands for.
fn reference(body: &str) -> Option<char> {
    let named = match body {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    let number = body.strip_prefix('#')?;
    let code = match number.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => number.parse().ok()?,
    };
    char::from_u32(code)
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
        fonts_stream_named("Test Sans", &[file])
    }

    /// `/EmbeddedFonts6/Data` holding each of `files`, packed, under the full name `full`.
    fn fonts_stream_named(full: &str, files: &[&[u8]]) -> Vec<u8> {
        let mut out = Vec::new();
        for file in files {
            for s in [full, "Test Sans", "Book"] {
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
        }
        out
    }

    fn fonts_report(file: &[u8], watch: &[&str]) -> Report {
        let options = Options {
            watch: watch.iter().map(|w| w.to_string()).collect(),
            ..Options::default()
        };
        let mut report = Report::default();
        let mut budget = MAX_INPUT_BYTES;
        check_fonts(
            FONTS_STREAM,
            &fonts_stream(file),
            &options,
            &mut budget,
            &mut report,
        );
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
        let mut budget = MAX_INPUT_BYTES;
        check_fonts(
            FONTS_STREAM,
            b"\x05\x00\x00\x00C:\\x\\y",
            &Options::default(),
            &mut budget,
            &mut raw,
        );
        assert!(raw.findings.iter().any(|f| f.stream == FONTS_STREAM));
    }

    #[test]
    fn many_fonts_share_one_unpacking_budget() {
        let big = vec![0u8; 1_000];
        let data = fonts_stream_named(r"C:\Users\someone", &[&big, &big, &big]);
        let streams = |budget: usize| {
            let mut report = Report::default();
            let mut budget = budget;
            check_fonts(
                FONTS_STREAM,
                &data,
                &Options::default(),
                &mut budget,
                &mut report,
            );
            let mut s: Vec<String> = report.findings.into_iter().map(|f| f.stream).collect();
            s.dedup();
            s
        };
        // Room for all three: each font's names are checked on their own.
        assert_eq!(
            streams(3_000),
            [0, 1, 2].map(|i| format!("{FONTS_STREAM} font {i} names"))
        );
        // Room for two: the third does not fit in what is left, so the whole stream is checked
        // byte for byte as well, which still finds the path in the names.
        assert_eq!(
            streams(2_500).last().map(String::as_str),
            Some(FONTS_STREAM)
        );
    }

    #[test]
    fn a_name_table_whose_records_all_point_at_one_long_span_is_checked_byte_for_byte() {
        // 65,535 records, each pointing at the same 60,000 bytes (the records themselves, as
        // the string storage may start anywhere): about 3.9 GB of text if read.
        let count: u16 = u16::MAX;
        let span: u16 = 60_000;
        let mut f = vec![0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        f.extend(b"name");
        f.extend([0; 4]);
        f.extend(28u32.to_be_bytes());
        f.extend([0; 4]);
        f.extend([0, 0]);
        f.extend(count.to_be_bytes());
        f.extend(6u16.to_be_bytes());
        for _ in 0..count {
            f.extend([0, 1, 0, 0, 0, 0, 0, 0]);
            f.extend(span.to_be_bytes());
            f.extend([0, 0]);
        }
        f.extend(b"\0C:\\Users\\x\0");
        assert_eq!(truetype_names(&f), None);
        let report = fonts_report(&f, &[]);
        let found: Vec<_> = report
            .findings
            .iter()
            .map(|r| (r.stream.as_str(), &r.kind))
            .collect();
        assert_eq!(
            found,
            [("/EmbeddedFonts6/Data font 0 unpacked", &Kind::Path)]
        );
    }

    /// Standard base64 with `=` padding.
    fn base64(data: &[u8]) -> String {
        const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let b = [0, 1, 2].map(|i| chunk.get(i).copied().unwrap_or(0));
            let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
            for i in 0..4 {
                if i <= chunk.len() {
                    let six = usize::try_from((n >> (18 - 6 * i)) & 63).unwrap_or(0);
                    out.push(char::from(ABC.get(six).copied().unwrap_or(b'A')));
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    fn packed(xml: &[u8]) -> String {
        base64(&miniz_oxide::deflate::compress_to_vec_zlib(xml, 6))
    }

    /// A `/Board6/Data`-like record holding each value after the stack key.
    fn record(values: &[String]) -> Vec<u8> {
        let mut out = b"|RECORD=Board".to_vec();
        for v in values {
            out.extend(b"|V9_STACKCUSTOMDATA=");
            out.extend(v.bytes());
        }
        out.extend(b"|\0");
        out
    }

    fn stacks_report(data: &[u8], watch: &[&str], budget: usize) -> Report {
        let options = Options {
            watch: watch.iter().map(|w| w.to_string()).collect(),
            ..Options::default()
        };
        let mut report = Report::default();
        let mut budget = budget;
        check_stream("/Board6/Data", data, &options, &mut report);
        check_stacks("/Board6/Data", data, &options, &mut budget, &mut report);
        report
    }

    fn found(report: &Report) -> Vec<(&str, &Kind)> {
        report
            .findings
            .iter()
            .map(|f| (f.stream.as_str(), &f.kind))
            .collect()
    }

    #[test]
    fn the_packed_layer_stack_is_unpacked_and_checked() {
        let clean = record(&[packed(b"<Stack><Layer Name=\"Top\"/></Stack>")]);
        let report = stacks_report(&clean, &[], MAX_INPUT_BYTES);
        assert!(report.is_clean(), "{:?}", report.findings);

        let leaky = record(&[
            packed(b"<Stack>"),
            packed(br"<Stack><Property Name='File'>C:\Users\someone\stack.xml</Property></Stack>"),
        ]);
        let report = stacks_report(&leaky, &["someone"], MAX_INPUT_BYTES);
        assert_eq!(
            found(&report),
            [
                ("/Board6/Data V9_STACKCUSTOMDATA 1", &Kind::Path),
                (
                    "/Board6/Data V9_STACKCUSTOMDATA 1",
                    &Kind::Watched("someone".into())
                ),
            ]
        );
    }

    #[test]
    fn a_value_that_cannot_be_unpacked_is_reported() {
        for value in ["not base64!", "AAAA", ""] {
            let report = stacks_report(&record(&[value.to_string()]), &[], MAX_INPUT_BYTES);
            assert_eq!(
                found(&report),
                [("/Board6/Data", &Kind::Unreadable)],
                "{value:?}"
            );
            assert!(!report.is_clean());
        }
    }

    #[test]
    fn a_truncated_stack_still_has_its_readable_start_checked() {
        let mut xml =
            br"<Stack><Property Name='File'>C:\Users\someone\stack.xml</Property>".to_vec();
        // Text that does not repeat, so the packed stream stays long and its end can be cut.
        for i in 0..2_000u32 {
            xml.extend(format!("<Layer Id='{}'/>", i.wrapping_mul(2_654_435_761)).bytes());
        }
        let whole = miniz_oxide::deflate::compress_to_vec_zlib(&xml, 6);
        let cut = whole.get(..whole.len() / 2).unwrap_or_default();
        let report = stacks_report(&record(&[base64(cut)]), &[], MAX_INPUT_BYTES);
        assert_eq!(
            found(&report),
            [
                ("/Board6/Data V9_STACKCUSTOMDATA 0", &Kind::Path),
                ("/Board6/Data", &Kind::Unreadable),
            ]
        );
        let note = &report
            .findings
            .last()
            .map(|f| f.text.clone())
            .unwrap_or_default();
        assert!(
            note.contains("could not be unpacked in full (not whole zlib data"),
            "{note}"
        );
    }

    #[test]
    fn a_stack_over_the_per_value_limit_is_checked_up_to_it() {
        let limit = crate::appearance::MAX_STACK_XML;
        let mut xml = br"<Stack>C:\Users\someone</Stack>".to_vec();
        xml.resize(limit.saturating_add(1), b' ');
        let report = stacks_report(&record(&[packed(&xml)]), &[], MAX_INPUT_BYTES);
        assert_eq!(
            found(&report),
            [
                ("/Board6/Data V9_STACKCUSTOMDATA 0", &Kind::Path),
                ("/Board6/Data", &Kind::Unreadable),
            ]
        );
        let note = report
            .findings
            .last()
            .map(|f| f.text.as_str())
            .unwrap_or_default();
        assert!(
            note.contains(&format!("it unpacks to more than {limit} bytes")),
            "{note}"
        );
    }

    #[test]
    fn a_stack_key_in_utf16_is_unpacked_too() {
        let text = format!(
            "|RECORD=Board|V9_STACKCUSTOMDATA={}|",
            packed(br"<Stack>\\server\share</Stack>")
        );
        let mut data: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        data.extend([0, 0]);
        let report = stacks_report(&data, &[], MAX_INPUT_BYTES);
        assert_eq!(
            found(&report),
            [("/Board6/Data V9_STACKCUSTOMDATA 0", &Kind::Path)]
        );
    }

    #[test]
    fn the_key_counts_only_after_a_bar() {
        let value = packed(br"<Stack>C:\Users\someone</Stack>");
        let data = format!("|RECORD=Board|XV9_STACKCUSTOMDATA={value}|");
        let report = stacks_report(data.as_bytes(), &[], MAX_INPUT_BYTES);
        assert!(report.is_clean(), "{:?}", report.findings);
    }

    #[test]
    fn character_references_are_decoded_before_the_check() {
        let xml = b"<Stack Owner='C:&#x5C;Users&#92;x' City='Z&#xFC;rich &amp; Co'/>";
        let report = stacks_report(&record(&[packed(xml)]), &["Zürich"], MAX_INPUT_BYTES);
        assert_eq!(
            found(&report),
            [
                ("/Board6/Data V9_STACKCUSTOMDATA 0 decoded", &Kind::Path),
                (
                    "/Board6/Data V9_STACKCUSTOMDATA 0 decoded",
                    &Kind::Watched("Zürich".into())
                ),
            ]
        );
        assert_eq!(
            decode_references("a &lt;b&gt; &#65;&#x42; &bogus; & &#xZZ;"),
            Some("a <b> AB &bogus; & &#xZZ;".to_string())
        );
        assert_eq!(decode_references("no references"), None);
        assert_eq!(decode_references("& alone"), None);
    }

    #[test]
    fn fonts_and_stacks_share_one_budget() {
        let font = vec![0u8; 1_000];
        let stack = vec![b' '; 1_000];
        let mut budget = 1_500;
        let mut report = Report::default();
        let options = Options::default();
        check_fonts(
            FONTS_STREAM,
            &fonts_stream(&font),
            &options,
            &mut budget,
            &mut report,
        );
        assert_eq!(budget, 500);
        check_stacks(
            "/Board6/Data",
            &record(&[packed(&stack)]),
            &options,
            &mut budget,
            &mut report,
        );
        assert_eq!(budget, 0);
        assert_eq!(found(&report), [("/Board6/Data", &Kind::Unreadable)]);
    }

    #[test]
    fn check_reads_the_stack_of_a_real_board() -> Result<(), Box<dyn std::error::Error>> {
        let board = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/boards/one-enig-resistor.PcbDoc");
        let options = Options {
            // A word only the unpacked stack holds: its finish material, "Nickel, Gold".
            watch: vec!["Nickel".into()],
            ..Options::default()
        };
        let report = check(&std::fs::read(board)?, &options)?;
        let streams: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.kind == Kind::Watched("Nickel".into()))
            .map(|f| f.stream.as_str())
            .collect();
        assert!(!streams.is_empty());
        assert!(
            streams
                .iter()
                .all(|s| s.starts_with("/Board6/Data V9_STACKCUSTOMDATA ")),
            "{streams:?}"
        );
        Ok(())
    }

    #[test]
    fn a_project_file_is_checked_for_links_and_printers() -> Result<(), ReadError> {
        let project = "\u{feff}[Design]\r\nVersion=1.0\r\n[Document1]\r\nDocumentPath=a.PcbDoc\r\n\
            [Document2]\r\nDocumentPath=..\\up.PcbDoc\r\n[OutputGroup1]\r\nTargetPrinter=\r\n\
            [OutputGroup2]\r\nTargetPrinter=Office Printer 3\r\n";
        let report = check_project(project.as_bytes(), &Options::default())?;
        let found: Vec<_> = report
            .findings
            .iter()
            .map(|f| (&f.kind, f.text.as_str()))
            .collect();
        assert_eq!(
            found,
            [
                (
                    &Kind::Path,
                    "a document link that leaves the project's folder: ..\\up.PcbDoc"
                ),
                (&Kind::Printer, "TargetPrinter=Office Printer 3"),
            ]
        );
        let clean = "[Design]\r\n[Document1]\r\nDocumentPath=a.PcbDoc\r\nTargetPrinter=\r\n";
        assert!(check_project(clean.as_bytes(), &Options::default())?.is_clean());
        let options = Options {
            watch: vec!["secretco".into()],
            ..Options::default()
        };
        let watched = check_project(b"[Design]\r\nOwner=SecretCo\r\n", &options)?;
        assert_eq!(watched.findings.len(), 1);
        Ok(())
    }

    #[test]
    fn the_committed_test_project_is_clean() -> Result<(), Box<dyn std::error::Error>> {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/boards/param-project/param-project.PrjPcb");
        let report = check_project(&std::fs::read(file)?, &Options::default())?;
        assert!(report.is_clean(), "{:?}", report.findings);
        Ok(())
    }
}
