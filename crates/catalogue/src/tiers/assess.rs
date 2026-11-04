//! Signal assembly and the tier rules for every entry of one check.

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, Date, EntryKind, Signals, Slug, Verdict, assess};
use catalogue_signals::assemble::choice::{ChoiceFacts, ChoiceInputs, DependentIndex, measure};
use catalogue_signals::assemble::{
    Assembled, CrateIndex, FactSources, FetchDates, LibrarySources, app_signals, context,
    library_signals,
};
use catalogue_signals::dump::{DumpSnapshot, IdSet, utc_date};
use catalogue_signals::history::{History, RepoValues, Row};
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
    /// The facts a person reads to choose a project. No rule reads them.
    pub(crate) choice: ChoiceFacts,
}

/// Assesses every entry in slug order.
///
/// Usage comes from one [`usage::compute`] call over every library crate,
/// evaluated on the dump date. An entry absent from `fetched` reads as having
/// no network signals. Fetch notes come before assembly notes. `history` supplies
/// each entry's previous tier, its dead run and its Docker baseline. The baseline
/// window counts back from the date of the pull count in use.
/// `build_checks` supplies the sandbox results of each library's crates.
///
/// Each network value carries its own data date: `today` for a fresh fetch, the
/// stored date for a cached link check or a fallback. RustSec values carry the
/// archive date, dump values the dump date. The row keeps every network value
/// with that date, so a later fallback counts its age from the real fetch.
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
    let listed: IdSet = catalogue
        .entries()
        .values()
        .flat_map(|entry| &entry.crates)
        .filter_map(|name| index.id(name.as_str()))
        .collect();
    let dependents = DependentIndex::new(snapshot, &listed);
    let choice_inputs = ChoiceInputs {
        dump: snapshot,
        index: &index,
        backends: catalogue.backends(),
        dependents: &dependents,
    };
    let none = Fetched::default();
    let assessments = catalogue
        .entries()
        .iter()
        .map(|(slug, entry)| {
            let network = fetched.get(slug).unwrap_or(&none);
            let values = &network.values;
            let facts = values.facts.as_ref().map(|found| &found.value);
            let link = values.link.as_ref().map(|found| &found.value);
            let docker = values.docker.as_ref();
            let docker_date = docker.map_or(today, |found| found.date);
            let baseline = history.docker_baseline(slug.as_str(), docker_date);
            let Assembled { signals, notes } = match entry.kind {
                EntryKind::Library => library_signals(entry, &sources, facts, link, today),
                EntryKind::App => app_signals(
                    facts,
                    values.releases.as_ref().map(|found| &found.value),
                    values.homebrew.as_ref().map(|found| found.value),
                    values.repology.as_ref().map(|found| found.value),
                    docker.map(|found| found.value),
                    baseline,
                ),
            };
            let dates = FetchDates {
                rustsec: advisories.archive_date,
                github: values.facts.as_ref().map(|found| found.date),
                link: values.link.as_ref().map(|found| found.date),
                releases: values.releases.as_ref().map(|found| found.date),
                homebrew: values.homebrew.as_ref().map(|found| found.date),
                repology: values.repology.as_ref().map(|found| found.date),
                docker: docker.map(|found| found.date),
                docker_baseline: baseline,
                contributors: values.contributors.as_ref().map(|found| found.date),
            };
            let contributors = values.contributors.as_ref().map(|found| &found.value);
            let choice = measure(entry, &choice_inputs, facts, contributors);
            let repo = github_repo(entry).ok();
            let cited = FactSources::new(entry, repo.as_ref(), dump_date, dates);
            let stars = facts.and_then(RepoValues::stars);
            let ctx = context(slug, catalogue, history, today);
            let verdict = assess(&signals, &ctx, thresholds);
            let row = Row {
                kind: entry.kind,
                tier: verdict.tier,
                dead_now: verdict.dead_now,
                dormant: verdict.dormant,
                active_users: signals.active_users,
                docker_pulls: docker
                    .filter(|found| found.date == today)
                    .map(|found| found.value),
                network: (!values.is_empty()).then(|| values.clone()),
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
                choice,
            }
        })
        .collect();
    Ok(assessments)
}
