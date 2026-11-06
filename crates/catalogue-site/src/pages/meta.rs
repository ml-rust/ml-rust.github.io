//! The title, description and address of each page, for search engines and link previews.

use catalogue_core::{Category, Slug, Topic};

use super::layout::{SHORT_SITE_NAME, SITE_NAME, SITE_URL};
use super::shared::{category_href, project_href, topic_href};
use crate::href::SafeHref;

/// The longest description, in characters. Search results cut longer text.
const MAX_DESCRIPTION: usize = 160;

/// The longest `<title>`, in characters. Search results cut longer text.
const MAX_TITLE: usize = 65;

/// What a project title adds to the project name, longest first.
const PROJECT_TITLE_DETAILS: [&str; 2] = [
    ": usage, activity and alternatives",
    ": usage and alternatives",
];

/// What a topic or category title adds to its name.
const LIST_TITLE_DETAILS: [&str; 1] = [" in Rust: libraries and apps"];

/// What a project description ends with when it fits.
const PROJECT_TAIL: &str = "See its downloads, dependents, activity and alternatives.";

/// What a topic or category description ends with when it fits.
const LIST_TAIL: &str = "Compare Rust libraries and apps by downloads, dependents and activity.";

/// The data the `<head>` of `base.html` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PageMeta {
    /// The title a link preview shows, without the site name.
    pub title: String,
    /// The `<title>` text, at most [`MAX_TITLE`] characters.
    pub document_title: String,
    /// The description. Escaped by the template, because it can hold entry text.
    pub description: String,
    /// The address of the page. `None` keeps the page out of search results.
    pub canonical: Option<SafeHref>,
}

impl PageMeta {
    /// The meta of a page named `name`, whose title adds the first of `details` that fits.
    fn new(name: &str, details: &[&str], description: String, canonical: Option<SafeHref>) -> Self {
        let (title, document_title) = titles(name, details);
        Self {
            title,
            document_title,
            description,
            canonical,
        }
    }

    /// The meta of a fixed page at `path`.
    fn fixed(title: &str, description: &str, path: &str) -> Self {
        Self::new(title, &[], description.to_owned(), Some(address(path)))
    }

    /// The home page.
    pub(crate) fn home() -> Self {
        Self::fixed(
            "Rust machine learning libraries and apps",
            "Find the right Rust library or app for your ML job. Compare LLM, inference, \
             embeddings, speech and vector search projects by real downloads and activity.",
            "/",
        )
    }

    /// The Featured list.
    pub(crate) fn featured() -> Self {
        Self::fixed(
            "Featured Rust machine learning projects",
            "Rust machine learning libraries and apps that pass every adoption, health and docs \
             check, with real downloads and activity.",
            "/featured/",
        )
    }

    /// The list of the organization's own projects.
    pub(crate) fn ml_rust() -> Self {
        Self::fixed(
            "ML Rust organization projects",
            "Machine learning projects maintained by the ML Rust organization: tensors, \
             training, inference, tokenizers, speech and more.",
            "/ml-rust/",
        )
    }

    /// The state of ML in Rust.
    pub(crate) fn state() -> Self {
        Self::fixed(
            "The state of ML in Rust",
            "Can you build real machine learning products in Rust today? What works, what to \
             build with, and the real gaps, updated every week.",
            "/state/",
        )
    }

    /// The jobs and categories.
    pub(crate) fn topics() -> Self {
        Self::fixed(
            "Browse Rust machine learning projects by job",
            "Browse Rust machine learning libraries and apps by what you need to do: run \
             models, build LLM apps, train, search text, process speech and more.",
            "/topics/",
        )
    }

    /// The tier change log.
    pub(crate) fn changes() -> Self {
        Self::fixed(
            "Tier changes and new projects",
            "New Rust machine learning projects and every tier change in the catalogue, by date.",
            "/changes/",
        )
    }

    /// The tier rules.
    pub(crate) fn rules() -> Self {
        Self::fixed(
            "How projects earn a tier",
            "The published rules behind every tier: adoption, health, docs and how the catalogue \
             detects unmaintained Rust projects.",
            "/rules/",
        )
    }

    /// The issue forms.
    pub(crate) fn submit() -> Self {
        Self::fixed(
            "Add or correct a project",
            "Add a Rust machine learning library or app to the catalogue, or report a wrong fact \
             on a listed project.",
            "/submit/",
        )
    }

    /// The search page.
    pub(crate) fn search() -> Self {
        Self::fixed(
            "Search Rust machine learning projects",
            "Search Rust machine learning libraries and apps by name or job, and filter by kind, \
             backend and tier.",
            "/search/",
        )
    }

    /// The compare page.
    pub(crate) fn compare() -> Self {
        Self::fixed(
            "Compare Rust machine learning projects",
            "Compare up to 4 Rust machine learning libraries and apps side by side on downloads, \
             dependents and activity.",
            "/compare/",
        )
    }

    /// The not-found page, which stays out of search results.
    pub(crate) fn not_found() -> Self {
        Self::new(
            "Page not found",
            &[],
            "This page is not in the ML Rust catalogue.".to_owned(),
            None,
        )
    }

    /// The page of `topic`.
    pub(crate) fn topic(topic: &Topic) -> Self {
        Self::new(
            &topic.name,
            &LIST_TITLE_DETAILS,
            with_tail(&topic.blurb, LIST_TAIL),
            Some(address(topic_href(&topic.id).as_str())),
        )
    }

    /// The page of `category`.
    pub(crate) fn category(category: &Category) -> Self {
        Self::new(
            &category.name,
            &LIST_TITLE_DETAILS,
            with_tail(&category.blurb, LIST_TAIL),
            Some(address(category_href(&category.id).as_str())),
        )
    }

    /// The page of the entry `slug`, named `name` and described by `description`.
    pub(crate) fn project(slug: &Slug, name: &str, description: &str) -> Self {
        Self::new(
            name,
            &PROJECT_TITLE_DETAILS,
            with_tail(description, PROJECT_TAIL),
            Some(address(project_href(slug).as_str())),
        )
    }
}

/// The full address of the site path `path`.
fn address(path: &str) -> SafeHref {
    SafeHref::new(&format!("{SITE_URL}{path}"))
}

/// The preview title and the `<title>` text of the page named `name`.
///
/// The name always survives whole when it fits alone. A longer form wins over a shorter one: each
/// of `details` in order, then the name alone, each with the full and then the short site name.
fn titles(name: &str, details: &[&str]) -> (String, String) {
    let bodies = details
        .iter()
        .map(|detail| format!("{name}{detail}"))
        .chain([name.to_owned()]);
    for body in bodies {
        for site in [SITE_NAME, SHORT_SITE_NAME] {
            let document_title = format!("{body} · {site}");
            if document_title.chars().count() <= MAX_TITLE {
                return (body, document_title);
            }
        }
    }
    let alone = cut(name, MAX_TITLE);
    (alone.clone(), alone)
}

/// `text` with single spaces, followed by `tail` when both fit in [`MAX_DESCRIPTION`] characters.
fn with_tail(text: &str, tail: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    // Upstream descriptions often end with no punctuation.
    let stop = if text.ends_with(['.', '!', '?']) {
        ""
    } else {
        "."
    };
    let full = format!("{text}{stop} {tail}");
    if full.chars().count() <= MAX_DESCRIPTION {
        full
    } else {
        fit(&text)
    }
}

/// `text` cut to at most [`MAX_DESCRIPTION`] characters.
fn fit(text: &str) -> String {
    cut(text, MAX_DESCRIPTION)
}

/// `text` cut at a word to at most `limit` characters, with an ellipsis when cut.
///
/// A single word longer than `limit` has no word end to cut at, so the cut falls inside it.
fn cut(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let kept: String = text.chars().take(limit - 1).collect();
    let cut = kept
        .rfind(' ')
        .map_or(kept.as_str(), |space| &kept[..space]);
    format!("{}…", cut.trim_end_matches([' ', ',', '.', ':']))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_text_stays_whole() {
        assert_eq!(fit("A short text."), "A short text.");
    }

    #[test]
    fn a_long_text_ends_at_a_word_within_the_limit() {
        let text = "word, ".repeat(60);
        let cut = fit(&text);
        assert!(cut.chars().count() <= MAX_DESCRIPTION);
        assert!(cut.ends_with("word…"), "{cut}");
    }

    #[test]
    fn a_cut_never_splits_a_character() {
        let cut = fit(&"日本語 ".repeat(80));
        assert!(cut.chars().count() <= MAX_DESCRIPTION);
        assert!(cut.ends_with("日本語…"), "{cut}");
    }

    #[test]
    fn every_fixed_page_fits_and_has_an_address() {
        for meta in [
            PageMeta::home(),
            PageMeta::featured(),
            PageMeta::ml_rust(),
            PageMeta::topics(),
            PageMeta::changes(),
            PageMeta::rules(),
            PageMeta::submit(),
            PageMeta::search(),
            PageMeta::compare(),
        ] {
            assert!(
                meta.description.chars().count() <= MAX_DESCRIPTION,
                "{}",
                meta.description
            );
            let url = meta.canonical.expect("an address");
            assert!(url.as_str().starts_with(SITE_URL), "{url}");
            assert!(!url.is_refused());
        }
    }

    #[test]
    fn the_not_found_page_has_no_address() {
        assert!(PageMeta::not_found().canonical.is_none());
    }

    #[test]
    fn every_fixed_title_keeps_its_words_and_the_full_site_name() {
        for meta in [
            PageMeta::home(),
            PageMeta::featured(),
            PageMeta::ml_rust(),
            PageMeta::topics(),
            PageMeta::changes(),
            PageMeta::rules(),
            PageMeta::submit(),
            PageMeta::search(),
            PageMeta::compare(),
            PageMeta::not_found(),
        ] {
            assert_eq!(meta.document_title, format!("{} · {SITE_NAME}", meta.title));
            assert!(meta.document_title.chars().count() <= MAX_TITLE);
        }
    }

    #[test]
    fn a_project_title_gives_way_to_the_name_as_the_name_grows() {
        let title = |len: usize| titles(&"n".repeat(len), &PROJECT_TITLE_DETAILS).1;
        let name = |len: usize| "n".repeat(len);
        for (len, want) in [
            (
                11,
                format!(
                    "{}: usage, activity and alternatives · ML Rust catalogue",
                    name(11)
                ),
            ),
            (
                21,
                format!("{}: usage, activity and alternatives · ML Rust", name(21)),
            ),
            (
                31,
                format!("{}: usage and alternatives · ML Rust", name(31)),
            ),
            (45, format!("{} · ML Rust catalogue", name(45))),
            (55, format!("{} · ML Rust", name(55))),
            (65, name(65)),
        ] {
            let found = title(len);
            assert_eq!(found, want);
            assert_eq!(found.chars().count(), 65, "{found}");
            assert!(title(len + 1).chars().count() <= MAX_TITLE);
            assert_ne!(
                title(len + 1).replace('n', ""),
                found.replace('n', ""),
                "{len}"
            );
        }
    }

    #[test]
    fn a_list_title_keeps_its_detail_with_the_short_site_name() {
        let (title, document_title) = titles("Text, embeddings and search", &LIST_TITLE_DETAILS);
        assert_eq!(
            title,
            "Text, embeddings and search in Rust: libraries and apps"
        );
        assert_eq!(
            document_title,
            "Text, embeddings and search in Rust: libraries and apps · ML Rust"
        );
        assert_eq!(document_title.chars().count(), MAX_TITLE);
    }

    #[test]
    fn a_name_too_long_for_a_title_ends_at_a_word() {
        let name = "word ".repeat(30);
        let (title, document_title) = titles(name.trim(), &PROJECT_TITLE_DETAILS);
        assert_eq!(title, document_title);
        assert!(document_title.chars().count() <= MAX_TITLE);
        assert!(document_title.ends_with("word…"), "{document_title}");
    }

    #[test]
    fn a_topic_or_category_description_holds_its_definition_whole() {
        let blurb = "Word ".repeat(40);
        assert_eq!(
            with_tail("A line.", LIST_TAIL),
            format!("A line. {LIST_TAIL}")
        );
        let room = MAX_DESCRIPTION - LIST_TAIL.chars().count() - 1;
        let longest = "w".repeat(room - 1) + ".";
        assert_eq!(
            with_tail(&longest, LIST_TAIL),
            format!("{longest} {LIST_TAIL}")
        );
        let over = "w".repeat(room) + ".";
        assert_eq!(with_tail(&over, LIST_TAIL), over);
        assert!(with_tail(&blurb, LIST_TAIL).chars().count() <= MAX_DESCRIPTION);
    }

    #[test]
    fn a_project_description_gains_the_tail_only_when_it_fits() {
        let slug: Slug = "alpha".parse().expect("slug");
        let short = PageMeta::project(&slug, "alpha", "Tensors   for\nRust.");
        assert_eq!(
            short.description,
            format!("Tensors for Rust. {PROJECT_TAIL}")
        );
        assert_eq!(short.title, "alpha: usage, activity and alternatives");
        let bare = PageMeta::project(&slug, "alpha", "Tensors for Rust");
        assert_eq!(
            bare.description,
            format!("Tensors for Rust. {PROJECT_TAIL}")
        );
        let long = PageMeta::project(&slug, "alpha", &"tensor ".repeat(60));
        assert!(!long.description.contains(PROJECT_TAIL));
        assert!(long.description.chars().count() <= MAX_DESCRIPTION);
        assert_eq!(
            long.canonical.expect("an address").as_str(),
            "https://ml-rust.github.io/projects/alpha/"
        );
    }
}
