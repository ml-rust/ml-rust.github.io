//! The links card of an entry page: where to read about the project and how to get it.

use catalogue_core::Entry;

use super::links::{LinkRow, link_rows};
use crate::pages::shared::{InstallLine, install_line};

/// The data of the links card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActionPanel {
    /// Whether the entry is Archived. The card then steps back.
    pub archived: bool,
    /// How to get the project.
    pub install: InstallLine,
    /// The docs, the repository, the homepage, the crates.io page and each install channel.
    pub links: Vec<LinkRow>,
}

impl ActionPanel {
    /// The card of `entry`.
    pub(crate) fn new(entry: &Entry, archived: bool) -> Self {
        Self {
            archived,
            install: install_line(entry),
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

    fn labels(panel: &ActionPanel) -> Vec<&'static str> {
        panel.links.iter().map(|link| link.label).collect()
    }

    #[test]
    fn a_library_leads_with_the_docs_of_its_primary_crate() {
        let entry = entry(EntryKind::Library, &["first", "second"]);
        let panel = ActionPanel::new(&entry, false);
        assert_eq!(labels(&panel), ["Documentation", "Repository", "crates.io"]);
        assert_eq!(panel.links[0].href.as_str(), "https://docs.rs/first");
        assert_eq!(panel.links[0].text, "docs.rs/first");
        assert_eq!(panel.links[1].href.as_str(), "https://github.com/o/tool");
        assert_eq!(panel.links[1].text, "github.com/o/tool");
        assert_eq!(panel.install.command.as_deref(), Some("cargo add first"));
    }

    #[test]
    fn an_app_claims_no_docs_link_and_leads_with_its_repository() {
        let entry = entry(EntryKind::App, &[]);
        let panel = ActionPanel::new(&entry, false);
        assert_eq!(labels(&panel), ["Repository"]);
        assert_eq!(panel.install.command, None);
    }
}
