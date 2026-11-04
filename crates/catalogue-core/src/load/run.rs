use super::categories::check_categories;
use super::deprecated::check_deprecated;
use super::diagnostics::Diagnostics;
use super::discovery::check_discovery;
use super::entries::check_entries;
use super::judgments::{Refs, check_judgments};
use super::maintainers::check_maintainers;
use super::parse::{parse_all, parse_optional};
use super::sources::DataSources;
use super::synonyms::check_synonyms;
use super::thresholds::check_thresholds;
use super::topics::check_topics;
use crate::catalogue::Catalogue;
use crate::entry::Entry;
use crate::judgment::Judgment;
use crate::registry::{Categories, Discovery, Maintainers, Synonyms, Topics};

/// Parses and checks every file, collecting every error.
pub(crate) fn load(sources: DataSources) -> Result<Catalogue, Diagnostics> {
    let mut diags = Diagnostics::new();

    let topics = parse_optional::<Topics>(sources.topics.as_ref(), "topics.toml", &mut diags);
    let categories =
        parse_optional::<Categories>(sources.categories.as_ref(), "categories.toml", &mut diags);
    let maintainers =
        parse_optional::<Maintainers>(sources.maintainers.as_ref(), "maintainers.toml", &mut diags);
    let synonyms =
        parse_optional::<Synonyms>(sources.synonyms.as_ref(), "synonyms.toml", &mut diags);

    let discovery = sources.discovery.as_ref().map(|file| {
        let mut found = Diagnostics::new();
        let parsed = parse_optional::<Discovery>(Some(file), "discovery.toml", &mut found);
        if found.is_empty() {
            check_discovery(&parsed, &mut found);
        }
        diags.extend(found);
        parsed.value
    });

    let thresholds = sources
        .thresholds
        .as_ref()
        .and_then(|file| check_thresholds(file, &mut diags));
    let deprecated = sources
        .deprecated
        .as_ref()
        .and_then(|file| check_deprecated(file, &mut diags));

    let topic_ids = check_topics(&topics, &mut diags);
    let category_ids = check_categories(&categories, &mut diags);
    let maintainer_index = check_maintainers(&maintainers, &mut diags);
    check_synonyms(&synonyms, &mut diags);

    let entry_files = parse_all::<Entry>(&sources.entries, &mut diags);
    let entries = check_entries(entry_files, &topic_ids, &category_ids, &mut diags);

    let judgment_files = parse_all::<Judgment>(&sources.judgments, &mut diags);
    let refs = Refs {
        entries: &entries,
        topics: &topic_ids,
        maintainers: &maintainer_index,
    };
    let judgments = check_judgments(judgment_files, &refs, &mut diags);

    if diags.is_empty() {
        Ok(Catalogue {
            entries,
            judgments,
            topics: topics.value.topic,
            categories: categories.value.category,
            maintainers: maintainers.value.maintainer,
            synonyms: synonyms.value.group,
            discovery,
            thresholds,
            deprecated,
        })
    } else {
        Err(diags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{put, valid};

    #[test]
    fn valid_data_loads() {
        assert!(load(valid()).is_ok());
    }

    #[test]
    fn empty_sources_load_as_an_empty_catalogue() {
        let catalogue = load(DataSources::default()).expect("empty data is valid");
        assert!(catalogue.entries().is_empty());
        assert!(catalogue.judgments().is_empty());
    }

    #[test]
    fn every_error_is_collected() {
        let mut src = valid();
        put(&mut src, "entries/alpha.toml", "not valid toml = = =");
        put(&mut src, "synonyms.toml", "[[group]]\nterms = [\"x\"]\n");
        put(&mut src, "maintainers.toml", "bogus = 1");
        let Err(diags) = load(src) else {
            panic!("expected diagnostics");
        };
        let paths: Vec<&str> = diags.iter().map(|d| d.path.as_str()).collect();
        assert!(paths.contains(&"entries/alpha.toml"));
        assert!(paths.contains(&"synonyms.toml"));
        assert!(paths.contains(&"maintainers.toml"));
    }
}
