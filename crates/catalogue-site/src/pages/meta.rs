//! The title, description and address of each page, for search engines and link previews.

use catalogue_core::{Category, Slug, Topic};

use super::layout::SITE_URL;
use super::shared::{category_href, project_href, topic_href};
use crate::href::SafeHref;

/// The longest description, in characters. Search results cut longer text.
const MAX_DESCRIPTION: usize = 160;

/// What a project description ends with when it fits.
const PROJECT_TAIL: &str = "See its downloads, dependents, activity and alternatives.";

/// What a topic or category description ends with.
const LIST_TAIL: &str = "Compare Rust libraries and apps by downloads, dependents and activity.";

/// The data the `<head>` of `base.html` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PageMeta {
    /// The page title, without the site name.
    pub title: String,
    /// The description. Escaped by the template, because it can hold entry text.
    pub description: String,
    /// The address of the page. `None` keeps the page out of search results.
    pub canonical: Option<SafeHref>,
}

impl PageMeta {
    /// The meta of a fixed page at `path`.
    fn fixed(title: &str, description: &str, path: &str) -> Self {
        Self {
            title: title.to_owned(),
            description: description.to_owned(),
            canonical: Some(address(path)),
        }
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
        Self {
            title: "Page not found".to_owned(),
            description: "This page is not in the ML Rust catalogue.".to_owned(),
            canonical: None,
        }
    }

    /// The page of `topic`.
    pub(crate) fn topic(topic: &Topic) -> Self {
        Self {
            title: format!("{} in Rust: libraries and apps", topic.name),
            description: fit(&format!("{}. {LIST_TAIL}", topic.name)),
            canonical: Some(address(topic_href(&topic.id).as_str())),
        }
    }

    /// The page of `category`.
    pub(crate) fn category(category: &Category) -> Self {
        Self {
            title: format!("{} in Rust: libraries and apps", category.name),
            description: fit(&format!("{} {LIST_TAIL}", category.blurb.trim())),
            canonical: Some(address(category_href(&category.id).as_str())),
        }
    }

    /// The page of the entry `slug`, named `name` and described by `description`.
    pub(crate) fn project(slug: &Slug, name: &str, description: &str) -> Self {
        let text = description.split_whitespace().collect::<Vec<_>>().join(" ");
        // Upstream descriptions often end with no punctuation.
        let stop = if text.ends_with(['.', '!', '?']) {
            ""
        } else {
            "."
        };
        let full = format!("{text}{stop} {PROJECT_TAIL}");
        Self {
            title: format!("{name}: usage, activity and alternatives"),
            description: if full.chars().count() <= MAX_DESCRIPTION {
                full
            } else {
                fit(&text)
            },
            canonical: Some(address(project_href(slug).as_str())),
        }
    }
}

/// The full address of the site path `path`.
fn address(path: &str) -> SafeHref {
    SafeHref::new(&format!("{SITE_URL}{path}"))
}

/// `text` cut at a word to at most [`MAX_DESCRIPTION`] characters, with an ellipsis when cut.
fn fit(text: &str) -> String {
    if text.chars().count() <= MAX_DESCRIPTION {
        return text.to_owned();
    }
    let kept: String = text.chars().take(MAX_DESCRIPTION - 1).collect();
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
