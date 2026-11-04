//! The description of a family draft.

use catalogue_core::description_text;

use crate::dump::DumpSnapshot;

/// The first usable description among `members` and the name of its crate.
///
/// Errors with the primary's reason when no member has one.
pub(super) fn description_of<'a>(
    snapshot: &'a DumpSnapshot,
    members: &[(u32, &str)],
) -> Result<(String, &'a str), String> {
    let mut first_error = None;
    for (id, _) in members {
        let Some(row) = snapshot.crates.get(id) else {
            continue;
        };
        match usable_text(row.description.as_deref()) {
            Ok(text) => return Ok((text, row.name.as_str())),
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    Err(first_error.unwrap_or_else(|| "no description".to_owned()))
}

/// The whole description, or its leading whole sentences when the whole text is too long.
fn usable_text(raw: Option<&str>) -> Result<String, String> {
    description_text(raw).or_else(|error| leading_sentences(raw.unwrap_or_default()).ok_or(error))
}

/// The longest run of leading sentences that is a valid description.
///
/// A sentence ends at a period before a space and a capital letter. A text without such a
/// boundary yields `None`, so a long text never ends mid-sentence.
fn leading_sentences(raw: &str) -> Option<String> {
    let text = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let ends = text.match_indices(". ").filter_map(|(at, _)| {
        let starts_sentence = text
            .get(at + 2..)?
            .chars()
            .next()
            .is_some_and(char::is_uppercase);
        starts_sentence.then_some(at + 1)
    });
    ends.filter_map(|end| description_text(text.get(..end)).ok())
        .last()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_description_keeps_its_leading_sentences() {
        let raw = format!(
            "Reads the format.\nIt aims to be safe. {}. Last part.",
            "Word ".repeat(70).trim_end()
        );
        assert_eq!(
            usable_text(Some(&raw)).as_deref(),
            Ok("Reads the format. It aims to be safe.")
        );
    }

    #[test]
    fn a_description_that_fits_stays_whole() {
        assert_eq!(usable_text(Some("One. Two.")).as_deref(), Ok("One. Two."));
    }

    #[test]
    fn a_long_text_without_a_sentence_end_keeps_its_error() {
        let found = usable_text(Some(&"x".repeat(301)));
        assert_eq!(
            found,
            Err("description is longer than 300 characters".to_owned())
        );
    }

    #[test]
    fn an_abbreviation_does_not_end_a_sentence() {
        let raw = format!("Runs e.g. on a GPU {}", "word ".repeat(70));
        assert_eq!(leading_sentences(&raw), None);
    }

    #[test]
    fn a_missing_description_stays_missing() {
        assert_eq!(usable_text(None), Err("no description".to_owned()));
    }
}
