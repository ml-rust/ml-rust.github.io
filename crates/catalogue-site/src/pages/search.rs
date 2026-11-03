//! `/search/`: the search form, its filters, and the results the script fills in.

use askama::Template;
use catalogue_core::{Catalogue, EntryKind, Tier, TierReport};

use super::layout::Layout;
use super::shared::{NamedLink, kind_name, tier_name, topic_href};
use crate::href::SafeHref;
use crate::search::{KIND_TAG, ORG_AFFILIATED, ORG_COMMUNITY, ORG_TAG, TIER_TAG, TOPIC_TAG};

/// The label of the [`ORG_AFFILIATED`] filter.
const ORG_AFFILIATED_LABEL: &str = "ml-rust organization";
/// The label of the [`ORG_COMMUNITY`] filter.
const ORG_COMMUNITY_LABEL: &str = "Community";

/// The search page.
#[derive(Debug, Template)]
#[template(path = "search.html")]
pub(crate) struct SearchPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// The link to the search script.
    pub script: SafeHref,
    /// The filter groups, in display order.
    pub groups: Vec<FilterGroup<'a>>,
    /// Every topic, in file order, for readers without scripts.
    pub topics: Vec<NamedLink<'a>>,
}

/// One set of checkboxes over the values of one search tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilterGroup<'a> {
    /// The tag name. Every checkbox of the group submits under it.
    pub key: &'static str,
    /// The group heading.
    pub legend: &'static str,
    /// One checkbox per tag value.
    pub options: Vec<FilterOption<'a>>,
}

/// One checkbox of a [`FilterGroup`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilterOption<'a> {
    /// The tag value.
    pub value: &'a str,
    /// The text beside the checkbox.
    pub label: &'a str,
    /// The element id of the checkbox: `filter-<key>-<value>`.
    pub id: String,
}

impl<'a> SearchPage<'a> {
    /// The search page in `layout`, loading the script at `script`.
    ///
    /// The groups are kind, topic, tier and organization. The tier group is
    /// left out without `report`.
    pub(crate) fn new(
        layout: Layout,
        script: SafeHref,
        catalogue: &'a Catalogue,
        report: Option<&TierReport>,
    ) -> Self {
        let kinds = EntryKind::ALL
            .into_iter()
            .map(|kind| (kind.as_str(), kind_name(kind)));
        let topics = catalogue
            .topics()
            .iter()
            .map(|topic| (topic.id.as_str(), topic.name.as_str()));
        let tiers = Tier::ALL
            .into_iter()
            .map(|tier| (tier.as_str(), tier_name(tier)));
        let orgs = [
            (ORG_AFFILIATED, ORG_AFFILIATED_LABEL),
            (ORG_COMMUNITY, ORG_COMMUNITY_LABEL),
        ];
        let mut groups = vec![
            group(KIND_TAG, "Kind", kinds),
            group(TOPIC_TAG, "Topic", topics),
        ];
        if report.is_some() {
            groups.push(group(TIER_TAG, "Tier", tiers));
        }
        groups.push(group(ORG_TAG, "Organization", orgs));
        let topics = catalogue
            .topics()
            .iter()
            .map(|topic| NamedLink {
                name: &topic.name,
                href: topic_href(&topic.id),
            })
            .collect();
        Self {
            layout,
            script,
            groups,
            topics,
        }
    }
}

/// The group `key` headed `legend`, with one checkbox per `(value, label)`.
fn group<'a>(
    key: &'static str,
    legend: &'static str,
    values: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> FilterGroup<'a> {
    FilterGroup {
        key,
        legend,
        options: values
            .into_iter()
            .map(|(value, label)| FilterOption {
                value,
                label,
                id: format!("filter-{key}-{value}"),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_names_each_checkbox_by_key_and_value() {
        let found = group("kind", "Kind", [("library", "Library"), ("app", "App")]);
        let ids: Vec<&str> = found
            .options
            .iter()
            .map(|option| option.id.as_str())
            .collect();
        assert_eq!(ids, ["filter-kind-library", "filter-kind-app"]);
        assert_eq!(found.options[1].label, "App");
    }
}
