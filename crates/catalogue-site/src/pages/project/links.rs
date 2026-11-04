//! The description of an entry with its source, and the entry's outside links.

use catalogue_core::{
    DescriptionSource, Entry, crate_page, docker_page, homebrew_page, repology_page,
};
use url::Url;

use crate::href::SafeHref;
use crate::pages::shared::docs_href;

/// The data the description section of `project.html` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DescriptionView<'a> {
    /// The description text.
    pub text: &'a str,
    /// Where the text comes from.
    pub source: &'static str,
    /// The page the text was taken from. `None` for a text written for the catalogue.
    pub href: Option<SafeHref>,
}

impl<'a> DescriptionView<'a> {
    /// The description of `entry`.
    pub(crate) fn of(entry: &'a Entry) -> Self {
        let description = &entry.description;
        let source = match description.source {
            DescriptionSource::Readme => "the project README",
            DescriptionSource::Crate => "the crates.io description",
            DescriptionSource::Human => "written for the catalogue",
            DescriptionSource::Repo => "the GitHub repository description",
        };
        Self {
            text: &description.text,
            source,
            href: description.source_url.as_ref().map(SafeHref::from),
        }
    }
}

/// One outside link of an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinkRow {
    /// What the link points to, such as `Repository`.
    pub label: &'static str,
    /// The host and path of the target, with no scheme.
    pub text: String,
    /// The link target.
    pub href: SafeHref,
}

impl LinkRow {
    /// The row `label` linking `url`.
    fn new(label: &'static str, url: &Url) -> Self {
        Self {
            label,
            text: target_text(url),
            href: SafeHref::from(url),
        }
    }
}

/// One crate of an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CrateRow {
    /// The crate name.
    pub name: String,
    /// The crates.io page.
    pub href: SafeHref,
    /// Whether it is the primary crate: the first the entry lists.
    pub primary: bool,
}

/// Each crate of `entry` with its crates.io page.
pub(crate) fn crate_rows(entry: &Entry) -> Vec<CrateRow> {
    entry
        .crates
        .iter()
        .enumerate()
        .filter_map(|(at, name)| {
            let url = crate_page(name.as_str())?;
            Some(CrateRow {
                name: name.to_string(),
                href: SafeHref::from(&url),
                primary: at == 0,
            })
        })
        .collect()
}

/// The text of a link: its host without `www.` and its path without the last `/`, or the whole
/// link when it has no host.
fn target_text(url: &Url) -> String {
    let Some(host) = url.host_str().filter(|host| !host.is_empty()) else {
        return url.as_str().to_owned();
    };
    let host = host.strip_prefix("www.").unwrap_or(host);
    format!("{host}{}", url.path().trim_end_matches('/'))
}

/// The docs, the repository, the homepage, the crates.io page of the primary crate, and each
/// install channel page.
pub(crate) fn link_rows(entry: &Entry) -> Vec<LinkRow> {
    let mut rows = Vec::new();
    if let Some(url) = docs_href(entry).and_then(|href| Url::parse(href.as_str()).ok()) {
        rows.push(LinkRow::new("Documentation", &url));
    }
    rows.push(LinkRow::new("Repository", &entry.repo));
    if let Some(homepage) = &entry.homepage {
        rows.push(LinkRow::new("Homepage", homepage));
    }
    if let Some(name) = entry.crates.first()
        && let Some(url) = crate_page(name.as_str())
    {
        rows.push(LinkRow::new("crates.io", &url));
    }
    let Some(distribution) = &entry.distribution else {
        return rows;
    };
    if let Some(formula) = distribution.homebrew.as_deref()
        && let Some(url) = homebrew_page(formula)
    {
        rows.push(LinkRow::new("Homebrew", &url));
    }
    for image in &distribution.docker {
        if let Some(url) = docker_page(image) {
            rows.push(LinkRow::new("Docker Hub", &url));
        }
    }
    if let Some(project) = distribution.repology.as_deref()
        && let Some(url) = repology_page(project)
    {
        rows.push(LinkRow::new("Repology", &url));
    }
    rows
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Date, Description, EntryKind, Slug};

    use super::*;

    fn text_of(raw: &str) -> String {
        target_text(&Url::parse(raw).expect("url"))
    }

    #[test]
    fn a_link_shows_its_host_and_path_with_no_scheme() {
        assert_eq!(text_of("https://qdrant.tech/"), "qdrant.tech");
        assert_eq!(
            text_of("https://www.qdrant.tech/docs?x=1"),
            "qdrant.tech/docs"
        );
        assert_eq!(text_of("http://example.org:8080/a/"), "example.org/a");
        assert_eq!(
            text_of("https://crates.io/crates/numr"),
            "crates.io/crates/numr"
        );
    }

    #[test]
    fn a_link_with_no_host_keeps_its_text() {
        assert_eq!(text_of("mailto:a@b.c"), "mailto:a@b.c");
    }

    #[test]
    fn a_repo_description_names_its_source_and_links_the_repository() {
        let repo = Url::parse("https://github.com/o/tool").expect("repo");
        let entry = Entry {
            name: "tool".to_owned(),
            kind: EntryKind::App,
            repo: repo.clone(),
            homepage: None,
            crates: Vec::new(),
            topics: vec![Slug::new("topic").expect("slug")],
            category: None,
            part_of: None,
            description: Description {
                text: "A tool.".to_owned(),
                source: DescriptionSource::Repo,
                source_url: Some(repo.clone()),
            },
            org_affiliated: false,
            distribution: None,
            added: Date::new(2026, 11, 3).expect("date"),
        };
        let view = DescriptionView::of(&entry);
        assert_eq!(view.source, "the GitHub repository description");
        assert_eq!(view.href, Some(SafeHref::from(&repo)));
    }
}
