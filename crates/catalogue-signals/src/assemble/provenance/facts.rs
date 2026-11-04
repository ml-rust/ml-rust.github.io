//! The signal fact table: every signal of one entry with its value, source and date.

use std::fmt::Display;

use catalogue_core::amount_text::precise;
use catalogue_core::{
    ACTIVE_DEPENDENTS_LABEL, Date, EntryKind, FORMER_GLORY_LABEL, NEW_DEPENDENTS_LABEL,
    PEAK_DEPENDENTS_LABEL, STARS_SIGNAL, SignalFact, Signals, SourceRef,
};
use url::Url;

use super::sources::{BUILD_CHECK_PAGE, DUMP_PAGE, DUMP_SOURCE, FactSources};

/// The value text of a finding that the check looked for and did not find.
const NONE_FOUND: &str = "none";

/// One row before citing: id, label, value text and the origin of its source.
type Row = (&'static str, &'static str, Option<String>, Origin);

/// Where one signal comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Dump,
    Rustsec,
    Build,
    Repo,
    Link,
    Releases,
    Homebrew,
    Repology,
    Docker,
    DockerBaseline,
}

/// The source and the data date of `origin`. Either is `None` when the run lacks it.
fn cite(
    origin: Origin,
    signals: &Signals,
    sources: &FactSources,
) -> (Option<SourceRef>, Option<Date>) {
    let dates = &sources.dates;
    let source =
        |label: &str, url: &Option<Url>| url.as_ref().map(|url| SourceRef::new(label, url.clone()));
    match origin {
        Origin::Dump => (
            Url::parse(DUMP_PAGE)
                .ok()
                .map(|url| SourceRef::new(DUMP_SOURCE, url)),
            Some(sources.dump),
        ),
        Origin::Rustsec => (
            source("RustSec advisory database", &sources.advisory_page),
            dates.rustsec,
        ),
        Origin::Build => (
            Url::parse(BUILD_CHECK_PAGE)
                .ok()
                .map(|url| SourceRef::new("sandbox cargo check", url)),
            signals.build_checked,
        ),
        Origin::Repo => (
            source("GitHub repository", &sources.repo_page),
            dates.github,
        ),
        Origin::Link => (
            source(
                "crates.io package .cargo_vcs_info.json",
                &sources.crate_page,
            ),
            dates.link,
        ),
        Origin::Releases => (
            source("GitHub releases", &sources.releases_page),
            dates.releases,
        ),
        Origin::Homebrew => (
            source("Homebrew formula", &sources.homebrew_page),
            dates.homebrew,
        ),
        Origin::Repology => (
            source("Repology project", &sources.repology_page),
            dates.repology,
        ),
        Origin::Docker => (source("Docker Hub", &sources.docker_page), dates.docker),
        Origin::DockerBaseline => (
            source("Docker Hub", &sources.docker_page),
            dates.docker_baseline.map(|(date, _)| date),
        ),
    }
}

/// The text of a known value.
fn text<T: Display>(value: Option<T>) -> Option<String> {
    value.map(|value| value.to_string())
}

/// The text of a known weighted amount, with float noise removed.
fn amount(value: Option<f64>) -> Option<String> {
    value.map(precise)
}

/// The text of a finding: the finding, or `none` when the check looked and found none.
fn finding(value: Option<&str>, looked: bool) -> Option<String> {
    match value {
        Some(found) => Some(found.to_owned()),
        None => looked.then(|| NONE_FOUND.to_owned()),
    }
}

/// The text of a list of ids: the ids joined with `, `, or `none` for an empty list.
fn ids(value: Option<&[String]>) -> Option<String> {
    value.map(|list| {
        if list.is_empty() {
            NONE_FOUND.to_owned()
        } else {
            list.join(", ")
        }
    })
}

/// The crates.io dump rows, `first_release` to `downloads_per_month`.
///
/// `last_release` cites the dump for a library and the releases page for an app.
fn dump_rows(signals: &Signals) -> Vec<Row> {
    let release_origin = if signals.kind == EntryKind::Library {
        Origin::Dump
    } else {
        Origin::Releases
    };
    vec![
        (
            "first_release",
            "First release",
            text(signals.first_release),
            Origin::Dump,
        ),
        (
            "latest_version",
            "Latest version",
            signals.latest_version.clone(),
            Origin::Dump,
        ),
        (
            "release_count",
            "Releases",
            text(signals.release_count),
            Origin::Dump,
        ),
        (
            "last_release",
            "Latest release",
            text(signals.last_release),
            release_origin,
        ),
        (
            "all_yanked",
            "Every version yanked",
            text(signals.all_yanked),
            Origin::Dump,
        ),
        (
            "crate_deleted",
            "Crate deleted from crates.io",
            text(signals.crate_deleted),
            Origin::Dump,
        ),
        (
            "all_prerelease",
            "Every version a prerelease",
            text(signals.all_prerelease),
            Origin::Dump,
        ),
        (
            "active_users",
            ACTIVE_DEPENDENTS_LABEL,
            amount(signals.active_users),
            Origin::Dump,
        ),
        (
            "new_adopters",
            NEW_DEPENDENTS_LABEL,
            amount(signals.new_adopters),
            Origin::Dump,
        ),
        (
            "peak_users",
            PEAK_DEPENDENTS_LABEL,
            amount(signals.peak_users),
            Origin::Dump,
        ),
        (
            "former_glory",
            FORMER_GLORY_LABEL,
            amount(signals.former_glory),
            Origin::Dump,
        ),
        (
            "downloads_per_month",
            "Downloads over 30 days",
            text(signals.downloads_per_month),
            Origin::Dump,
        ),
    ]
}

/// The RustSec rows: the unmaintained advisory and the unpatched vulnerabilities.
fn rustsec_rows(signals: &Signals) -> Vec<Row> {
    let advisories_read = signals.unpatched_vulnerabilities.is_some();
    vec![
        (
            "rustsec_unmaintained",
            "RustSec unmaintained advisory",
            finding(signals.rustsec_unmaintained.as_deref(), advisories_read),
            Origin::Rustsec,
        ),
        (
            "unpatched_vulnerabilities",
            "Unpatched vulnerabilities past the grace window",
            ids(signals.unpatched_vulnerabilities.as_deref()),
            Origin::Rustsec,
        ),
    ]
}

/// The deprecation rows, read from the primary crate's dump row.
fn deprecation_rows(signals: &Signals) -> Vec<Row> {
    let primary_read = signals.crate_deleted == Some(false);
    vec![
        (
            "deprecation_notice",
            "Deprecation notice",
            finding(signals.deprecation_notice.as_deref(), primary_read),
            Origin::Dump,
        ),
        (
            "deprecated_requirement",
            "Deprecated requirement",
            finding(signals.deprecated_requirement.as_deref(), primary_read),
            Origin::Dump,
        ),
    ]
}

/// The sandbox build check rows.
fn build_rows(signals: &Signals) -> Vec<Row> {
    vec![
        (
            "build_fails",
            "Latest release fails cargo check",
            text(signals.build_fails),
            Origin::Build,
        ),
        (
            "build_checked",
            "Build check date",
            text(signals.build_checked),
            Origin::Build,
        ),
        (
            "build_toolchain",
            "Build check toolchain",
            signals.build_toolchain.clone(),
            Origin::Build,
        ),
    ]
}

/// The GitHub repo rows, `repo_archived` to `maintainer_replies_365`.
fn github_rows(signals: &Signals) -> Vec<Row> {
    vec![
        (
            "repo_archived",
            "Repository archived",
            text(signals.repo_archived),
            Origin::Repo,
        ),
        (
            "repo_gone",
            "Repository gone",
            text(signals.repo_gone),
            Origin::Repo,
        ),
        (
            "last_human_commit",
            "Latest human commit",
            text(signals.last_human_commit),
            Origin::Repo,
        ),
        (
            "oldest_unanswered_days",
            "Days the oldest outside issue waits for a reply",
            text(signals.oldest_unanswered_days),
            Origin::Repo,
        ),
        (
            "maintainer_replies_365",
            "Maintainer replies over 365 days",
            text(signals.maintainer_replies_365),
            Origin::Repo,
        ),
    ]
}

/// The published docs row and the repo link check row.
fn publication_rows(signals: &Signals) -> Vec<Row> {
    vec![
        (
            "docs_present",
            "Published docs",
            text(signals.docs_present),
            Origin::Dump,
        ),
        (
            "repo_link_verified",
            "Release commit on a repo branch or tag",
            text(signals.repo_link_verified),
            Origin::Link,
        ),
    ]
}

/// The app distribution rows: GitHub releases, Homebrew, Repology and Docker Hub.
fn distribution_rows(signals: &Signals, sources: &FactSources) -> Vec<Row> {
    let baseline = signals
        .docker_pull_growth_90d
        .and(sources.dates.docker_baseline)
        .map(|(_, pulls)| pulls);
    vec![
        (
            "binary_downloads_last3",
            "Binary downloads over the last 3 releases",
            text(signals.binary_downloads_last3),
            Origin::Releases,
        ),
        (
            "binary_downloads_per_release",
            "Binary downloads per release",
            text(signals.binary_downloads_per_release),
            Origin::Releases,
        ),
        (
            "homebrew_on_request_365d",
            "Homebrew installs on request over 365 days",
            text(signals.homebrew_on_request_365d),
            Origin::Homebrew,
        ),
        (
            "repology_families",
            "Repology repository families",
            text(signals.repology_families),
            Origin::Repology,
        ),
        (
            "docker_pull_growth_90d",
            "Docker pull growth over 90 days",
            text(signals.docker_pull_growth_90d),
            Origin::Docker,
        ),
        (
            "docker_pull_baseline",
            "Docker pulls at the growth baseline",
            text(baseline),
            Origin::DockerBaseline,
        ),
    ]
}

/// Every signal of one entry with its value, source and data date, for the entry page.
///
/// One row per [`Signals`] field except `kind`, in this order:
///
/// | rows | source | date |
/// | --- | --- | --- |
/// | `first_release` to `downloads_per_month` | crates.io database dump | dump date |
/// | `rustsec_unmaintained`, `unpatched_vulnerabilities` | RustSec page of the primary crate | archive date |
/// | `deprecation_notice`, `deprecated_requirement` | crates.io database dump | dump date |
/// | `build_fails`, `build_checked`, `build_toolchain` | sandbox check workflow | `build_checked` |
/// | `repo_archived` to `maintainer_replies_365` | GitHub repo page | GitHub fetch date |
/// | `docs_present` | crates.io database dump | dump date |
/// | `repo_link_verified` | crates.io page of the primary crate | link check date |
/// | `binary_downloads_last3`, `binary_downloads_per_release` | GitHub releases page | releases fetch date |
/// | `homebrew_on_request_365d` | Homebrew formula page | Homebrew fetch date |
/// | `repology_families` | Repology project page | Repology fetch date |
/// | `docker_pull_growth_90d` | Docker Hub page | Docker fetch date |
///
/// `last_release` cites the dump for a library and the GitHub releases page for an app.
/// Two rows follow the [`Signals`] fields:
///
/// - `docker_pull_baseline`: the pull count growth measures against, dated by
///   its history snapshot. Present only with a growth value.
/// - `stars`: the GitHub star count, cited like the repo rows. It never counts toward a tier.
///
/// A `None` signal is a row with no value, no source and no date. A finding
/// field reads `none` when its source was read and holds no finding: the
/// advisories for `rustsec_unmaintained`, the primary crate's dump row for
/// `deprecation_notice` and `deprecated_requirement`.
pub fn signal_facts(
    signals: &Signals,
    sources: &FactSources,
    stars: Option<u64>,
) -> Vec<SignalFact> {
    let mut rows: Vec<Row> = Vec::with_capacity(33);
    rows.extend(dump_rows(signals));
    rows.extend(rustsec_rows(signals));
    rows.extend(deprecation_rows(signals));
    rows.extend(build_rows(signals));
    rows.extend(github_rows(signals));
    rows.extend(publication_rows(signals));
    rows.extend(distribution_rows(signals, sources));
    rows.push((STARS_SIGNAL, "GitHub stars", text(stars), Origin::Repo));
    rows.into_iter()
        .map(|(name, label, value, origin)| {
            let (source, fetched) = match value {
                Some(_) => cite(origin, signals, sources),
                None => (None, None),
            };
            SignalFact {
                name: name.to_owned(),
                label: label.to_owned(),
                value,
                source,
                fetched,
                counts_toward_tier: name != STARS_SIGNAL,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::fixtures::{library, today};
    use crate::assemble::provenance::FetchDates;
    use crate::github::RepoRef;
    use catalogue_core::Entry;

    fn day(on: i8) -> Date {
        Date::new(2026, 11, on).expect("date")
    }

    fn app() -> Entry {
        let text = "name = \"Tool\"\nkind = \"app\"\nrepo = \"https://github.com/ml-rust/tool\"\n\
            crates = [\"tool\"]\ntopics = [\"ml\"]\nadded = 2026-01-01\n\
            [distribution]\nhomebrew = \"python@3.12\"\nrepology = \"tool\"\ndocker = [\"ml-rust/tool\"]\n\
            [description]\ntext = \"A tool.\"\nsource = \"human\"\n";
        toml::from_str(text).expect("entry parses")
    }

    fn every_date() -> FetchDates {
        FetchDates {
            rustsec: Some(day(1)),
            github: Some(today()),
            link: Some(day(2)),
            releases: Some(today()),
            homebrew: Some(today()),
            repology: Some(today()),
            docker: Some(today()),
            docker_baseline: Some((Date::new(2026, 8, 1).expect("date"), 1_000)),
            contributors: Some(today()),
        }
    }

    fn repo() -> RepoRef {
        RepoRef::parse("ml-rust/tool").expect("repo")
    }

    /// Signals with every field known.
    fn full(kind: EntryKind) -> Signals {
        Signals {
            kind,
            first_release: Some(Date::new(2024, 1, 1).expect("date")),
            latest_version: Some("1.4.2".to_owned()),
            release_count: Some(12),
            last_release: Some(Date::new(2026, 9, 1).expect("date")),
            all_yanked: Some(false),
            crate_deleted: Some(false),
            all_prerelease: Some(false),
            active_users: Some(40.0),
            new_adopters: Some(6.5),
            peak_users: Some(50.0),
            former_glory: Some(0.8),
            downloads_per_month: Some(9_000),
            rustsec_unmaintained: Some("RUSTSEC-2026-0001".to_owned()),
            unpatched_vulnerabilities: Some(vec!["RUSTSEC-2026-0002".to_owned()]),
            deprecation_notice: Some("deprecated".to_owned()),
            deprecated_requirement: Some("tokio 0.1".to_owned()),
            build_fails: Some(false),
            build_checked: Some(day(1)),
            build_toolchain: Some("rustc 1.95.0".to_owned()),
            repo_archived: Some(false),
            repo_gone: Some(false),
            last_human_commit: Some(Date::new(2026, 10, 1).expect("date")),
            oldest_unanswered_days: Some(12),
            maintainer_replies_365: Some(7),
            docs_present: Some(true),
            repo_link_verified: Some(true),
            binary_downloads_last3: Some(900),
            binary_downloads_per_release: Some(300),
            homebrew_on_request_365d: Some(640),
            repology_families: Some(2),
            docker_pull_growth_90d: Some(500),
        }
    }

    fn value<'a>(facts: &'a [SignalFact], name: &str) -> &'a SignalFact {
        facts
            .iter()
            .find(|fact| fact.name == name)
            .unwrap_or_else(|| panic!("no fact {name}"))
    }

    #[test]
    fn every_known_signal_is_cited_and_dated() {
        let sources = FactSources::new(&app(), Some(&repo()), day(2), every_date());
        for kind in [EntryKind::App, EntryKind::Library] {
            let facts = signal_facts(&full(kind), &sources, Some(321));
            assert_eq!(facts.len(), 33);
            for fact in &facts {
                assert!(fact.value.is_some(), "{}", fact.name);
                assert!(fact.problems().is_empty(), "{:?}", fact.problems());
            }
        }
    }

    #[test]
    fn unknown_signals_have_no_value_source_or_date() {
        let sources = FactSources::new(&library(&["main"]), None, day(2), FetchDates::default());
        let facts = signal_facts(&Signals::unknown(EntryKind::Library), &sources, None);
        assert_eq!(facts.len(), 33);
        for fact in &facts {
            assert_eq!(fact.value, None, "{}", fact.name);
            assert_eq!(fact.source, None, "{}", fact.name);
            assert_eq!(fact.fetched, None, "{}", fact.name);
        }
    }

    #[test]
    fn rows_follow_the_signal_order_and_stars_never_count() {
        let sources = FactSources::new(&app(), Some(&repo()), day(2), every_date());
        let facts = signal_facts(&full(EntryKind::App), &sources, Some(321));
        let names: Vec<&str> = facts.iter().map(|fact| fact.name.as_str()).collect();
        assert_eq!(
            names[..4],
            [
                "first_release",
                "latest_version",
                "release_count",
                "last_release"
            ]
        );
        assert_eq!(names[names.len() - 2..], ["docker_pull_baseline", "stars"]);
        let stars = value(&facts, "stars");
        assert_eq!(stars.value.as_deref(), Some("321"));
        assert!(!stars.counts_toward_tier);
        assert!(
            facts
                .iter()
                .filter(|fact| fact.name != "stars")
                .all(|fact| fact.counts_toward_tier)
        );
    }

    #[test]
    fn weighted_amounts_lose_only_float_noise() {
        let signals = Signals {
            active_users: Some(2.9999999999999996),
            new_adopters: Some(289.00000000000006),
            peak_users: Some(38.357142857142854),
            former_glory: Some(0.9036144578313253),
            ..full(EntryKind::Library)
        };
        let sources = FactSources::new(&app(), Some(&repo()), day(2), every_date());
        let facts = signal_facts(&signals, &sources, None);
        let shown = |name: &str| value(&facts, name).value.clone();
        assert_eq!(shown("active_users").as_deref(), Some("3"));
        assert_eq!(shown("new_adopters").as_deref(), Some("289"));
        assert_eq!(shown("peak_users").as_deref(), Some("38.3571"));
        assert_eq!(shown("former_glory").as_deref(), Some("0.9036"));
    }

    #[test]
    fn each_row_cites_its_own_source_and_date() {
        let sources = FactSources::new(&app(), Some(&repo()), day(2), every_date());
        let facts = signal_facts(&full(EntryKind::App), &sources, Some(321));
        let cited = |name: &str| {
            let fact = value(&facts, name);
            let source = fact.source.as_ref().expect("source");
            (source.url.as_str().to_owned(), fact.fetched)
        };
        assert_eq!(cited("active_users"), (DUMP_PAGE.to_owned(), Some(day(2))));
        assert_eq!(
            value(&facts, "active_users")
                .source
                .as_ref()
                .map(|s| s.label.as_str()),
            Some(DUMP_SOURCE)
        );
        assert_eq!(
            cited("last_release"),
            (
                "https://github.com/ml-rust/tool/releases".to_owned(),
                Some(today())
            )
        );
        assert_eq!(cited("rustsec_unmaintained").1, Some(day(1)));
        assert_eq!(
            cited("build_toolchain"),
            (BUILD_CHECK_PAGE.to_owned(), Some(day(1)))
        );
        assert_eq!(
            cited("repo_gone"),
            ("https://github.com/ml-rust/tool".to_owned(), Some(today()))
        );
        assert_eq!(cited("repo_link_verified").1, Some(day(2)));
        assert_eq!(
            cited("homebrew_on_request_365d").0,
            "https://formulae.brew.sh/formula/python@3.12"
        );
        assert_eq!(
            cited("repology_families").0,
            "https://repology.org/project/tool/versions"
        );
        assert_eq!(
            cited("docker_pull_growth_90d"),
            (
                "https://hub.docker.com/r/ml-rust/tool".to_owned(),
                Some(today())
            )
        );
        let baseline = value(&facts, "docker_pull_baseline");
        assert_eq!(baseline.value.as_deref(), Some("1000"));
        assert_eq!(baseline.fetched, Date::new(2026, 8, 1).ok());
    }

    #[test]
    fn a_library_cites_last_release_from_the_dump() {
        let entry = library(&["main", "helper"]);
        let sources = FactSources::new(&entry, Some(&repo()), day(2), every_date());
        let facts = signal_facts(&full(EntryKind::Library), &sources, None);
        let release = value(&facts, "last_release");
        assert_eq!(
            release.source.as_ref().map(|s| s.url.as_str()),
            Some(DUMP_PAGE)
        );
        assert_eq!(value(&facts, "stars").value, None);
    }

    #[test]
    fn findings_read_none_once_their_source_was_read() {
        let sources = FactSources::new(&library(&["main"]), None, day(2), every_date());
        let mut signals = Signals::unknown(EntryKind::Library);
        signals.unpatched_vulnerabilities = Some(Vec::new());
        signals.crate_deleted = Some(false);
        let facts = signal_facts(&signals, &sources, None);
        for name in [
            "rustsec_unmaintained",
            "unpatched_vulnerabilities",
            "deprecation_notice",
            "deprecated_requirement",
        ] {
            assert_eq!(value(&facts, name).value.as_deref(), Some("none"), "{name}");
        }
        let unread = signal_facts(&Signals::unknown(EntryKind::Library), &sources, None);
        assert_eq!(value(&unread, "rustsec_unmaintained").value, None);
        assert_eq!(value(&unread, "deprecation_notice").value, None);
    }

    #[test]
    fn a_missing_date_shows_as_a_problem() {
        let sources = FactSources::new(&library(&["main"]), None, day(2), FetchDates::default());
        let mut signals = Signals::unknown(EntryKind::Library);
        signals.unpatched_vulnerabilities = Some(Vec::new());
        let facts = signal_facts(&signals, &sources, None);
        let problems = value(&facts, "unpatched_vulnerabilities").problems();
        assert_eq!(
            problems,
            ["signal `unpatched_vulnerabilities` has a value but no fetch date"]
        );
    }
}
