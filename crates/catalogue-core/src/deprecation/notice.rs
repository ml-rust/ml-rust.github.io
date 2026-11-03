//! The notice type and the combined scan.

use serde::{Deserialize, Serialize};

use super::{description, readme};

/// Longest evidence text kept, in characters.
pub const EVIDENCE_MAX_CHARS: usize = 120;

/// The crate field that holds a deprecation notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Field {
    /// The crates.io `description` field.
    Description,
    /// The README text published with the crate.
    Readme,
}

/// A deprecation notice and the text that carries it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeprecationNotice {
    /// The field that holds the notice.
    pub field: Field,
    /// The matched clause, heading or line: trimmed, at most 120 characters.
    pub phrase: String,
}

/// Returns the first deprecation notice in `description`, then in `readme`.
///
/// `readme` can hold the whole README. The scan reads its first 1000 characters.
pub fn scan(description: Option<&str>, readme: Option<&str>) -> Option<DeprecationNotice> {
    if let Some(found) = description.and_then(description::find) {
        return Some(DeprecationNotice {
            field: Field::Description,
            phrase: evidence(found),
        });
    }
    readme
        .and_then(readme::find)
        .map(|found| DeprecationNotice {
            field: Field::Readme,
            phrase: evidence(found),
        })
}

fn evidence(text: &str) -> String {
    text.trim().chars().take(EVIDENCE_MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn description(text: &str) -> Option<String> {
        scan(Some(text), None).map(|notice| {
            assert_eq!(notice.field, Field::Description);
            notice.phrase
        })
    }

    #[test]
    fn evidence_is_trimmed_and_capped() {
        let long = format!("Deprecated in favor of {}", "x".repeat(300));
        let found = description(&long).expect("matches");
        assert_eq!(found.chars().count(), EVIDENCE_MAX_CHARS);
        assert!(found.starts_with("Deprecated in favor of"));
    }

    #[test]
    fn description_wins_over_readme() {
        let found = scan(Some("Deprecated"), Some("# foo (deprecated)")).expect("found");
        assert_eq!(found.field, Field::Description);
    }

    #[test]
    fn nothing_found_in_clean_text() {
        assert_eq!(
            scan(Some("A fast tensor library"), Some("# numr\n\nTensors.")),
            None
        );
        assert_eq!(scan(None, None), None);
    }
}
