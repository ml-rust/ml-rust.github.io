//! The term rule of discovery: whole words in a crate name or description.

use std::collections::HashMap;

/// The words of `text`, split at every character that is not a letter or digit. Case stays.
///
/// This is the one definition of a word for discovery and for the seed mappings.
pub(crate) fn text_words(text: &str) -> impl Iterator<Item = &str> + Clone {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
}

/// One rule term split into words.
struct Term<'a> {
    /// The term as the rules write it.
    text: &'a str,
    /// The words after the first.
    rest: Vec<&'a str>,
}

/// The rule terms by their first word, so a scan looks up each word of a text once.
pub(super) struct TermIndex<'a> {
    by_first_word: HashMap<&'a str, Vec<Term<'a>>>,
}

impl<'a> TermIndex<'a> {
    /// Indexes `terms`. A term is lowercase words separated by one space.
    pub(super) fn new(terms: &'a [String]) -> Self {
        let mut by_first_word: HashMap<&'a str, Vec<Term<'a>>> = HashMap::new();
        for text in terms {
            let mut parts = text.split(' ');
            if let Some(first) = parts.next() {
                by_first_word.entry(first).or_default().push(Term {
                    text,
                    rest: parts.collect(),
                });
            }
        }
        Self { by_first_word }
    }

    /// Calls `hit` with each term that `text` holds.
    ///
    /// A term matches whole words in order, ignoring case and punctuation, as
    /// `topics_by_terms` does. `word` is scratch space that the caller reuses across texts, so
    /// the scan allocates nothing per crate. A repeated term gives a repeated call.
    pub(super) fn each_term(&self, text: &str, word: &mut String, mut hit: impl FnMut(&'a str)) {
        let mut words = text_words(text);
        while let Some(next) = words.next() {
            word.clear();
            word.extend(next.chars().flat_map(char::to_lowercase));
            let Some(terms) = self.by_first_word.get(word.as_str()) else {
                continue;
            };
            for term in terms {
                let mut ahead = words.clone();
                let whole = term
                    .rest
                    .iter()
                    .all(|part| ahead.next().is_some_and(|word| same_word(word, part)));
                if whole {
                    hit(term.text);
                }
            }
        }
    }
}

/// Whether the lowercase form of `word` is `part`.
fn same_word(word: &str, part: &str) -> bool {
    word.chars().flat_map(char::to_lowercase).eq(part.chars())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(terms: &[&str], text: &str) -> Vec<String> {
        let terms: Vec<String> = terms.iter().map(|term| (*term).to_owned()).collect();
        let index = TermIndex::new(&terms);
        let mut hits = Vec::new();
        index.each_term(text, &mut String::new(), |term| hits.push(term.to_owned()));
        hits
    }

    #[test]
    fn a_dot_and_a_dash_are_word_breaks() {
        for text in ["Bindings for whisper.cpp", "whisper-cpp", "WHISPER_CPP"] {
            assert_eq!(found(&["whisper cpp"], text), ["whisper cpp"], "{text}");
        }
    }

    #[test]
    fn a_term_matches_whole_words_only() {
        assert!(found(&["tensor"], "Tensorial tensors of a subtensor.").is_empty());
        assert!(found(&["whisper cpp"], "whisper cppish").is_empty());
        assert!(found(&["whisper cpp"], "cpp whisper").is_empty());
        assert!(found(&["whisper cpp"], "whisper").is_empty());
    }

    #[test]
    fn two_terms_with_one_first_word_both_match() {
        assert_eq!(
            found(
                &["vector search", "vector search engine"],
                "A vector search engine."
            ),
            ["vector search", "vector search engine"]
        );
    }

    #[test]
    fn scratch_space_carries_nothing_between_texts() {
        let terms = vec!["llm".to_owned()];
        let index = TermIndex::new(&terms);
        let mut word = String::new();
        let mut hits = 0;
        index.each_term("LLM", &mut word, |_| hits += 1);
        index.each_term("A zip reader.", &mut word, |_| hits += 1);
        assert_eq!(hits, 1);
    }
}
