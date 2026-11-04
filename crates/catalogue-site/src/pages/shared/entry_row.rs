//! One entry as a list row: name link, tier badge, description, and the facts a person compares.

use catalogue_core::{Entry, EntryKind, EntryReport, Slug, Tier, TierReport};

use super::badge::TierBadge;
use super::choice::ChoiceText;
use super::install::{InstallLine, install_line};
use super::labels::kind_name;
use super::paths::{NamedLink, project_href};
use crate::href::SafeHref;

/// The tag of an entry the dormancy rule finds dormant.
pub(crate) const DORMANT: &str = "Dormant";
/// What [`DORMANT`] means, in the words of the rules page.
pub(crate) const DORMANT_MEANING: &str =
    "The days since its latest release or latest human commit reach the interval the rules expect.";
/// The tag of a finished entry.
pub(crate) const STABLE: &str = "Stable";
/// What [`STABLE`] means, in the words of the entry page.
pub(crate) const STABLE_MEANING: &str = "The project is finished: dormant by design, not decayed.";

/// The data `entry_row.html` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryRow<'a> {
    /// The entry slug, for the row's data attribute.
    pub slug: String,
    /// How to get the project.
    pub install: Option<InstallLine>,
    /// The entry name.
    pub name: &'a str,
    /// The link to the entry page.
    pub href: SafeHref,
    /// The entry kind.
    pub kind: &'static str,
    /// The kind id, for the row's data attribute.
    pub kind_id: &'static str,
    /// The entry kind, which picks the download signal of the row.
    entry_kind: EntryKind,
    /// The tier badge. `None` without a report.
    pub badge: Option<TierBadge>,
    /// The tier id, for the row's data attribute. `None` without a report.
    pub tier_id: Option<&'static str>,
    /// The description text.
    pub description: &'a str,
    /// The facts a person reads to choose, for lists that compare projects.
    pub choice: Option<ChoiceText>,
    /// The state words that say something about this row.
    pub statuses: Vec<RowStatus>,
    /// The plain word for how the entry moved.
    pub movement: Option<&'static str>,
    /// The link that stands for the related projects this row hides.
    pub related: Option<RelatedLink>,
    /// The first shared topic, on a row that compares projects from an entry page.
    pub also_in: Option<NamedLink<'a>>,
    /// Whether the row is the project of the page that shows it.
    pub current: bool,
}

/// One state word on a row, with its meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RowStatus {
    /// The word the row prints.
    pub word: &'static str,
    /// What the word means.
    pub meaning: &'static str,
}

/// The link from a family row to the parent page, which lists every related project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelatedLink {
    /// `and N related projects`.
    pub text: String,
    /// The link to the parent page.
    pub href: SafeHref,
    /// How many related projects the link stands for.
    pub count: usize,
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
            count,
        })
    }
}

impl<'a> EntryRow<'a> {
    /// The row of entry `slug`, with a badge for `tier` when given.
    pub(crate) fn new(slug: &Slug, entry: &'a Entry, tier: Option<Tier>) -> Self {
        Self {
            slug: slug.as_str().to_owned(),
            install: None,
            name: &entry.name,
            href: project_href(slug),
            kind: kind_name(entry.kind),
            kind_id: entry.kind.as_str(),
            entry_kind: entry.kind,
            badge: tier.map(TierBadge::of),
            tier_id: tier.map(Tier::as_str),
            description: &entry.description.text,
            choice: None,
            statuses: Vec::new(),
            movement: None,
            related: None,
            also_in: None,
            current: false,
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
        self.choice = Some(ChoiceText::of(self.entry_kind, record));
        self.statuses = record.map(row_statuses).unwrap_or_default();
        self
    }

    /// The row with how to get the project.
    pub(crate) fn with_install(mut self, entry: &Entry) -> Self {
        self.install = Some(install_line(entry));
        self
    }

    /// The row with the topic it shares with the project of the page.
    pub(crate) fn with_also_in(mut self, topic: NamedLink<'a>) -> Self {
        self.also_in = Some(topic);
        self
    }

    /// The row marked as the project of the page that shows it.
    pub(crate) fn marked_current(mut self) -> Self {
        self.current = true;
        self
    }

    /// How many projects the row stands for: its own and every related one.
    pub(crate) fn project_count(&self) -> usize {
        1 + self.related.as_ref().map_or(0, |related| related.count)
    }
}

/// The state words of `record`. The Archived tier already says more than the word dormant.
pub(crate) fn row_statuses(record: &EntryReport) -> Vec<RowStatus> {
    if record.stable {
        return vec![RowStatus {
            word: STABLE,
            meaning: STABLE_MEANING,
        }];
    }
    if record.dormant == Some(true) && record.tier != Tier::Archived {
        return vec![RowStatus {
            word: DORMANT,
            meaning: DORMANT_MEANING,
        }];
    }
    Vec::new()
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
    use crate::testing::record;

    #[test]
    fn the_related_text_counts_the_projects() {
        let slug = Slug::new("core").expect("slug");
        assert_eq!(RelatedLink::of(&slug, 0), None);
        let one = RelatedLink::of(&slug, 1).expect("link");
        assert_eq!(one.text, "and 1 related project");
        assert_eq!(one.href.as_str(), "/projects/core/");
        let two = RelatedLink::of(&slug, 2).expect("link");
        assert_eq!(two.text, "and 2 related projects");
        assert_eq!(two.count, 2);
    }

    #[test]
    fn a_status_prints_only_when_it_says_something() {
        let plain = record(Tier::Listed, None);
        assert!(row_statuses(&plain).is_empty());

        let mut dormant = record(Tier::Listed, None);
        dormant.dormant = Some(true);
        assert_eq!(row_statuses(&dormant)[0].word, DORMANT);

        let mut archived = record(Tier::Archived, None);
        archived.dormant = Some(true);
        assert!(row_statuses(&archived).is_empty());

        let mut stable = record(Tier::Listed, None);
        stable.stable = true;
        stable.dormant = Some(true);
        assert_eq!(row_statuses(&stable)[0].word, STABLE);
    }
}
