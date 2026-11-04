//! One search record per entry.

use catalogue_core::{Catalogue, EffectiveEntry, LICENSE_SIGNAL, RUNS_ON_SIGNAL, TierReport};
use oxidoc_search::records::NAME_FIELD;
use oxidoc_search::{Field, Record};

use super::aliases::alias_terms;
use super::boost::tier_boost;
use super::choice_tags::{LICENSE_TAG, RUNS_ON_TAG, license_value, runs_on_values};
use crate::pages::{category_link, project_href, topic_link};

/// The field of the effective topic names.
const TOPICS_FIELD: &str = "topics";
/// The field of the category name.
const CATEGORY_FIELD: &str = "category";
/// The field of the description text.
const DESCRIPTION_FIELD: &str = "description";
/// The field of the synonym terms the other fields gain.
const ALIASES_FIELD: &str = "aliases";

/// The weight of the name, slug and crate names.
const NAME_WEIGHT: f32 = 3.0;
/// The weight of the effective topic names.
const TOPICS_WEIGHT: f32 = 2.0;
/// The weight of the category name.
const CATEGORY_WEIGHT: f32 = 2.0;
/// The weight of the description.
const DESCRIPTION_WEIGHT: f32 = 1.5;
/// The weight of the synonym terms.
const ALIASES_WEIGHT: f32 = 1.0;

/// The tag of the entry kind.
pub(crate) const KIND_TAG: &str = "kind";
/// The tag of the organization: [`ORG_AFFILIATED`] or [`ORG_COMMUNITY`].
pub(crate) const ORG_TAG: &str = "org";
/// The tag of one effective topic id.
pub(crate) const TOPIC_TAG: &str = "topic";
/// The tag of the category id. Present only on an entry with a category.
pub(crate) const CATEGORY_TAG: &str = "category";
/// The tag of the tier. Present only with a tier report.
pub(crate) const TIER_TAG: &str = "tier";

/// The `org` tag of an entry with `org_affiliated = true`.
pub(crate) const ORG_AFFILIATED: &str = "ml-rust";
/// The `org` tag of every other entry.
pub(crate) const ORG_COMMUNITY: &str = "community";

/// One search record per entry of `catalogue`, in slug order.
///
/// A record links the entry page and shows the description as its snippet.
/// Its fields, in order:
/// - `name`, weight 3: the name, the slug and the crate names.
/// - `topics`, weight 2: the names of the effective topics.
/// - `category`, weight 2: the name of the category. Left out without a category.
/// - `description`, weight 1.5: the description text.
/// - `aliases`, weight 1: the synonym terms the fields above gain from
///   [`Catalogue::synonyms`]. Left out when there are none.
///
/// Tags: `kind`, `org` (`ml-rust` or `community`), one `topic` per effective
/// topic id, `category` when the entry has one, and `license`, one `runs_on` per
/// backend and `tier` when `report` is given. An unknown license or backend tags
/// as `not-measured`. The boost comes from the tier through [`super::TIER_BOOSTS`].
/// Without `report` none of those three tags exists and the boost is
/// [`super::NO_TIER_BOOST`].
pub fn search_records(catalogue: &Catalogue, report: Option<&TierReport>) -> Vec<Record> {
    catalogue
        .entries()
        .keys()
        .filter_map(|slug| catalogue.effective(slug))
        .map(|effective| record(catalogue, report, &effective))
        .collect()
}

/// The search record of `effective`.
fn record(
    catalogue: &Catalogue,
    report: Option<&TierReport>,
    effective: &EffectiveEntry<'_>,
) -> Record {
    let entry = effective.entry;
    let slug = effective.slug.as_str();
    let mut record = Record::new(slug, project_href(effective.slug).as_str(), &entry.name);

    let mut names: Vec<&str> = Vec::new();
    let crates = entry.crates.iter().map(|name| name.as_str());
    for name in [entry.name.as_str(), slug].into_iter().chain(crates) {
        if !names.iter().any(|seen| seen.eq_ignore_ascii_case(name)) {
            names.push(name);
        }
    }
    let topics: Vec<&str> = effective
        .topics
        .iter()
        .filter_map(|id| topic_link(catalogue, id))
        .map(|link| link.name)
        .collect();
    let name_text = names.join(" ");
    let topics_text = topics.join(", ");
    let category = entry
        .category
        .as_ref()
        .and_then(|id| Some((id, category_link(catalogue, id)?.name)));
    let category_name = category.map_or("", |(_, name)| name);
    let description = entry.description.text.as_str();
    let aliases = alias_terms(
        catalogue.synonyms(),
        &[&name_text, &topics_text, category_name, description],
    );

    record.fields = vec![
        weighted(NAME_FIELD, name_text, NAME_WEIGHT),
        weighted(TOPICS_FIELD, topics_text, TOPICS_WEIGHT),
    ];
    if category.is_some() {
        record.fields.push(weighted(
            CATEGORY_FIELD,
            category_name.to_owned(),
            CATEGORY_WEIGHT,
        ));
    }
    record.fields.push(weighted(
        DESCRIPTION_FIELD,
        description.to_owned(),
        DESCRIPTION_WEIGHT,
    ));
    if !aliases.is_empty() {
        record
            .fields
            .push(weighted(ALIASES_FIELD, aliases.join(", "), ALIASES_WEIGHT));
    }

    let org = if entry.org_affiliated {
        ORG_AFFILIATED
    } else {
        ORG_COMMUNITY
    };
    record.tags = vec![tag(KIND_TAG, entry.kind.as_str()), tag(ORG_TAG, org)];
    record.tags.extend(
        effective
            .topics
            .iter()
            .map(|topic| tag(TOPIC_TAG, topic.as_str())),
    );
    if let Some((id, _)) = category {
        record.tags.push(tag(CATEGORY_TAG, id.as_str()));
    }
    let found = report.and_then(|report| report.entries.get(effective.slug));
    if report.is_some() {
        let license = found.and_then(|found| found.value_of(LICENSE_SIGNAL));
        record.tags.push(tag(LICENSE_TAG, &license_value(license)));
        let runs_on = found.and_then(|found| found.value_of(RUNS_ON_SIGNAL));
        for id in runs_on_values(runs_on, catalogue.backends()) {
            record.tags.push(tag(RUNS_ON_TAG, id));
        }
    }
    let tier = found.map(|found| found.tier);
    if let Some(tier) = tier {
        record.tags.push(tag(TIER_TAG, tier.as_str()));
    }
    record.boost = tier_boost(tier);
    record.snippet = Some(description.to_owned());
    record
}

/// The field `name` holding `text`, with `weight`.
fn weighted(name: &str, text: String, weight: f32) -> Field {
    Field {
        weight,
        ..Field::new(name, text)
    }
}

/// The tag `name` with `value`.
fn tag(name: &str, value: &str) -> (String, String) {
    (name.to_owned(), value.to_owned())
}
