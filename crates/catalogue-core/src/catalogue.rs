use std::collections::BTreeMap;

use crate::entry::Entry;
use crate::ids::Slug;
use crate::judgment::Judgment;
use crate::load::{self, DataSources, Diagnostics};
use crate::registry::{Maintainer, SynonymGroup, Topic};

/// A checked data set.
///
/// Every cross-file rule holds for a value of this type.
#[derive(Debug, Clone)]
pub struct Catalogue {
    entries: BTreeMap<Slug, Entry>,
    judgments: BTreeMap<u64, Judgment>,
    topics: Vec<Topic>,
    maintainers: Vec<Maintainer>,
    synonyms: Vec<SynonymGroup>,
}

impl Catalogue {
    /// Parses and checks `sources`.
    ///
    /// On failure the result lists every error found, not only the first.
    pub fn load(sources: DataSources) -> Result<Self, Diagnostics> {
        load::load(sources)
    }

    pub(crate) fn new(
        entries: BTreeMap<Slug, Entry>,
        judgments: BTreeMap<u64, Judgment>,
        topics: Vec<Topic>,
        maintainers: Vec<Maintainer>,
        synonyms: Vec<SynonymGroup>,
    ) -> Self {
        Self {
            entries,
            judgments,
            topics,
            maintainers,
            synonyms,
        }
    }

    /// Entries keyed by slug, in sorted slug order.
    pub fn entries(&self) -> &BTreeMap<Slug, Entry> {
        &self.entries
    }

    /// The entry with this slug.
    pub fn entry(&self, slug: &Slug) -> Option<&Entry> {
        self.entries.get(slug)
    }

    /// Judgments keyed by issue number, in ascending order.
    pub fn judgments(&self) -> &BTreeMap<u64, Judgment> {
        &self.judgments
    }

    /// Topics in file order.
    pub fn topics(&self) -> &[Topic] {
        &self.topics
    }

    /// Maintainers in file order.
    pub fn maintainers(&self) -> &[Maintainer] {
        &self.maintainers
    }

    /// Synonym groups in file order.
    pub fn synonyms(&self) -> &[SynonymGroup] {
        &self.synonyms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{BETA, put, valid};

    #[test]
    fn exposes_loaded_data() {
        let catalogue = Catalogue::load(valid()).expect("valid data");
        let slugs: Vec<&str> = catalogue.entries().keys().map(Slug::as_str).collect();
        assert_eq!(slugs, ["alpha", "beta"]);
        assert_eq!(
            catalogue.judgments().keys().copied().collect::<Vec<_>>(),
            [7]
        );
        assert_eq!(catalogue.topics().len(), 2);
        assert_eq!(catalogue.maintainers().len(), 2);
        assert_eq!(catalogue.synonyms().len(), 1);
        let alpha = Slug::new("alpha").expect("slug");
        assert!(catalogue.entry(&alpha).is_some());
    }

    #[test]
    fn entries_come_back_sorted_regardless_of_input_order() {
        let mut src = valid();
        put(
            &mut src,
            "entries/aardvark.toml",
            &BETA.replace("Beta", "Aardvark"),
        );
        let catalogue = Catalogue::load(src).expect("valid data");
        let slugs: Vec<&str> = catalogue.entries().keys().map(Slug::as_str).collect();
        assert_eq!(slugs, ["aardvark", "alpha", "beta"]);
    }

    #[test]
    fn failure_lists_diagnostics() {
        let mut src = valid();
        put(&mut src, "topics.toml", "");
        let Err(diags) = Catalogue::load(src) else {
            panic!("expected diagnostics");
        };
        assert!(!diags.is_empty());
    }
}
