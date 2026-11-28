//! Applies assembly choices before model resolution and text drawing.

use std::collections::{BTreeMap, BTreeSet};

use board_model::{Board, LayerKind, Note, Severity};

pub(crate) fn apply(board: &mut Board, choice: Option<&str>) -> Result<Vec<Note>, String> {
    let names: Vec<_> = board.variants.iter().map(|v| v.name.as_str()).collect();
    let variant = choice
        .map(|name| {
            board
                .variants
                .iter()
                .find(|v| v.name == name)
                .ok_or_else(|| {
                    let available = if names.is_empty() {
                        "the board has none".to_string()
                    } else {
                        format!("available variants: {}", names.join(", "))
                    };
                    format!("unknown variant {name:?}; {available}")
                })
        })
        .transpose()?;
    let mut notes = Vec::new();
    let mut say = |severity, message| notes.push(Note { severity, message });
    if choice.is_none() && !names.is_empty() {
        say(
            Severity::Info,
            format!(
                "Available variants: {}; the base design was converted",
                names.join(", ")
            ),
        );
    }
    if variant.is_none() && board.components.iter().all(|p| p.fitted) {
        return Ok(notes);
    }
    let not_fitted: BTreeSet<_> = variant.into_iter().flat_map(|v| &v.not_fitted).collect();
    let fitted: BTreeSet<_> = variant.into_iter().flat_map(|v| &v.fitted).collect();
    let conflicts: Vec<_> = not_fitted
        .intersection(&fitted)
        .map(|s| s.as_str())
        .collect();
    if !conflicts.is_empty() {
        say(
            Severity::Warning,
            format!(
                "Variant {} lists parts as both fitted and not fitted; not fitted wins: {}",
                choice.unwrap_or_default(),
                conflicts.join(", ")
            ),
        );
    }
    let values: BTreeMap<_, _> = variant
        .into_iter()
        .flat_map(|v| &v.values)
        .map(|v| (&v.designator, &v.comment))
        .collect();
    let mut omitted = Vec::new();
    let mut stale = BTreeSet::new();
    let mut mapping = Vec::with_capacity(board.components.len());
    let mut next = 0usize;
    for (index, part) in board.components.iter_mut().enumerate() {
        let absent = not_fitted.contains(&part.designator)
            || (!part.fitted && !fitted.contains(&part.designator));
        if absent {
            omitted.push(part.designator.clone());
            mapping.push(None);
            continue;
        }
        mapping.push(Some(next));
        next = next.saturating_add(1);
        if let Some(value) = values
            .get(&part.designator)
            .filter(|v| **v != &part.comment)
        {
            // We cannot reconstruct references already expanded by a reader. Literal base
            // values on owned silk are retained and reported; unresolved references follow
            // the new comment when the package resolves them later.
            if board.texts.iter().any(|t| {
                t.component == Some(index)
                    && board
                        .layers
                        .get(t.layer)
                        .is_some_and(|l| l.kind == LayerKind::Silkscreen)
                    && !part.comment.is_empty()
                    && t.content.contains(&part.comment)
            }) {
                stale.insert(part.designator.clone());
            }
            part.comment.clone_from(value);
        }
    }
    board.texts.retain_mut(|text| {
        let Some(old) = text.component else {
            return true;
        };
        let new = mapping.get(old).copied().flatten();
        if new.is_none()
            && board
                .layers
                .get(text.layer)
                .is_some_and(|l| l.kind == LayerKind::Silkscreen)
        {
            return false;
        }
        text.component = new;
        true
    });
    let mut indices = mapping.into_iter();
    board
        .components
        .retain(|_| indices.next().flatten().is_some());
    if !omitted.is_empty() {
        let context = choice.map_or_else(
            || "(do not populate)".to_string(),
            |v| format!("in variant {v:?}"),
        );
        say(
            Severity::Info,
            format!("Not fitted {context}: {}", omitted.join(", ")),
        );
    }
    if !stale.is_empty() {
        say(
            Severity::Info,
            format!(
                "Drawn silkscreen text keeps the base value for: {}",
                stale.into_iter().collect::<Vec<_>>().join(", ")
            ),
        );
    }
    Ok(notes)
}

#[cfg(all(test, feature = "altium"))]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use board_model::{InputFormat, Variant};

    fn board() -> Board {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../altium-pcbdoc/tests/boards/variants-project");
        let mut board = altium_pcbdoc::Altium
            .read(&std::fs::read(path.join("variants-board.PcbDoc")).unwrap())
            .unwrap()
            .board;
        let project = altium_pcbdoc::project::read(
            &std::fs::read(path.join("variants-project.PrjPcb")).unwrap(),
        )
        .unwrap();
        altium_pcbdoc::project::add_variants(&mut board, &project);
        // The test board shows designators; add a comment reference to exercise drawing
        // with the same placement and the documented R1 value.
        let r1 = board
            .components
            .iter()
            .position(|p| p.designator == "R1")
            .unwrap();
        let mut comment = board
            .texts
            .iter()
            .find(|t| t.component == Some(r1))
            .unwrap()
            .clone();
        comment.content = ".Comment".into();
        board.texts.push(comment);
        board
    }

    #[test]
    fn alt_changes_the_comment_before_special_strings_are_drawn() {
        let mut b = board();
        apply(&mut b, Some("Alt")).unwrap();
        assert_eq!(
            b.components
                .iter()
                .find(|p| p.designator == "R1")
                .unwrap()
                .comment,
            "22k"
        );
        altium_pcbdoc::special_strings::resolve_texts(&mut b, &[]);
        assert!(b.texts.iter().any(|t| t.content == "22k"));
    }

    #[test]
    fn omitted_silk_is_removed_and_remaining_owners_are_remapped() {
        let mut b = board();
        // Put R1 first to exercise remapping when the first component is omitted.
        if b.components.first().unwrap().designator != "R1" {
            b.components.swap(0, 1);
            for text in &mut b.texts {
                text.component = text.component.map(|i| if i == 0 { 1 } else { 0 });
            }
        }
        b.variants.push(Variant {
            name: "First out".into(),
            not_fitted: vec!["R1".into()],
            fitted: vec!["R1".into()],
            ..Variant::default()
        });
        let notes = apply(&mut b, Some("First out")).unwrap();
        assert!(notes
            .iter()
            .any(|n| n.severity == Severity::Warning && n.message.contains("not fitted wins: R1")));
        assert_eq!(b.components.len(), 1);
        assert_eq!(b.components[0].designator, "R2");
        altium_pcbdoc::special_strings::resolve_texts(&mut b, &[]);
        assert!(b
            .texts
            .iter()
            .any(|t| t.content == "R2" && t.component == Some(0)));
        assert!(!b
            .texts
            .iter()
            .any(|t| t.content == "R1" || t.content == "10k"));
    }

    #[test]
    fn already_expanded_silk_keeps_its_value_with_one_note() {
        let mut b = board();
        altium_pcbdoc::special_strings::resolve_texts(&mut b, &[]);
        let notes = apply(&mut b, Some("Alt")).unwrap();
        assert!(b.texts.iter().any(|t| t.content == "10k"));
        assert_eq!(
            notes
                .iter()
                .filter(|n| n.message == "Drawn silkscreen text keeps the base value for: R1")
                .count(),
            1
        );
    }

    #[test]
    fn no_variants_gives_a_clear_error_and_base_is_unchanged() {
        let mut b = board();
        b.variants.clear();
        let original = b.clone();
        assert!(apply(&mut b, None).unwrap().is_empty());
        assert_eq!(b, original);
        assert_eq!(
            apply(&mut b, Some("Lite")).unwrap_err(),
            "unknown variant \"Lite\"; the board has none"
        );
    }
}
