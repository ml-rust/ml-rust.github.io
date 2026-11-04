//! The tags a result row prints. No filter reads them.

use catalogue_core::{EntryKind, EntryReport};

use crate::pages::{ChoiceText, row_statuses};

/// The tags of a result row for an entry of `kind` with `record`: the short facts, the full wording of a shortened fact,
/// and the state words.
///
/// A row reads `row_<fact>` and `row_<fact>_full`. A fact with no tag reads as not measured.
pub(super) fn row_tags(kind: EntryKind, record: Option<&EntryReport>) -> Vec<(String, String)> {
    let choice = ChoiceText::of(kind, record);
    let mut tags = Vec::new();
    for (name, fact) in [
        ("row_downloads", Some(&choice.downloads_cell)),
        ("row_dependents", Some(&choice.dependents_cell)),
        ("row_released", Some(&choice.latest_release)),
        ("row_gpu", Some(&choice.runs_on)),
        ("row_native", Some(&choice.native_code)),
        ("row_python", choice.python.as_ref()),
        ("row_license", Some(&choice.license)),
    ] {
        let Some(fact) = fact else { continue };
        tags.push((name.to_owned(), fact.short.clone()));
        if let Some(full) = &fact.full {
            tags.push((format!("{name}_full"), full.clone()));
        }
    }
    if let Some(version) = choice.version {
        tags.push(("row_version".to_owned(), version));
    }
    if let Some(iso) = choice.released {
        tags.push(("row_released_iso".to_owned(), iso));
    }
    for status in record.map(row_statuses).unwrap_or_default() {
        tags.push(("row_status".to_owned(), status.word.to_owned()));
        tags.push(("row_status_meaning".to_owned(), status.meaning.to_owned()));
    }
    tags
}
