//! One search record per entry.

use catalogue_core::{Catalogue, EffectiveEntry, LICENSE_SIGNAL, RUNS_ON_SIGNAL, TierReport};
use oxidoc_search::records::NAME_FIELD;
use oxidoc_search::{Field, Record};

use super::aliases::alias_terms;
use super::boost::tier_boost;
use super::choice_tags::{LICENSE_TAG, RUNS_ON_TAG, license_value, runs_on_values};
use super::row_tags::row_tags;
use crate::pages::{category_link, project_href, readme_search_text, topic_link};

/// The field of the effective topic names.
const TOPICS_FIELD: &str = "topics";
/// The field of the category name.
const CATEGORY_FIELD: &str = "category";
/// The field of the description text.
const DESCRIPTION_FIELD: &str = "description";
/// The field of the crate keywords, crate categories and repository topics.
const KEYWORDS_FIELD: &str = "keywords";
/// The field of the synonym terms the other fields gain.
const ALIASES_FIELD: &str = "aliases";
/// The field of the README prose.
const README_FIELD: &str = "readme";

/// The weight of the name, slug and crate names.
const NAME_WEIGHT: f32 = 3.0;
/// The weight of the effective topic names.
const TOPICS_WEIGHT: f32 = 2.0;
/// The weight of the category name.
const CATEGORY_WEIGHT: f32 = 2.0;
/// The weight of the description.
const DESCRIPTION_WEIGHT: f32 = 1.5;
/// The weight of the crate keywords, crate categories and repository topics.
///
/// A project picks its keywords to say what it is, so a keyword outranks a description that only
/// mentions the word. A word counts once per project, so without this a mention ties with a real
/// match, and the tie goes by slug.
const KEYWORDS_WEIGHT: f32 = 2.5;
/// The weight of the opening phrase of the description, which says what a project is.
///
/// Many projects list a keyword they only touch on, so this sits above the keywords and below
/// the name.
const LEAD_WEIGHT: f32 = 2.8;
/// The field of the opening phrase of the description.
const LEAD_FIELD: &str = "lead";
/// The most words of the opening phrase.
const MAX_LEAD_WORDS: usize = 10;
/// The weight of the synonym terms.
const ALIASES_WEIGHT: f32 = 1.0;
/// The weight of the README prose.
const README_WEIGHT: f32 = 0.5;

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

/// The tag of the description a result row prints. No filter reads it.
const ROW_DESCRIPTION_TAG: &str = "row_description";

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
/// - `keywords`, weight 2.5: the measured search terms of `report`, each with its words. Left out
///   when there are none, as in a build without `report`.
/// - `aliases`, weight 1: the synonym terms the fields above gain from
///   [`Catalogue::synonyms`]. Left out when there are none.
/// - `readme`, weight 0.5: the first prose of the README of `report`, with no code and no URLs.
///   Left out when there is none.
/// - `lead`, weight 2.8: the opening phrase of the description.
///
/// Tags: `kind`, `org` (`ml-rust` or `community`), one `topic` per effective
/// topic id, `category` when the entry has one, and `license`, one `runs_on` per
/// backend, `tier` and the `row_` tags the result rows print when `report` is given.
/// An unknown license or backend tags as `not-measured`. The boost comes from the tier through [`super::TIER_BOOSTS`].
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

/// The opening phrase of `description`: up to the first colon or sentence end, at most
/// [`MAX_LEAD_WORDS`] words.
fn lead_phrase(description: &str) -> String {
    let end = description
        .find(": ")
        .into_iter()
        .chain(description.find(". "))
        .min()
        .unwrap_or(description.len());
    let phrase = description.get(..end).unwrap_or(description);
    let words: Vec<&str> = phrase.split_whitespace().take(MAX_LEAD_WORDS).collect();
    words.join(" ")
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
    let found = report.and_then(|report| report.entries.get(effective.slug));
    let terms = found.map_or(&[][..], |found| found.search_terms.as_slice());
    // Each term is its own text, so a synonym never spans two terms.
    let mut texts = vec![
        name_text.as_str(),
        topics_text.as_str(),
        category_name,
        description,
    ];
    texts.extend(terms.iter().map(String::as_str));
    let aliases = alias_terms(catalogue.synonyms(), &texts);

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
    if !terms.is_empty() {
        record.fields.push(weighted(
            KEYWORDS_FIELD,
            keyword_text(terms),
            KEYWORDS_WEIGHT,
        ));
    }
    if !aliases.is_empty() {
        record
            .fields
            .push(weighted(ALIASES_FIELD, aliases.join(", "), ALIASES_WEIGHT));
    }
    let readme = found
        .and_then(|found| found.readme.as_ref())
        .map_or_else(String::new, |readme| readme_search_text(&readme.text));
    if !readme.is_empty() {
        record
            .fields
            .push(weighted(README_FIELD, readme, README_WEIGHT));
    }
    record
        .fields
        .push(weighted(LEAD_FIELD, lead_phrase(description), LEAD_WEIGHT));

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
    if report.is_some() {
        let license = found.and_then(|found| found.value_of(LICENSE_SIGNAL));
        record.tags.push(tag(LICENSE_TAG, &license_value(license)));
        let runs_on = found.and_then(|found| found.value_of(RUNS_ON_SIGNAL));
        for id in runs_on_values(runs_on, catalogue.backends()) {
            record.tags.push(tag(RUNS_ON_TAG, id));
        }
        record.tags.extend(row_tags(entry.kind, found));
    }
    let tier = found.map(|found| found.tier);
    if let Some(tier) = tier {
        record.tags.push(tag(TIER_TAG, tier.as_str()));
    }
    record.tags.push(tag(ROW_DESCRIPTION_TAG, description));
    record.boost = tier_boost(tier);
    record.snippet = Some(description.to_owned());
    record
}

/// `terms` for the index: each term, then its words split on `-`, `_` and `::`, each once.
fn keyword_text(terms: &[String]) -> String {
    let mut words: Vec<&str> = Vec::new();
    for term in terms {
        let parts = term.split(['-', '_']).flat_map(|part| part.split("::"));
        for word in std::iter::once(term.as_str()).chain(parts) {
            if !word.is_empty() && !words.contains(&word) {
                words.push(word);
            }
        }
    }
    words.join(", ")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lead_is_the_opening_phrase_of_the_description() {
        assert_eq!(
            lead_phrase("Tokenizer for Rust and Python: pure Rust by default."),
            "Tokenizer for Rust and Python"
        );
        assert_eq!(
            lead_phrase("Fast inference. Runs GGUF: yes."),
            "Fast inference"
        );
        assert_eq!(lead_phrase("A tensor library."), "A tensor library.");
        assert_eq!(
            lead_phrase("one two three four five six seven eight nine ten eleven"),
            "one two three four five six seven eight nine ten"
        );
    }

    fn text_of(terms: &[&str]) -> String {
        let owned: Vec<String> = terms.iter().map(|term| (*term).to_owned()).collect();
        keyword_text(&owned)
    }

    #[test]
    fn a_term_keeps_its_original_and_gains_its_words() {
        assert_eq!(
            text_of(&["linear-algebra", "science::math", "gpu_compute"]),
            "linear-algebra, linear, algebra, science::math, science, math, gpu_compute, gpu, compute"
        );
    }

    #[test]
    fn a_word_prints_once_and_a_plain_term_has_no_split() {
        assert_eq!(
            text_of(&["algebra", "linear-algebra"]),
            "algebra, linear-algebra, linear"
        );
        assert_eq!(text_of(&["tensor"]), "tensor");
    }

    #[test]
    fn an_empty_part_prints_nothing() {
        assert_eq!(text_of(&["-a-"]), "-a-, a");
    }
}
