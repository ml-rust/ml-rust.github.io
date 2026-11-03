//! Signal assembly and the tier rules for every entry of one check.

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, Date, EntryKind, Signals, Slug, Verdict, assess};
use catalogue_signals::assemble::{
    Assembled, CrateIndex, FactSources, FetchDates, LibrarySources, app_signals, context,
    library_signals,
};
use catalogue_signals::dump::{DumpSnapshot, IdSet, utc_date};
use catalogue_signals::github::RepoFacts;
use catalogue_signals::history::{History, Row};
use catalogue_signals::rustsec::Advisories;
use catalogue_signals::sandbox::BuildChecks;
use catalogue_signals::usage;

use super::collect::{Fetched, github_repo};
use super::command::required;
use super::error::TiersError;

/// One entry's verdict, the row it adds to the history, the notes about
/// inputs that stayed unresolved, and what the report needs to cite each signal.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Assessment {
    pub(crate) slug: Slug,
    pub(crate) verdict: Verdict,
    pub(crate) row: Row,
    pub(crate) notes: Vec<String>,
    /// The signals the tier rules read.
    pub(crate) signals: Signals,
    /// The source page and the data date of each signal.
    pub(crate) sources: FactSources,
    /// The GitHub star count. Display only: no rule reads it.
    pub(crate) stars: Option<u64>,
}

/// Assesses every entry in slug order.
///
/// Usage comes from one [`usage::compute`] call over every library crate,
/// evaluated on the dump date. An entry absent from `fetched` reads as having
/// no network signals. Fetch notes come before assembly notes. `history` supplies
/// each entry's previous tier, its dead run and its Docker baseline.
/// `build_checks` supplies the sandbox results of each library's crates.
///
/// Each network value carries `today` as its data date, except a link check,
/// which carries its own. RustSec values carry the archive date, dump values
/// the dump date.
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
    let dump_date = Date::from(utc_date(snapshot.meta.timestamp));
    let found = usage::compute(snapshot, &targets, dump_date.civil());
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
            let link = network.link.as_ref().map(|outcome| &outcome.check);
            let baseline = history.docker_baseline(slug.as_str(), today);
            let Assembled { signals, notes } = match entry.kind {
                EntryKind::Library => library_signals(entry, &sources, facts, link, today),
                EntryKind::App => app_signals(
                    facts,
                    network.releases.as_ref(),
                    network.homebrew.as_ref(),
                    network.repology.as_ref(),
                    network.docker_pulls,
                    baseline,
                ),
            };
            let on_run = |found: bool| found.then_some(today);
            let dates = FetchDates {
                rustsec: advisories.archive_date,
                github: on_run(facts.is_some()),
                link: network.link.as_ref().map(|outcome| outcome.checked),
                releases: on_run(network.releases.is_some()),
                homebrew: on_run(network.homebrew.is_some()),
                repology: on_run(network.repology.is_some()),
                docker: on_run(network.docker_pulls.is_some()),
                docker_baseline: baseline,
            };
            let repo = github_repo(entry).ok();
            let cited = FactSources::new(entry, repo.as_ref(), dump_date, dates);
            let stars = match facts {
                Some(RepoFacts::Found(found)) => Some(u64::from(found.stars)),
                Some(RepoFacts::Gone { .. }) | None => None,
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
                signals,
                sources: cited,
                stars,
            }
        })
        .collect();
    Ok(assessments)
}
