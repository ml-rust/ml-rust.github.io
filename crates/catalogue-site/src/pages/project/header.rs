//! The top of an entry page: name, kind, tier, and the notices about the entry.

use catalogue_core::{Catalogue, EffectiveEntry, EntryReport};

use crate::pages::shared::{
    NamedLink, TierBadge, entry_link, kind_name, project_kind_name, topic_link,
};

/// The data the top of `project.html` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Header<'a> {
    /// The entry name.
    pub name: &'a str,
    /// The entry kind.
    pub kind: &'static str,
    /// The product or demo label of the newest `kind` judgment.
    pub label: Option<&'static str>,
    /// The tier badge. `None` without a report.
    pub badge: Option<TierBadge>,
    /// Whether the report finds the entry finished. False without a report.
    pub stable: bool,
    /// Whether the ml-rust organization maintains the entry.
    pub org_affiliated: bool,
    /// The entry that replaces this one.
    pub superseded_by: Option<NamedLink<'a>>,
    /// The entries this one replaces.
    pub replaces: Vec<NamedLink<'a>>,
    /// The effective topics of the entry.
    pub topics: Vec<NamedLink<'a>>,
}

impl<'a> Header<'a> {
    /// The header of `effective`, with the tier of `record` when given.
    pub(crate) fn new(
        catalogue: &'a Catalogue,
        effective: &EffectiveEntry<'a>,
        record: Option<&EntryReport>,
    ) -> Self {
        let entry = effective.entry;
        Self {
            name: &entry.name,
            kind: kind_name(entry.kind),
            label: effective.kind_label.map(project_kind_name),
            badge: record.map(|record| TierBadge::of(record.tier)),
            stable: record.is_some_and(|record| record.stable),
            org_affiliated: entry.org_affiliated,
            superseded_by: effective
                .superseded_by
                .and_then(|slug| entry_link(catalogue, slug)),
            replaces: effective
                .supersedes
                .iter()
                .filter_map(|&slug| entry_link(catalogue, slug))
                .collect(),
            topics: effective
                .topics
                .iter()
                .filter_map(|id| topic_link(catalogue, id))
                .collect(),
        }
    }
}
