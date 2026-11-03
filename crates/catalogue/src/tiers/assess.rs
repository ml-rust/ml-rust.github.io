//! Signal assembly and the tier rules for every entry of one check.

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, Date, EntryKind, Slug, Verdict, assess};
use catalogue_signals::assemble::{
    Assembled, CrateIndex, LibrarySources, app_signals, context, library_signals,
};
use catalogue_signals::dump::{DumpSnapshot, IdSet, utc_date};
use catalogue_signals::rustsec::Advisories;
use catalogue_signals::usage;

use super::collect::Fetched;
use super::command::{TiersError, required};

/// One entry's verdict and the notes about inputs that stayed unresolved.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Assessment {
    pub(crate) slug: Slug,
    pub(crate) verdict: Verdict,
    pub(crate) notes: Vec<String>,
}

/// Assesses every entry in slug order.
///
/// Usage comes from one [`usage::compute`] call over every library crate,
/// evaluated on the dump date. An entry absent from `fetched` reads as having
/// no network signals. Fetch notes come before assembly notes.
pub(crate) fn assess_entries(
    catalogue: &Catalogue,
    snapshot: &DumpSnapshot,
    advisories: &Advisories,
    fetched: &BTreeMap<Slug, Fetched>,
    today: Date,
) -> Result<Vec<Assessment>, TiersError> {
    let (thresholds, deprecated) = required(catalogue)?;
    let index = CrateIndex::new(snapshot);
    let targets: IdSet = catalogue
        .entries()
        .values()
        .filter(|entry| entry.kind == EntryKind::Library)
        .flat_map(|entry| &entry.crates)
        .filter_map(|name| index.id(name.as_str()))
        .collect();
    let found = usage::compute(snapshot, &targets, utc_date(snapshot.meta.timestamp));
    let sources = LibrarySources {
        dump: snapshot,
        index: &index,
        usage: &found,
        advisories,
        deprecated,
        thresholds,
    };
    let none = Fetched::default();
    let assessments = catalogue
        .entries()
        .iter()
        .map(|(slug, entry)| {
            let network = fetched.get(slug).unwrap_or(&none);
            let facts = network.facts.as_ref();
            let Assembled { signals, notes } = match entry.kind {
                EntryKind::Library => {
                    library_signals(entry, &sources, facts, network.link.as_ref(), today)
                }
                EntryKind::App => app_signals(
                    facts,
                    network.releases.as_ref(),
                    network.homebrew.as_ref(),
                    network.repology.as_ref(),
                ),
            };
            let verdict = assess(&signals, &context(slug, catalogue, today), thresholds);
            let mut all = network.notes.clone();
            all.extend(notes);
            Assessment {
                slug: slug.clone(),
                verdict,
                notes: all,
            }
        })
        .collect();
    Ok(assessments)
}
