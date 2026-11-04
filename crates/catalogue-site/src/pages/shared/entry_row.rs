//! One entry as a list row: name link, kind, tier badge and description.

use catalogue_core::{Entry, EntryReport, Slug, Tier, TierReport};

use super::badge::TierBadge;
use super::choice::ChoiceText;
use super::install::{InstallLine, install_line};
use super::labels::kind_name;
use super::paths::project_href;
use crate::href::SafeHref;

/// The data `entry_row.html` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryRow<'a> {
    /// How to get the project, for lists that lead with it.
    pub install: Option<InstallLine>,
    /// The entry name.
    pub name: &'a str,
    /// The link to the entry page.
    pub href: SafeHref,
    /// The entry kind.
    pub kind: &'static str,
    /// The tier badge. `None` without a report.
    pub badge: Option<TierBadge>,
    /// The date the entry joined the catalogue, for lists that show it.
    pub added: Option<String>,
    /// The description text.
    pub description: &'a str,
    /// The facts a person reads to choose, for lists that compare projects.
    pub choice: Option<ChoiceText>,
    /// The plain word for how the entry moved.
    pub movement: Option<&'static str>,
    /// The link that stands for the related projects this row hides.
    pub related: Option<RelatedLink>,
}

/// The link from a family row to the parent page, which lists every related project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelatedLink {
    /// `and N related projects`.
    pub text: String,
    /// The link to the parent page.
    pub href: SafeHref,
}

impl RelatedLink {
    /// The link for `count` related projects of entry `slug`. `None` for zero.
    pub(crate) fn of(slug: &Slug, count: usize) -> Option<Self> {
        let text = match count {
            0 => return None,
            1 => "and 1 related project".to_owned(),
            more => format!("and {more} related projects"),
        };
        Some(Self {
            text,
            href: project_href(slug),
        })
    }
}

impl<'a> EntryRow<'a> {
    /// The row of entry `slug`, with a badge for `tier` when given.
    pub(crate) fn new(slug: &Slug, entry: &'a Entry, tier: Option<Tier>) -> Self {
        Self {
            install: None,
            name: &entry.name,
            href: project_href(slug),
            kind: kind_name(entry.kind),
            badge: tier.map(TierBadge::of),
            added: None,
            description: &entry.description.text,
            choice: None,
            movement: None,
            related: None,
        }
    }

    /// The row with how the entry moved and the link to its related projects.
    pub(crate) fn with_family(
        mut self,
        movement: Option<&'static str>,
        related: Option<RelatedLink>,
    ) -> Self {
        self.movement = movement;
        self.related = related;
        self
    }

    /// The row with the facts of `record`. Without a record, each fact reads as not measured.
    pub(crate) fn with_choice(mut self, record: Option<&EntryReport>) -> Self {
        self.choice = Some(ChoiceText::of(record));
        self
    }

    /// The row with how to get the project.
    pub(crate) fn with_install(mut self, entry: &Entry) -> Self {
        self.install = Some(install_line(entry));
        self
    }

    /// The row with the date the entry joined the catalogue.
    pub(crate) fn with_added(mut self, entry: &Entry) -> Self {
        self.added = Some(entry.added.to_string());
        self
    }
}

/// The tier of `slug` in `report`. `None` without a report or without a record.
pub(crate) fn tier_of(report: Option<&TierReport>, slug: &Slug) -> Option<Tier> {
    report
        .and_then(|report| report.entries.get(slug))
        .map(|record| record.tier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_related_text_counts_the_projects() {
        let slug = Slug::new("core").expect("slug");
        assert_eq!(RelatedLink::of(&slug, 0), None);
        let one = RelatedLink::of(&slug, 1).expect("link");
        assert_eq!(one.text, "and 1 related project");
        assert_eq!(one.href.as_str(), "/projects/core/");
        let two = RelatedLink::of(&slug, 2).expect("link");
        assert_eq!(two.text, "and 2 related projects");
    }
}
