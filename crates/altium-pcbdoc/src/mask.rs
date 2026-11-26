//! The solder mask expansion: how far the mask opens past the edge of each bare pad.
//!
//! Worked out from our own CC0 test boards (see LEGAL.md): `/Rules6/Data` (records as in
//! `planes.rs`) holds one `RULEKIND=SolderMaskExpansion` record per board, with
//! `SCOPE1EXPRESSION=All`, `SCOPE2EXPRESSION=All`, `ENABLED=TRUE`, `PRIORITY=1` and the expansion
//! as `EXPANSION`, a length in mil. The `mask-expansion-*` boards give it as `0mil`, `1.9685mil`,
//! `5.9055mil` and `7.874mil` (0, 0.05, 0.15 and 0.2 mm); the others as `3.937mil` (0.1 mm),
//! except `one-linked-step-resistor`, which keeps `0mil`.
//!
//! Only the board-wide rule is read: a rule for some objects only, and an override on a single
//! pad, are not, and the board-wide value is used for every pad.
use board_model::{Note, Severity};

use crate::planes::rule_records;

/// Reads the board-wide `SolderMaskExpansion` rule: the enabled one for every object with the
/// best (lowest) priority. `None` when the file has no such rule. A negative expansion (Altium
/// allows one, to open the mask less than the pad) is read as 0, with a note.
pub(crate) fn expansion(data: Option<&[u8]>, notes: &mut Vec<Note>) -> Result<Option<f64>, String> {
    let Some(data) = data else {
        return Ok(None);
    };
    let mut best: Option<(i64, f64)> = None;
    let mut narrower = 0usize;
    for rule in rule_records(data)? {
        if rule.get("RULEKIND").map(str::trim) != Some("SolderMaskExpansion")
            || rule.get("ENABLED").map(str::trim) == Some("FALSE")
        {
            continue;
        }
        let all = |key: &str| rule.get(key).is_none_or(|v| v.trim() == "All");
        if !(all("SCOPE1EXPRESSION") && all("SCOPE2EXPRESSION")) {
            narrower += 1;
            continue;
        }
        let Some(value) = rule.length_mm("EXPANSION")? else {
            continue;
        };
        let priority = rule
            .get("PRIORITY")
            .and_then(|p| p.trim().parse().ok())
            .unwrap_or(i64::MAX);
        if best.is_none_or(|(p, _)| priority < p) {
            best = Some((priority, value));
        }
    }
    if narrower > 0 {
        notes.push(Note {
            severity: Severity::Info,
            message: format!(
                "{narrower} solder mask expansion rule(s) for some objects only are not read; \
                 the board-wide rule is used for every pad"
            ),
        });
    }
    Ok(best.map(|(_, value)| {
        if value < 0.0 {
            notes.push(Note {
                severity: Severity::Info,
                message: format!(
                    "the solder mask expansion is negative ({value:.4} mm); the mask is opened \
                     at the pad's edge instead"
                ),
            });
            0.0
        } else {
            value
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One `/Rules6/Data` record: a `u16` kind, a `u32` length, then the text.
    fn record(text: &str) -> Vec<u8> {
        let mut out = vec![0, 0];
        let len = u32::try_from(text.len()).unwrap_or(u32::MAX);
        out.extend(len.to_le_bytes());
        out.extend(text.as_bytes());
        out
    }

    fn mask_rule(scope: &str, priority: u32, enabled: bool, expansion: &str) -> Vec<u8> {
        record(&format!(
            "|RULEKIND=SolderMaskExpansion|NAME=Mask|ENABLED={}|PRIORITY={priority}\
             |SCOPE1EXPRESSION={scope}|SCOPE2EXPRESSION=All|EXPANSION={expansion}|",
            if enabled { "TRUE" } else { "FALSE" }
        ))
    }

    fn near(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-9)
    }

    #[test]
    fn reads_the_board_wide_rule() -> Result<(), String> {
        let mut data = record("|RULEKIND=Clearance|NAME=Clearance|GAP=10mil|");
        data.extend(mask_rule("All", 1, true, "3.937mil"));
        let mut notes = Vec::new();
        assert!(near(expansion(Some(&data), &mut notes)?, 3.937 * 0.0254));
        assert!(notes.is_empty(), "{notes:?}");
        Ok(())
    }

    #[test]
    fn no_rule_or_no_stream_gives_none() -> Result<(), String> {
        let mut notes = Vec::new();
        assert_eq!(expansion(None, &mut notes)?, None);
        let data = record("|RULEKIND=Clearance|NAME=Clearance|GAP=10mil|");
        assert_eq!(expansion(Some(&data), &mut notes)?, None);
        assert!(notes.is_empty(), "{notes:?}");
        Ok(())
    }

    #[test]
    fn the_best_priority_wins_and_disabled_and_narrower_rules_are_skipped() -> Result<(), String> {
        let mut data = mask_rule("All", 2, true, "4mil");
        data.extend(mask_rule("All", 1, false, "8mil"));
        data.extend(mask_rule("InNet('GND')", 1, true, "6mil"));
        data.extend(mask_rule("All", 1, true, "2mil"));
        let mut notes = Vec::new();
        assert!(near(expansion(Some(&data), &mut notes)?, 2.0 * 0.0254));
        let [note] = notes.as_slice() else {
            return Err(format!("{notes:?}"));
        };
        assert!(note
            .message
            .starts_with("1 solder mask expansion rule(s) for some objects"));
        Ok(())
    }

    #[test]
    fn a_negative_expansion_is_read_as_zero_with_a_note() -> Result<(), String> {
        let data = mask_rule("All", 1, true, "-2mil");
        let mut notes = Vec::new();
        assert!(near(expansion(Some(&data), &mut notes)?, 0.0));
        assert_eq!(notes.len(), 1, "{notes:?}");
        Ok(())
    }
}
