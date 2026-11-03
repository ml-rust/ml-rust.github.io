//! One search record per entry.

use catalogue_core::{Catalogue, EffectiveEntry, TierReport};
use oxidoc_search::records::NAME_FIELD;
use oxidoc_search::{Field, Record};

use super::aliases::alias_terms;
use super::boost::tier_boost;
use crate::pages::{project_href, topic_link};

/// The field of the effective topic names.
const TOPICS_FIELD: &str = "topics";
/// The field of the description text.
const DESCRIPTION_FIELD: &str = "description";
/// The field of the synonym terms the other fields gain.
const ALIASES_FIELD: &str = "aliases";

/// The weight of the name, slug and crate names.
const NAME_WEIGHT: f32 = 3.0;
/// The weight of the effective topic names.
const TOPICS_WEIGHT: f32 = 2.0;
/// The weight of the description.
const DESCRIPTION_WEIGHT: f32 = 1.5;
/// The weight of the synonym terms.
const ALIASES_WEIGHT: f32 = 1.0;

/// The `org` tag of an entry with `org_affiliated = true`.
const ORG_AFFILIATED: &str = "ml-rust";
/// The `org` tag of every other entry.
const ORG_COMMUNITY: &str = "community";

/// One search record per entry of `catalogue`, in slug order.
///
/// A record links the entry page and shows the description as its snippet.
/// Its fields, in order:
/// - `name`, weight 3: the name, the slug and the crate names.
/// - `topics`, weight 2: the names of the effective topics.
/// - `description`, weight 1.5: the description text.
/// - `aliases`, weight 1: the synonym terms the fields above gain from
///   [`Catalogue::synonyms`]. Left out when there are none.
///
/// Tags: `kind`, `org` (`ml-rust` or `community`), one `topic` per effective
/// topic id, and `tier` when `report` is given. The boost comes from the tier
/// through [`super::TIER_BOOSTS`]. Without `report` there is no `tier` tag and
/// the boost is [`super::NO_TIER_BOOST`].
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
    let description = entry.description.text.as_str();
    let aliases = alias_terms(
        catalogue.synonyms(),
        &[&name_text, &topics_text, description],
    );

    record.fields = vec![
        weighted(NAME_FIELD, name_text, NAME_WEIGHT),
        weighted(TOPICS_FIELD, topics_text, TOPICS_WEIGHT),
        weighted(
            DESCRIPTION_FIELD,
            description.to_owned(),
            DESCRIPTION_WEIGHT,
        ),
    ];
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
    record.tags = vec![tag("kind", entry.kind.as_str()), tag("org", org)];
    record.tags.extend(
        effective
            .topics
            .iter()
            .map(|topic| tag("topic", topic.as_str())),
    );
    let tier = report
        .and_then(|report| report.entries.get(effective.slug))
        .map(|found| found.tier);
    if let Some(tier) = tier {
        record.tags.push(tag("tier", tier.as_str()));
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
