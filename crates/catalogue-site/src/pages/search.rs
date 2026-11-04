//! `/search/`: the search form, its filters, and the results the script fills in.

use std::borrow::Cow;
use std::collections::BTreeSet;

use askama::Template;
use catalogue_core::{
    Catalogue, EntryKind, LICENSE_SIGNAL, NO_GPU_FEATURE, NO_GPU_FEATURE_ID, NONE_STATED,
    NOT_MEASURED, NOT_MEASURED_ID, Tier, TierReport,
};

use super::layout::Layout;
use super::shared::{
    ExampleSearch, TopicCard, example_searches, kind_name, tier_name, topic_cards,
};
use crate::href::SafeHref;
use crate::search::{
    CATEGORY_TAG, KIND_TAG, LICENSE_TAG, ORG_AFFILIATED, ORG_COMMUNITY, ORG_TAG, RUNS_ON_TAG,
    TIER_TAG, TOPIC_TAG, is_stated_license, license_value,
};

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
    /// One tile per top-level topic, for the empty and no-result states.
    pub topics: Vec<TopicCard<'a>>,
    /// The example searches.
    pub examples: Vec<ExampleSearch>,
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
    pub value: Cow<'a, str>,
    /// The text beside the checkbox.
    pub label: Cow<'a, str>,
    /// The element id of the checkbox: `filter-<key>-<value>`.
    pub id: String,
}

impl<'a> SearchPage<'a> {
    /// The search page in `layout`, loading the script at `script`.
    ///
    /// The groups are kind, topic, category, license, runs on, tier and organization.
    /// The license, runs-on and tier groups are left out without `report`.
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
        let categories = catalogue
            .categories()
            .iter()
            .map(|category| (category.id.as_str(), category.name.as_str()));
        let tiers = Tier::ALL
            .into_iter()
            .map(|tier| (tier.as_str(), tier_name(tier)));
        let orgs = [
            (ORG_AFFILIATED, ORG_AFFILIATED_LABEL),
            (ORG_COMMUNITY, ORG_COMMUNITY_LABEL),
        ];
        let mut groups = vec![
            group(KIND_TAG, "Kind", kinds),
            group(TOPIC_TAG, "Job", topics),
            group(CATEGORY_TAG, "Category", categories),
        ];
        if let Some(report) = report {
            groups.push(group(LICENSE_TAG, "License", license_options(report)));
            groups.push(group(RUNS_ON_TAG, "Runs on", runs_on_options(catalogue)));
            groups.push(group(TIER_TAG, "Tier", tiers));
        }
        groups.push(group(ORG_TAG, "Organization", orgs));
        Self {
            layout,
            script,
            groups,
            topics: topic_cards(catalogue, report),
            examples: example_searches(),
        }
    }
}

/// Every license the report states, by name, then none stated and not measured.
fn license_options(report: &TierReport) -> Vec<(String, String)> {
    let stated: BTreeSet<&str> = report
        .entries
        .values()
        .filter_map(|record| record.value_of(LICENSE_SIGNAL))
        .filter(|text| is_stated_license(text))
        .collect();
    stated
        .into_iter()
        .chain([NONE_STATED, NOT_MEASURED])
        .map(|text| (license_value(Some(text)), text.to_owned()))
        .collect()
}

/// Every backend of the table in file order, then no GPU feature found and not measured.
fn runs_on_options(catalogue: &Catalogue) -> Vec<(&str, &str)> {
    catalogue
        .backends()
        .backend
        .iter()
        .map(|row| (row.id.as_str(), row.name.as_str()))
        .chain([
            (NO_GPU_FEATURE_ID, NO_GPU_FEATURE),
            (NOT_MEASURED_ID, NOT_MEASURED),
        ])
        .collect()
}

/// The group `key` headed `legend`, with one checkbox per `(value, label)`.
fn group<'a, V, L>(
    key: &'static str,
    legend: &'static str,
    values: impl IntoIterator<Item = (V, L)>,
) -> FilterGroup<'a>
where
    V: Into<Cow<'a, str>>,
    L: Into<Cow<'a, str>>,
{
    FilterGroup {
        key,
        legend,
        options: values
            .into_iter()
            .map(|(value, label)| {
                let value = value.into();
                FilterOption {
                    id: format!("filter-{key}-{value}"),
                    value,
                    label: label.into(),
                }
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
