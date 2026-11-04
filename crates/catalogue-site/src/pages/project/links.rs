//! The description of an entry with its source, and the entry's outside links.

use catalogue_core::{
    DescriptionSource, Entry, crate_page, docker_page, homebrew_page, repology_page,
};
use url::Url;

use crate::href::SafeHref;

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
    /// The link text.
    pub text: String,
    /// The link target.
    pub href: SafeHref,
}

impl LinkRow {
    /// The row `label` linking `url` with text `text`.
    fn new(label: &'static str, text: &str, url: &Url) -> Self {
        Self {
            label,
            text: text.to_owned(),
            href: SafeHref::from(url),
        }
    }
}

/// The repo, the homepage, the crates.io page of each crate, and each install channel page.
pub(crate) fn link_rows(entry: &Entry) -> Vec<LinkRow> {
    let mut rows = vec![LinkRow::new("Repository", entry.repo.as_str(), &entry.repo)];
    if let Some(homepage) = &entry.homepage {
        rows.push(LinkRow::new("Homepage", homepage.as_str(), homepage));
    }
    for name in &entry.crates {
        if let Some(url) = crate_page(name.as_str()) {
            rows.push(LinkRow::new("crates.io", name.as_str(), &url));
        }
    }
    let Some(distribution) = &entry.distribution else {
        return rows;
    };
    if let Some(formula) = distribution.homebrew.as_deref()
        && let Some(url) = homebrew_page(formula)
    {
        rows.push(LinkRow::new("Homebrew", formula, &url));
    }
    for image in &distribution.docker {
        if let Some(url) = docker_page(image) {
            rows.push(LinkRow::new("Docker Hub", image, &url));
        }
    }
    if let Some(project) = distribution.repology.as_deref()
        && let Some(url) = repology_page(project)
    {
        rows.push(LinkRow::new("Repology", project, &url));
    }
    rows
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Date, Description, EntryKind, Slug};

    use super::*;

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
