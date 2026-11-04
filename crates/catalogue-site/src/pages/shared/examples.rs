//! Example searches, for readers who do not yet know the words to type.

use crate::href::SafeHref;

/// The example terms, with the path of the search each one runs.
const EXAMPLES: [(&str, &str); 4] = [
    ("speech recognition", "/search/?q=speech+recognition"),
    ("GGUF", "/search/?q=GGUF"),
    ("vector database", "/search/?q=vector+database"),
    ("embeddings", "/search/?q=embeddings"),
];

/// One example search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExampleSearch {
    /// The search term.
    pub text: &'static str,
    /// The link that runs the search.
    pub href: SafeHref,
}

/// Every example search, in display order.
pub(crate) fn example_searches() -> Vec<ExampleSearch> {
    EXAMPLES
        .into_iter()
        .map(|(text, path)| ExampleSearch {
            text,
            href: SafeHref::new(path),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_example_links_a_search_with_its_term() {
        for example in example_searches() {
            assert!(!example.href.is_refused(), "{}", example.text);
            let query = example.href.as_str().replace('+', " ");
            assert_eq!(query, format!("/search/?q={}", example.text));
        }
    }
}
