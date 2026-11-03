//! Signal assembly and the tier rules for every entry of one check.

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, Date, EntryKind, Slug, Verdict, assess};
use catalogue_signals::assemble::{
    Assembled, CrateIndex, LibrarySources, app_signals, context, library_signals,
};
use catalogue_signals::dump::{DumpSnapshot, IdSet, utc_date};
use catalogue_signals::history::{History, Row};
use catalogue_signals::rustsec::Advisories;
use catalogue_signals::sandbox::BuildChecks;
use catalogue_signals::usage;

use super::collect::Fetched;
use super::command::required;
use super::error::TiersError;

/// One entry's verdict, the row it adds to the history and the notes about
/// inputs that stayed unresolved.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Assessment {
    pub(crate) slug: Slug,
    pub(crate) verdict: Verdict,
    pub(crate) row: Row,
    pub(crate) notes: Vec<String>,
}

/// Assesses every entry in slug order.
///
/// Usage comes from one [`usage::compute`] call over every library crate,
/// evaluated on the dump date. An entry absent from `fetched` reads as having
/// no network signals. Fetch notes come before assembly notes. `history` supplies
/// each entry's previous tier, its dead run and its Docker baseline.
/// `build_checks` supplies the sandbox results of each library's crates.
pub(crate) fn assess_entries(
    catalogue: &Catalogue,
    snapshot: &DumpSnapshot,
    advisories: &Advisories,
    build_checks: &BuildChecks,
    fetched: &BTreeMap<Slug, Fetched>,
    history: &History,
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
        build_checks,
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
                    network.docker_pulls,
                    history.docker_baseline(slug.as_str(), today),
                ),
            };
            let ctx = context(slug, catalogue, history, today);
            let verdict = assess(&signals, &ctx, thresholds);
            let row = Row {
                kind: entry.kind,
                tier: verdict.tier,
                dead_now: verdict.dead_now,
                dormant: verdict.dormant,
                active_users: signals.active_users,
                docker_pulls: network.docker_pulls,
            };
            let mut all = network.notes.clone();
            all.extend(notes);
            Assessment {
                slug: slug.clone(),
                verdict,
                row,
                notes: all,
            }
        })
        .collect();
    Ok(assessments)
}
