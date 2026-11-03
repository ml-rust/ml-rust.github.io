use serde::{Deserialize, Serialize};

/// Contents of `data/synonyms.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Synonyms {
    /// The `[[group]]` tables.
    #[serde(default)]
    pub group: Vec<SynonymGroup>,
}

/// Search terms that mean the same thing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynonymGroup {
    /// Lowercase, trimmed terms. At least two.
    pub terms: Vec<String>,
}

impl SynonymGroup {
    /// Lists every rule this group breaks on its own, without other groups.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.terms.len() < 2 {
            out.push("a group needs at least two terms".to_owned());
        }
        for term in &self.terms {
            if term.trim().is_empty() {
                out.push("a term must not be empty".to_owned());
            } else if term != term.trim() {
                out.push(format!(
                    "term `{term}` must not have surrounding whitespace"
                ));
            } else if *term != term.to_lowercase() {
                out.push(format!("term `{term}` must be lowercase"));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(terms: &[&str]) -> SynonymGroup {
        SynonymGroup {
            terms: terms.iter().map(|t| (*t).to_owned()).collect(),
        }
    }

    #[test]
    fn valid_group_passes() {
        assert!(group(&["gpu", "cuda"]).problems().is_empty());
    }

    #[test]
    fn single_term_is_reported() {
        assert!(!group(&["gpu"]).problems().is_empty());
    }

    #[test]
    fn bad_terms_are_reported() {
        assert!(!group(&["gpu", ""]).problems().is_empty());
        assert!(!group(&["gpu", " cuda"]).problems().is_empty());
        assert!(!group(&["gpu", "CUDA"]).problems().is_empty());
    }
}
