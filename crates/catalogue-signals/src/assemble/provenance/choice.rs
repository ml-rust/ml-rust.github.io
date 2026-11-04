//! The choice facts of one entry as cited rows of the signal table.

use catalogue_core::{
    CONTRIBUTORS_SIGNAL, DEPENDENTS_SIGNAL, Date, LICENSE_SIGNAL, NATIVE_CODE_SIGNAL,
    PYTHON_SIGNAL, RUNS_ON_EVIDENCE_SIGNAL, RUNS_ON_SIGNAL, SignalFact, SourceRef,
};
use url::Url;

use super::sources::{DUMP_PAGE, DUMP_SOURCE, FactSources};
use crate::assemble::choice::{ChoiceFacts, LicenseOrigin};

/// The source and the data date of one row.
type Cited = (Option<SourceRef>, Option<Date>);

/// The dump as a source.
fn dump_source(sources: &FactSources) -> Cited {
    (
        Url::parse(DUMP_PAGE)
            .ok()
            .map(|url| SourceRef::new(DUMP_SOURCE, url)),
        Some(sources.dump),
    )
}

/// The GitHub page `url`, named `label`, with the data date `date`.
fn github_source(label: &str, url: Option<&Url>, date: Option<Date>) -> Cited {
    (url.map(|url| SourceRef::new(label, url.clone())), date)
}

/// One row. A fact without a value has no source and no date.
fn row(name: &str, label: &str, value: Option<String>, cited: Cited) -> SignalFact {
    let (source, fetched) = if value.is_some() { cited } else { (None, None) };
    SignalFact {
        name: name.to_owned(),
        label: label.to_owned(),
        value,
        source,
        fetched,
        counts_toward_tier: false,
    }
}

/// The rows of `facts`, each with its source and data date. No tier rule reads any of them.
///
/// The labels state what the dump covers: each listed crate and its direct dependencies.
pub fn choice_signal_facts(facts: &ChoiceFacts, sources: &FactSources) -> Vec<SignalFact> {
    let dump = dump_source(sources);
    let license = match facts.license.as_ref().map(|found| found.origin) {
        Some(LicenseOrigin::Repo) => github_source(
            "GitHub repository",
            sources.repo_page.as_ref(),
            sources.dates.github,
        ),
        _ => dump.clone(),
    };
    let contributors = github_source(
        "GitHub contributors",
        sources.contributors_page.as_ref(),
        sources.dates.contributors,
    );
    vec![
        row(
            LICENSE_SIGNAL,
            "License",
            facts.license.as_ref().map(|found| found.text.clone()),
            license,
        ),
        row(
            RUNS_ON_SIGNAL,
            "Runs on, from listed crates and their direct dependencies",
            facts.runs_on.as_ref().map(|found| found.value()),
            dump.clone(),
        ),
        row(
            RUNS_ON_EVIDENCE_SIGNAL,
            "What shows each backend",
            facts.runs_on.as_ref().map(|found| found.evidence()),
            dump.clone(),
        ),
        row(
            NATIVE_CODE_SIGNAL,
            "Native code, from listed crates and their dependencies",
            facts.native_code.clone(),
            dump.clone(),
        ),
        row(
            PYTHON_SIGNAL,
            "Python bindings, from direct dependencies of listed crates",
            facts.python.clone(),
            dump.clone(),
        ),
        row(
            DEPENDENTS_SIGNAL,
            "Crates that depend on a listed crate directly",
            facts.dependents.map(|count| count.to_string()),
            dump,
        ),
        row(
            CONTRIBUTORS_SIGNAL,
            "Contributors with a GitHub account",
            facts.contributors.map(|count| count.to_string()),
            contributors,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use catalogue_core::never_counts;

    use super::*;
    use crate::assemble::choice::{Licensed, RunsOn};
    use crate::assemble::fixtures::{library, today};
    use crate::assemble::provenance::FetchDates;
    use crate::github::RepoRef;

    fn day(on: i8) -> Date {
        Date::new(2026, 10, on).expect("date")
    }

    fn sources() -> FactSources {
        let repo = RepoRef::parse("ml-rust/tool").expect("repo");
        let dates = FetchDates {
            github: Some(day(1)),
            contributors: Some(day(2)),
            ..FetchDates::default()
        };
        FactSources::new(&library(&["tool"]), Some(&repo), today(), dates)
    }

    fn full(origin: LicenseOrigin) -> ChoiceFacts {
        ChoiceFacts {
            license: Some(Licensed {
                text: "MIT".to_owned(),
                origin,
            }),
            runs_on: Some(RunsOn::NoGpuFeature),
            native_code: Some("No native code found".to_owned()),
            python: Some("No Python binding crate found".to_owned()),
            dependents: Some(4),
            contributors: Some(38),
        }
    }

    fn find<'a>(rows: &'a [SignalFact], name: &str) -> &'a SignalFact {
        rows.iter()
            .find(|fact| fact.name == name)
            .unwrap_or_else(|| panic!("no fact {name}"))
    }

    #[test]
    fn every_measured_fact_is_cited_dated_and_never_counts() {
        let rows = choice_signal_facts(&full(LicenseOrigin::Dump), &sources());
        assert_eq!(rows.len(), 7);
        for fact in &rows {
            assert!(fact.value.is_some(), "{}", fact.name);
            assert!(fact.problems().is_empty(), "{:?}", fact.problems());
            assert!(!fact.counts_toward_tier, "{}", fact.name);
            assert!(never_counts(&fact.name), "{}", fact.name);
        }
    }

    #[test]
    fn each_fact_names_its_own_source_and_date() {
        let rows = choice_signal_facts(&full(LicenseOrigin::Repo), &sources());
        let license = find(&rows, LICENSE_SIGNAL);
        assert_eq!(
            license.source.as_ref().map(|source| source.url.as_str()),
            Some("https://github.com/ml-rust/tool")
        );
        assert_eq!(license.fetched, Some(day(1)), "a fallback keeps its date");
        let contributors = find(&rows, CONTRIBUTORS_SIGNAL);
        assert_eq!(
            contributors
                .source
                .as_ref()
                .map(|source| source.url.as_str()),
            Some("https://github.com/ml-rust/tool/graphs/contributors")
        );
        assert_eq!(contributors.fetched, Some(day(2)));
        let dependents = find(&rows, DEPENDENTS_SIGNAL);
        assert_eq!(
            dependents.source.as_ref().map(|source| source.url.as_str()),
            Some(DUMP_PAGE)
        );
        assert_eq!(dependents.fetched, Some(today()));
        let dump_license = choice_signal_facts(&full(LicenseOrigin::Dump), &sources());
        assert_eq!(
            find(&dump_license, LICENSE_SIGNAL)
                .source
                .as_ref()
                .map(|source| source.url.as_str()),
            Some(DUMP_PAGE)
        );
    }

    #[test]
    fn a_fact_with_no_value_has_no_source_and_no_date() {
        let rows = choice_signal_facts(&ChoiceFacts::default(), &sources());
        assert_eq!(rows.len(), 7);
        for fact in &rows {
            assert_eq!(fact.value, None, "{}", fact.name);
            assert_eq!(fact.source, None, "{}", fact.name);
            assert_eq!(fact.fetched, None, "{}", fact.name);
        }
    }
}
