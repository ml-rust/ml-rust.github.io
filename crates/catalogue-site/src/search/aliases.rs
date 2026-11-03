//! Synonym terms an entry gains from its own text.

use catalogue_core::SynonymGroup;

/// `text` as lowercase words, each with one space on both sides.
///
/// Every character that is not a letter or a digit splits words, so
/// `k-nearest` and `k nearest` read the same.
fn words(text: &str) -> String {
    let mut out = String::from(" ");
    for word in text.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
        if !word.is_empty() {
            out.push_str(word);
            out.push(' ');
        }
    }
    out
}

/// Whether `term` occurs as whole words in `haystack`, texts from [`words`]
/// joined with `\n`. A term never spans two texts.
///
/// A term with no letter or digit occurs nowhere.
fn occurs(haystack: &str, term: &str) -> bool {
    let needle = words(term);
    needle.len() > 1 && haystack.contains(&needle)
}

/// The terms `texts` gain from `groups`, in group and term order.
///
/// A group applies when one of its terms occurs as whole words in one of `texts`.
/// It adds each of its terms that does not occur there.
pub(crate) fn alias_terms<'a>(groups: &'a [SynonymGroup], texts: &[&str]) -> Vec<&'a str> {
    let haystack = texts
        .iter()
        .map(|text| words(text))
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = Vec::new();
    for group in groups {
        let (present, missing): (Vec<&String>, Vec<&String>) =
            group.terms.iter().partition(|term| occurs(&haystack, term));
        if !present.is_empty() {
            out.extend(missing.into_iter().map(String::as_str));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(terms: &[&str]) -> SynonymGroup {
        SynonymGroup {
            terms: terms.iter().map(|term| (*term).to_owned()).collect(),
        }
    }

    #[test]
    fn a_matching_group_adds_its_missing_terms() {
        let groups = [
            group(&["llm", "large language model"]),
            group(&["asr", "speech recognition", "speech to text"]),
        ];
        let found = alias_terms(&groups, &["Runs LLM inference", "Speech-to-text models"]);
        assert_eq!(found, ["large language model", "asr", "speech recognition"]);
    }

    #[test]
    fn a_term_matches_whole_words_only() {
        let groups = [group(&["rl", "reinforcement learning"])];
        assert!(alias_terms(&groups, &["A URL parser", "world models"]).is_empty());
        assert_eq!(
            alias_terms(&groups, &["An RL library"]),
            ["reinforcement learning"]
        );
    }

    #[test]
    fn a_phrase_matches_across_punctuation_inside_one_text() {
        let groups = [group(&["knn", "k-nearest neighbors"])];
        assert_eq!(
            alias_terms(&groups, &["Fast k nearest neighbors."]),
            ["knn"]
        );
        assert!(alias_terms(&groups, &["Fast k nearest", "neighbors search"]).is_empty());
    }

    #[test]
    fn a_group_with_every_term_present_adds_nothing() {
        let groups = [group(&["onnx", "open neural network exchange"])];
        let text = ["ONNX (Open Neural Network Exchange) runtime"];
        assert!(alias_terms(&groups, &text).is_empty());
    }

    #[test]
    fn a_term_without_letters_never_matches() {
        let groups = [group(&["++", "plus plus"])];
        assert!(alias_terms(&groups, &["a b c"]).is_empty());
    }
}
