//! The action panel of an entry page: how to get the project and where to read about it.

use catalogue_core::Entry;

use super::links::{LinkRow, link_rows};
use crate::href::SafeHref;
use crate::pages::shared::{InstallLine, docs_href, install_line};

/// A link that prints as a button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActionButton {
    /// The link text.
    pub label: &'static str,
    /// The link target.
    pub href: SafeHref,
}

/// The data of the action panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActionPanel<'a> {
    /// The project name.
    pub name: &'a str,
    /// Whether the entry is Archived. The panel then steps back.
    pub archived: bool,
    /// How to get the project.
    pub install: InstallLine,
    /// The main link: the docs of a library, else the homepage, else the repository.
    pub primary: ActionButton,
    /// The repository, when the main link is something else.
    pub repository: Option<ActionButton>,
    /// The homepage, the crates.io page and each install channel.
    pub links: Vec<LinkRow>,
}

impl<'a> ActionPanel<'a> {
    /// The panel of `entry`.
    pub(crate) fn new(entry: &'a Entry, archived: bool) -> Self {
        let repo = ActionButton {
            label: "Repository",
            href: SafeHref::from(&entry.repo),
        };
        let docs = docs_href(entry).map(|href| ActionButton {
            label: "Documentation",
            href,
        });
        let (primary, repository) = match docs {
            Some(docs) => (docs, Some(repo)),
            None => (repo, None),
        };
        Self {
            name: &entry.name,
            archived,
            install: install_line(entry),
            primary,
            repository,
            links: link_rows(entry),
        }
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::{CrateName, Date, Description, DescriptionSource, EntryKind, Slug};
    use url::Url;

    use super::*;

    fn entry(kind: EntryKind, crates: &[&str]) -> Entry {
        Entry {
            name: "tool".to_owned(),
            kind,
            repo: Url::parse("https://github.com/o/tool").expect("repo"),
            homepage: None,
            crates: crates
                .iter()
                .map(|name| CrateName::new(*name).expect("crate name"))
                .collect(),
            topics: vec![Slug::new("topic").expect("slug")],
            category: None,
            part_of: None,
            description: Description {
                text: "A tool.".to_owned(),
                source: DescriptionSource::Human,
                source_url: None,
            },
            org_affiliated: false,
            distribution: None,
            added: Date::new(2026, 11, 3).expect("date"),
        }
    }

    #[test]
    fn a_library_leads_with_the_docs_of_its_primary_crate() {
        let entry = entry(EntryKind::Library, &["first", "second"]);
        let panel = ActionPanel::new(&entry, false);
        assert_eq!(panel.primary.label, "Documentation");
        assert_eq!(panel.primary.href.as_str(), "https://docs.rs/first");
        assert_eq!(
            panel.repository.expect("repository").href.as_str(),
            "https://github.com/o/tool"
        );
        assert_eq!(panel.install.command.as_deref(), Some("cargo add first"));
    }

    #[test]
    fn an_app_claims_no_docs_link_and_leads_with_its_repository() {
        let entry = entry(EntryKind::App, &[]);
        let panel = ActionPanel::new(&entry, false);
        assert_eq!(panel.primary.label, "Repository");
        assert_eq!(panel.repository, None);
        assert_eq!(panel.install.command, None);
    }
}
