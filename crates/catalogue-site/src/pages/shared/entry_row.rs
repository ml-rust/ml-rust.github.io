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
        }
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

/// Sorts `rows` by name, ignoring case, then by link.
pub(crate) fn sort_by_name(rows: &mut [EntryRow<'_>]) {
    rows.sort_by_cached_key(|row| (row.name.to_lowercase(), row.href.as_str().to_owned()));
}
