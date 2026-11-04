//! The entry fields that derive from crate metadata.

use std::collections::BTreeSet;

use catalogue_core::{Slug, Topics, repo_link};
use url::Url;

use super::primary::words;

/// The key that groups the crates of one repository: the normalised `host/owner/repo`.
///
/// `None` when the link is not an https link with an owner and a name.
pub(super) fn repo_key(raw: Option<&str>) -> Option<String> {
    let (url, _) = repo_link(raw).ok()?;
    Some(url.as_str().trim_start_matches("https://").to_owned())
}

/// The homepage when it is an https link.
pub(super) fn https_link(raw: Option<&str>) -> Option<Url> {
    let url = Url::parse(raw?.trim()).ok()?;
    (url.scheme() == "https" && url.host_str().is_some()).then_some(url)
}

/// The ids of the topics that map to a crate's keywords or categories, sorted.
///
/// A topic keyword matches an equal keyword, ignoring case. A topic category
/// matches an equal category slug or a slug that extends it with `::`.
pub(super) fn matching_topics(
    topics: &Topics,
    keywords: &[String],
    categories: &[String],
) -> Vec<Slug> {
    let keywords: BTreeSet<String> = keywords.iter().map(|k| k.to_lowercase()).collect();
    let found: BTreeSet<&Slug> = topics
        .topic
        .iter()
        .filter(|topic| {
            topic.keywords.iter().any(|k| keywords.contains(k))
                || topic
                    .categories
                    .iter()
                    .any(|c| categories.iter().any(|slug| in_category(c, slug)))
        })
        .map(|topic| &topic.id)
        .collect();
    found.into_iter().cloned().collect()
}

/// The ids of the topics with a term in `name` or `description`, sorted.
///
/// A term matches whole words in order, ignoring case and punctuation. It never matches across
/// the name and the description.
pub(super) fn topics_by_terms(topics: &Topics, name: &str, description: Option<&str>) -> Vec<Slug> {
    topics_by_terms_in(topics, &[name, description.unwrap_or_default()])
}

/// The ids of the topics with a term in one of `texts`, sorted.
///
/// A term matches whole words in order, ignoring case and punctuation. It never matches across
/// two texts.
pub(super) fn topics_by_terms_in(topics: &Topics, texts: &[&str]) -> Vec<Slug> {
    let texts: Vec<Vec<String>> = texts.iter().map(|text| words(text)).collect();
    let found: BTreeSet<&Slug> = topics
        .topic
        .iter()
        .filter(|topic| {
            topic
                .terms
                .iter()
                .any(|term| texts.iter().any(|text| holds_term(text, term)))
        })
        .map(|topic| &topic.id)
        .collect();
    found.into_iter().cloned().collect()
}

fn holds_term(words: &[String], term: &str) -> bool {
    let parts: Vec<&str> = term.split(' ').collect();
    words
        .windows(parts.len().max(1))
        .any(|window| window.iter().map(String::as_str).eq(parts.iter().copied()))
}

/// Whether `slug` equals `category` or extends it with `::`.
fn in_category(category: &str, slug: &str) -> bool {
    slug == category
        || slug
            .strip_prefix(category)
            .is_some_and(|rest| rest.starts_with("::"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn topics(text: &str) -> Topics {
        toml::from_str(text).expect("topics parse")
    }

    fn ids(found: Vec<Slug>) -> Vec<String> {
        found.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn repo_key_is_the_normalised_host_owner_and_name() {
        for raw in [
            "https://github.com/Foo/Bar",
            "https://github.com/foo/bar.git",
            "https://www.github.com/foo/bar/tree/main/sub",
            " https://github.com/foo/bar/ ",
        ] {
            assert_eq!(repo_key(Some(raw)).as_deref(), Some("github.com/foo/bar"));
        }
    }

    #[test]
    fn unusable_repositories_have_no_key() {
        assert_eq!(repo_key(Some("http://github.com/a/b")), None);
        assert_eq!(repo_key(None), None);
    }

    #[test]
    fn homepage_must_be_https() {
        assert_eq!(
            https_link(Some("https://example.com/x")).map(|u| u.to_string()),
            Some("https://example.com/x".to_owned())
        );
        assert_eq!(https_link(Some("http://example.com")), None);
        assert_eq!(https_link(Some("not a link")), None);
        assert_eq!(https_link(Some("")), None);
        assert_eq!(https_link(None), None);
    }

    const TOPICS: &str = r#"
[[topic]]
id = "nlp"
name = "NLP"
keywords = ["nlp", "text"]

[[topic]]
id = "foundations"
name = "Foundations"
keywords = ["tensor"]
categories = ["science::math"]

[[topic]]
id = "other"
name = "Other"
"#;

    #[test]
    fn keywords_match_topics_sorted_by_id() {
        let words = vec!["Text".to_owned(), "tensor".to_owned(), "zip".to_owned()];
        assert_eq!(
            ids(matching_topics(&topics(TOPICS), &words, &[])),
            ["foundations", "nlp"]
        );
    }

    #[test]
    fn categories_match_equal_and_extending_slugs() {
        let found = |slug: &str| ids(matching_topics(&topics(TOPICS), &[], &[slug.to_owned()]));
        assert_eq!(found("science::math"), ["foundations"]);
        assert_eq!(found("science::math::linear"), ["foundations"]);
        assert!(found("science::mathematics").is_empty());
        assert!(found("science").is_empty());
    }

    #[test]
    fn keyword_and_category_of_one_topic_match_it_once() {
        let words = vec!["tensor".to_owned()];
        let cats = vec!["science::math".to_owned()];
        assert_eq!(
            ids(matching_topics(&topics(TOPICS), &words, &cats)),
            ["foundations"]
        );
    }

    #[test]
    fn no_match_gives_no_topic() {
        let words = vec!["zip".to_owned()];
        assert!(matching_topics(&topics(TOPICS), &words, &[]).is_empty());
    }

    const TERMS: &str = r#"
[[topic]]
id = "search"
name = "Search"
terms = ["embeddings", "vector database"]

[[topic]]
id = "audio"
name = "Audio"
terms = ["transcribe", "diarization"]

[[topic]]
id = "tabular"
name = "Tabular"
terms = ["t sne"]

[[topic]]
id = "keyword-only"
name = "Keyword only"
keywords = ["tensor"]
"#;

    fn by_terms(name: &str, description: &str) -> Vec<String> {
        ids(topics_by_terms(&topics(TERMS), name, Some(description)))
    }

    #[test]
    fn terms_find_crates_without_a_mapped_keyword() {
        for (name, description, expected) in [
            (
                "fastembed",
                "Library for generating vector embeddings, reranking locally.",
                "search",
            ),
            (
                "lancedb",
                "LanceDB: A serverless, low-latency vector database for AI applications",
                "search",
            ),
            (
                "transcribe-rs",
                "A simple library to help you transcribe audio",
                "audio",
            ),
            (
                "pyannote-rs",
                "Speaker diarization using pyannote in Rust",
                "audio",
            ),
            (
                "bhtsne",
                "Exact and Barnes-Hut implementations of t-SNE.",
                "tabular",
            ),
        ] {
            assert_eq!(by_terms(name, description), [expected], "{name}");
        }
    }

    #[test]
    fn terms_match_in_the_name_too() {
        assert_eq!(by_terms("transcribe-tool", "A tool."), ["audio"]);
    }

    #[test]
    fn noise_crates_match_no_term() {
        for (name, description) in [
            ("nu-command", "Nushell's built-in commands"),
            ("ocaml", "OCaml bindings for Rust"),
            (
                "rust_decimal",
                "Decimal number implementation written in pure Rust.",
            ),
            ("cloudflare", "Rust library for the Cloudflare v4 API"),
            (
                "risc0-zkp",
                "RISC Zero zero-knowledge proof system core crate",
            ),
        ] {
            assert!(by_terms(name, description).is_empty(), "{name}");
        }
    }

    /// The published topics.
    const PUBLISHED: &str = include_str!("../../../../data/topics.toml");

    #[test]
    fn published_terms_reject_a_parallelism_library_and_a_cloud_sdk() {
        let published = topics(PUBLISHED);
        for texts in [
            [
                "work-stealer",
                "A data parallelism library for Rust.",
                "parallelism",
                "concurrency",
            ],
            [
                "cloud-sdk-rust",
                "The SDK for a cloud platform, with one client per service.",
                "cloud",
                "sdk",
            ],
            [
                "string-search",
                "Fast multiple substring searching.",
                "text",
                "search",
            ],
        ] {
            assert!(
                topics_by_terms_in(&published, &texts).is_empty(),
                "{texts:?}"
            );
        }
    }

    #[test]
    fn published_keywords_reject_a_plain_text_crate() {
        let words = vec!["text".to_owned(), "string".to_owned(), "search".to_owned()];
        assert!(matching_topics(&topics(PUBLISHED), &words, &[]).is_empty());
    }

    #[test]
    fn published_terms_read_github_topics() {
        let published = topics(PUBLISHED);
        let found = ids(topics_by_terms_in(
            &published,
            &["tool", "A tool.", "deep-learning"],
        ));
        assert_eq!(found, ["train-models"]);
    }

    #[test]
    fn a_term_does_not_span_two_texts() {
        let terms = topics(TERMS);
        assert!(topics_by_terms_in(&terms, &["vector", "database"]).is_empty());
    }

    #[test]
    fn a_term_matches_whole_words_only() {
        assert!(by_terms("x", "An embeddingsish transcribed thing.").is_empty());
    }

    #[test]
    fn a_phrase_needs_its_words_in_order_and_together() {
        assert!(by_terms("x", "A database of vector images.").is_empty());
        assert!(by_terms("x", "A vector of database rows.").is_empty());
    }

    #[test]
    fn a_phrase_does_not_span_the_name_and_the_description() {
        assert!(by_terms("vector", "Database rows.").is_empty());
    }

    #[test]
    fn a_missing_description_matches_on_the_name_alone() {
        assert_eq!(
            ids(topics_by_terms(&topics(TERMS), "diarization", None)),
            ["audio"]
        );
    }

    #[test]
    fn a_keyword_is_not_a_term() {
        assert!(by_terms("tensor", "A tensor library.").is_empty());
    }
}
