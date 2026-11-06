//! The entry fields that derive from crate metadata.

use std::collections::BTreeSet;

use catalogue_core::{Categories, Slug, Topics, repo_link};
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
    let keywords = lowercase(keywords);
    let found: BTreeSet<&Slug> = topics
        .topic
        .iter()
        .filter(|topic| maps_metadata(&topic.keywords, &topic.categories, &keywords, categories))
        .map(|topic| &topic.id)
        .collect();
    found.into_iter().cloned().collect()
}

/// The first category, in file order, that maps to these keywords or crates.io category slugs.
///
/// The mapping rules are those of [`matching_topics`].
pub(super) fn category_by_metadata(
    file: &Categories,
    keywords: &[String],
    categories: &[String],
) -> Option<Slug> {
    let keywords = lowercase(keywords);
    file.category
        .iter()
        .find(|category| {
            maps_metadata(
                &category.keywords,
                &category.categories,
                &keywords,
                categories,
            )
        })
        .map(|category| category.id.clone())
}

/// The first category, in file order, with a term in one of `texts`.
///
/// The term rules are those of [`topics_by_terms_in`].
pub(super) fn category_by_terms(file: &Categories, texts: &[&str]) -> Option<Slug> {
    let texts = word_lists(texts);
    file.category
        .iter()
        .find(|category| holds_any_term(&category.terms, &texts))
        .map(|category| category.id.clone())
}

fn lowercase(keywords: &[String]) -> BTreeSet<String> {
    keywords.iter().map(|k| k.to_lowercase()).collect()
}

fn word_lists(texts: &[&str]) -> Vec<Vec<String>> {
    texts.iter().map(|text| words(text)).collect()
}

/// Whether a record with `own_keywords` and `own_categories` maps to this crate metadata.
fn maps_metadata(
    own_keywords: &[String],
    own_categories: &[String],
    keywords: &BTreeSet<String>,
    categories: &[String],
) -> bool {
    own_keywords.iter().any(|k| keywords.contains(k))
        || own_categories
            .iter()
            .any(|c| categories.iter().any(|slug| in_category(c, slug)))
}

/// Whether one of `terms` appears in one of `texts`.
fn holds_any_term(terms: &[String], texts: &[Vec<String>]) -> bool {
    terms
        .iter()
        .any(|term| texts.iter().any(|text| holds_term(text, term)))
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
    let texts = word_lists(texts);
    let found: BTreeSet<&Slug> = topics
        .topic
        .iter()
        .filter(|topic| holds_any_term(&topic.terms, &texts))
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
blurb = "A job."
keywords = ["nlp", "text"]

[[topic]]
id = "foundations"
name = "Foundations"
blurb = "A job."
keywords = ["tensor"]
categories = ["science::math"]

[[topic]]
id = "other"
name = "Other"
blurb = "A job."
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
blurb = "A job."
terms = ["embeddings", "vector database"]

[[topic]]
id = "audio"
name = "Audio"
blurb = "A job."
terms = ["transcribe", "diarization"]

[[topic]]
id = "tabular"
name = "Tabular"
blurb = "A job."
terms = ["t sne"]

[[topic]]
id = "keyword-only"
name = "Keyword only"
blurb = "A job."
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

    /// The published categories.
    const PUBLISHED_CATEGORIES: &str = include_str!("../../../../data/categories.toml");

    fn published_categories() -> Categories {
        toml::from_str(PUBLISHED_CATEGORIES).expect("categories parse")
    }

    /// Two categories that share a crate. The later one sorts first by id.
    const ORDERED: &str = r#"
[[category]]
id = "zeta"
name = "Zeta"
blurb = "Zeta."
keywords = ["shared"]
terms = ["shared term"]

[[category]]
id = "alpha"
name = "Alpha"
blurb = "Alpha."
keywords = ["other"]
categories = ["science::math"]
terms = ["other term"]
"#;

    fn ordered() -> Categories {
        toml::from_str(ORDERED).expect("categories parse")
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn id(found: Option<Slug>) -> Option<String> {
        found.map(|slug| slug.to_string())
    }

    #[test]
    fn the_first_category_in_file_order_wins_on_metadata() {
        let found = category_by_metadata(&ordered(), &strings(&["other", "Shared"]), &[]);
        assert_eq!(id(found).as_deref(), Some("zeta"));
        let by_slug = category_by_metadata(&ordered(), &[], &strings(&["science::math::linear"]));
        assert_eq!(id(by_slug).as_deref(), Some("alpha"));
    }

    #[test]
    fn the_first_category_in_file_order_wins_on_terms() {
        let found = category_by_terms(&ordered(), &["An other term.", "A shared term."]);
        assert_eq!(id(found).as_deref(), Some("zeta"));
    }

    #[test]
    fn no_match_gives_no_category() {
        assert_eq!(
            category_by_metadata(&ordered(), &strings(&["zip"]), &[]),
            None
        );
        assert_eq!(category_by_terms(&ordered(), &["A shared thing."]), None);
    }

    #[test]
    fn published_categories_are_in_the_promised_order() {
        let published = published_categories();
        let names: Vec<&str> = published
            .category
            .iter()
            .map(|category| category.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "Speech and audio",
                "Computer vision",
                "Inference runtimes",
                "LLM clients",
                "Agents and LLM apps",
                "Vector databases",
                "Embeddings",
                "Search engines",
                "Text and tokenizers",
                "Model formats",
                "Data formats and dataframes",
                "Classical ML",
                "ML frameworks",
                "Tensors and math",
                "Developer tools",
            ]
        );
    }

    #[test]
    fn published_terms_name_what_a_project_is() {
        let published = published_categories();
        for (text, expected) in [
            (
                "A serverless vector database for AI apps.",
                "vector-databases",
            ),
            (
                "Tract: ONNX inference for neural nets.",
                "inference-runtimes",
            ),
            ("A parser for the GGUF container.", "model-formats"),
            (
                "Read and write SafeTensors with tensors inside.",
                "model-formats",
            ),
            ("An N-dimensional array crate.", "math-libraries"),
            ("A deep learning framework.", "ml-frameworks"),
            ("Experiment tracking for runs.", "developer-tools"),
        ] {
            let found = category_by_terms(&published, &[text]);
            assert_eq!(id(found).as_deref(), Some(expected), "{text}");
        }
    }

    #[test]
    fn published_terms_skip_words_with_a_second_meaning() {
        let published = published_categories();
        for text in [
            "Type inference for a small language.",
            "Embedding a scripting language in Rust.",
            "A machine learning crate.",
            "An AI helper.",
            "Reads audio files.",
            "A data parallelism library for Rust.",
        ] {
            assert_eq!(category_by_terms(&published, &[text]), None, "{text}");
        }
    }

    #[test]
    fn published_terms_place_a_project_by_its_own_description() {
        let (topics, categories) = (topics(PUBLISHED), published_categories());
        for (name, description, topic, category) in [
            (
                "whisper-rs",
                "Rust bindings for whisper.cpp",
                "speech-audio",
                Some("speech-audio"),
            ),
            (
                "llama-cpp-2",
                "llama.cpp bindings for Rust",
                "run-models",
                Some("inference-runtimes"),
            ),
            (
                "hf-hub",
                "Rust client for the Hugging Face Hub API",
                "data-formats",
                Some("developer-tools"),
            ),
            (
                "hf-tool",
                "Downloads files from HuggingFace.",
                "data-formats",
                Some("developer-tools"),
            ),
            (
                "rmcp",
                "Rust SDK for Model Context Protocol",
                "llm-apps",
                Some("agent-frameworks"),
            ),
            (
                "usearch",
                "Smaller & Faster Single-File Vector Search Engine from Unum",
                "text-search",
                Some("vector-databases"),
            ),
            (
                "xgboost",
                "Machine learning using XGBoost",
                "tabular-ml",
                Some("classical-ml"),
            ),
            (
                "lightgbm",
                "Machine learning using LightGBM",
                "tabular-ml",
                Some("classical-ml"),
            ),
            (
                "catboost",
                "Catboost inference library for Rust",
                "tabular-ml",
                Some("classical-ml"),
            ),
            (
                "langchain-rust",
                "LangChain for Rust, the easiest way to write LLM-based programs in Rust",
                "llm-apps",
                Some("agent-frameworks"),
            ),
            (
                "solvr",
                "Advanced computing library for real-world problem solving - optimization, \
                 differential equations, interpolation, statistics, and more",
                "math-science",
                Some("math-libraries"),
            ),
            (
                "sci",
                "A scientific computing library.",
                "math-science",
                Some("math-libraries"),
            ),
            (
                "venus",
                "Reactive notebook environment for Rust",
                "math-science",
                Some("developer-tools"),
            ),
            (
                "venus-repo",
                "Reactive notebooks for Rust.",
                "math-science",
                Some("developer-tools"),
            ),
        ] {
            assert_eq!(
                ids(topics_by_terms(&topics, name, Some(description))),
                [topic],
                "{name}"
            );
            assert_eq!(
                id(category_by_terms(&categories, &[name, description])).as_deref(),
                category,
                "{name}"
            );
        }
    }

    #[test]
    fn an_earlier_category_wins_over_a_hugging_face_mention() {
        let found = category_by_terms(
            &published_categories(),
            &["A tokenizer for Hugging Face models."],
        );
        assert_eq!(id(found).as_deref(), Some("text-tools"));
    }

    #[test]
    fn a_plain_notebook_is_no_notebook_environment() {
        let text = ["notes", "A notebook for plain text notes."];
        assert!(topics_by_terms_in(&topics(PUBLISHED), &text).is_empty());
        assert_eq!(category_by_terms(&published_categories(), &text), None);
    }

    #[test]
    fn published_metadata_skips_the_generic_markers() {
        let published = published_categories();
        let generic = strings(&["machine-learning", "ml", "ai"]);
        let found = category_by_metadata(&published, &generic, &strings(&["science::ml"]));
        assert_eq!(found, None);
    }

    #[test]
    fn published_metadata_prefers_a_runtime_over_a_format() {
        let found = category_by_metadata(
            &published_categories(),
            &strings(&["onnx", "onnxruntime"]),
            &[],
        );
        assert_eq!(id(found).as_deref(), Some("inference-runtimes"));
    }
}
