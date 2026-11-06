#[path = "common/fallback_transport.rs"]
mod fallback_transport;
#[path = "common/tiers_dump.rs"]
mod tiers_dump;

use std::fs;
use std::path::Path;

use catalogue::{Network, TiersError, TiersRun, load_catalogue, tiers_report};
use catalogue_core::{Date, EntryKind, EntryReport, SignalFact, Slug, Tier};
use catalogue_signals::fallback::Source;
use catalogue_signals::github::{Contributors, RepoLicense};
use catalogue_signals::history::{
    Dated, NetworkValues, ReleaseValues, RepoValues, Row, Snapshot, read_all, write,
};
use catalogue_signals::link::LinkCheck;
use catalogue_signals::rustsec::Advisories;
use catalogue_signals::sandbox::BuildChecks;
use fallback_transport::{
    DOCKER, HOMEBREW, PACKAGE, RELEASES, REPOLOGY, Rules, answer, found_repo,
};
use tempfile::TempDir;
use tiers_dump::{fixture, snapshot};

fn day(month: i8, on: i8) -> Date {
    Date::new(2026, month, on).expect("date")
}

fn today() -> Date {
    day(11, 3)
}

/// A history and a cache directory under the build directory.
struct Dirs {
    guard: TempDir,
}

impl Dirs {
    fn new() -> Self {
        Self {
            guard: TempDir::new_in(env!("CARGO_TARGET_TMPDIR")).expect("temp dir"),
        }
    }

    fn history(&self) -> std::path::PathBuf {
        self.guard.path().join("history")
    }
}

fn dated<T>(date: Date, of: &str, value: T) -> Option<Dated<T>> {
    Some(Dated {
        date,
        of: of.to_owned(),
        value,
    })
}

fn archived_facts(date: Date, of: &str) -> NetworkValues {
    NetworkValues {
        facts: dated(
            date,
            of,
            RepoValues::Found {
                archived: true,
                stars: 12,
                forks: Some(3),
                commits: Some(450),
                last_human_commit: Some(day(9, 1)),
                oldest_unanswered_days: Some(3),
                maintainer_replies_365: 7,
                license: RepoLicense::Stated("MIT".to_owned()),
                topics: Vec::new(),
                pushed_at: None,
            },
        ),
        ..NetworkValues::default()
    }
}

fn row(kind: EntryKind, network: Option<NetworkValues>) -> Row {
    Row {
        kind,
        tier: Tier::Listed,
        dead_now: false,
        dormant: None,
        active_users: None,
        docker_pulls: None,
        network,
        reused: false,
        fetch_notes: Vec::new(),
    }
}

fn seed(dir: &Path, on: Date, rows: Vec<(&str, Row)>) {
    let rows = rows
        .into_iter()
        .map(|(slug, row)| (slug.to_owned(), row))
        .collect();
    write(dir, &Snapshot { date: on, rows }).expect("seed");
}

fn run(dirs: &Dirs, rules: &Rules) -> Result<TiersRun, TiersError> {
    let catalogue = load_catalogue(&fixture("valid"))
        .expect("fixture reads")
        .expect("fixture is valid");
    let cache_dir = dirs.guard.path().join("cache");
    let network = Network::Online {
        transport: rules,
        token: "sekret-xyz",
        cache_dir: &cache_dir,
    };
    let history = dirs.history();
    tiers_report(
        &catalogue,
        &snapshot(),
        &Advisories::default(),
        &BuildChecks::default(),
        network,
        Some(&history),
        today(),
    )
}

fn entry<'a>(run: &'a TiersRun, slug: &str) -> &'a EntryReport {
    let slug = Slug::new(slug).expect("slug");
    run.report.entries.get(&slug).expect("entry")
}

fn fact<'a>(report: &'a EntryReport, name: &str) -> &'a SignalFact {
    report
        .signals
        .iter()
        .find(|fact| fact.name == name)
        .unwrap_or_else(|| panic!("no fact {name}"))
}

fn has_note(report: &EntryReport, part: &str) -> bool {
    report.notes.iter().any(|note| note.contains(part))
}

fn todays_row(dirs: &Dirs, slug: &str) -> Row {
    let history = read_all(&dirs.history()).expect("history reads");
    let snapshot = history
        .snapshots()
        .iter()
        .find(|snapshot| snapshot.date == today())
        .expect("snapshot of the day");
    snapshot.rows.get(slug).expect("row").clone()
}

#[test]
fn a_500_on_one_repo_falls_back_with_the_old_date_and_a_note() {
    let dirs = Dirs::new();
    let old = day(10, 30);
    seed(
        &dirs.history(),
        day(11, 2),
        vec![(
            "alpha",
            row(
                EntryKind::Library,
                Some(archived_facts(old, "ml-rust/alpha")),
            ),
        )],
    );
    let rules = Rules::default()
        .repo("alpha", answer(500, ""))
        .repo("beta", answer(200, &found_repo("beta", 40)));
    let found = run(&dirs, &rules).expect("the run goes on");

    let alpha = entry(&found, "alpha");
    let archived = fact(alpha, "repo_archived");
    assert_eq!(archived.value.as_deref(), Some("true"));
    assert_eq!(archived.fetched, Some(old));
    assert_eq!(fact(alpha, "last_human_commit").fetched, Some(old));
    let stars = fact(alpha, "stars");
    assert_eq!(stars.value.as_deref(), Some("12"));
    assert_eq!(stars.fetched, Some(old));
    assert!(stars.counts_toward_tier);
    assert_eq!(fact(alpha, "forks").value.as_deref(), Some("3"));
    assert_eq!(fact(alpha, "forks").fetched, Some(old));
    assert!(fact(alpha, "forks").counts_toward_tier);
    assert_eq!(fact(alpha, "commits").value.as_deref(), Some("450"));
    assert!(!fact(alpha, "commits").counts_toward_tier);
    assert!(
        has_note(
            alpha,
            "GitHub repo facts failed: using the value from 2026-10-30."
        ),
        "{:?}",
        alpha.notes
    );

    let beta = entry(&found, "beta");
    assert_eq!(fact(beta, "stars").value.as_deref(), Some("40"));
    assert_eq!(fact(beta, "stars").fetched, Some(today()));

    assert_eq!(found.summary.fallback_count(), 1);
    assert_eq!(found.summary.failures[0].source, Source::RepoFacts);
    assert!(
        found.text.contains(
            "fallbacks:\n  alpha: GitHub repo facts (from 2026-10-30)\nfallback count: 1 of "
        ),
        "{}",
        found.text
    );
    assert!(!found.text.contains("WARNING"), "{}", found.text);

    let written = todays_row(&dirs, "alpha");
    let facts = written.network.expect("network").facts.expect("facts");
    assert_eq!(facts.date, old);
    assert_eq!(facts.of, "ml-rust/alpha");
}

#[test]
fn a_gone_repo_stays_gone_and_never_falls_back() {
    let dirs = Dirs::new();
    seed(
        &dirs.history(),
        day(11, 2),
        vec![(
            "alpha",
            row(
                EntryKind::Library,
                Some(archived_facts(day(11, 2), "ml-rust/alpha")),
            ),
        )],
    );
    let found = run(&dirs, &Rules::default()).expect("runs");
    let alpha = entry(&found, "alpha");
    let gone = fact(alpha, "repo_gone");
    assert_eq!(gone.value.as_deref(), Some("true"));
    assert_eq!(gone.fetched, Some(today()));
    assert_eq!(fact(alpha, "repo_archived").value, None);
    assert_eq!(fact(alpha, "stars").value, None);
    assert_eq!(found.summary.fallback_count(), 0);
    assert!(
        found.text.contains("fallbacks: -\nfallback count: 0 of "),
        "{}",
        found.text
    );
    let written = todays_row(&dirs, "alpha");
    let facts = written.network.expect("network").facts.expect("facts");
    assert_eq!(facts.value, RepoValues::Gone);
    assert_eq!(facts.date, today());
}

#[test]
fn a_404_on_a_distribution_name_never_falls_back() {
    let dirs = Dirs::new();
    let network = NetworkValues {
        homebrew: dated(day(11, 2), "gamma", 640),
        ..NetworkValues::default()
    };
    seed(
        &dirs.history(),
        day(11, 2),
        vec![("gamma", row(EntryKind::App, Some(network)))],
    );
    let found = run(&dirs, &Rules::default()).expect("runs");
    let gamma = entry(&found, "gamma");
    assert_eq!(fact(gamma, "homebrew_on_request_365d").value, None);
    assert!(
        has_note(gamma, "Homebrew has no formula gamma"),
        "{:?}",
        gamma.notes
    );
    assert_eq!(found.summary.fallback_count(), 0);
    let written = todays_row(&dirs, "gamma");
    assert_eq!(written.network.and_then(|values| values.homebrew), None);
}

#[test]
fn a_rejected_token_ends_the_run_and_writes_nothing() {
    let dirs = Dirs::new();
    let error = run(&dirs, &Rules::everything(401)).expect_err("a 401 ends the run");
    assert!(matches!(error, TiersError::Github(_)), "{error}");
    assert_eq!(error.exit_code(), 1);
    assert!(read_all(&dirs.history()).expect("reads").is_empty());
}

#[test]
fn a_rejected_token_on_the_releases_ends_the_run() {
    let dirs = Dirs::new();
    let rules = Rules::default().url(RELEASES, answer(401, ""));
    let error = run(&dirs, &rules).expect_err("a 401 ends the run");
    assert!(matches!(error, TiersError::Distribution(_)), "{error}");
    assert_eq!(error.exit_code(), 1);
}

#[test]
fn a_401_from_a_source_with_no_token_falls_back() {
    let dirs = Dirs::new();
    let network = NetworkValues {
        docker: dated(day(11, 2), "ml-rust/gamma", 5_000),
        ..NetworkValues::default()
    };
    seed(
        &dirs.history(),
        day(11, 2),
        vec![("gamma", row(EntryKind::App, Some(network)))],
    );
    let rules = Rules::default().url(DOCKER, answer(401, ""));
    let found = run(&dirs, &rules).expect("the run goes on");
    assert_eq!(found.summary.fallback_count(), 1);
    assert!(
        found.text.contains("gamma: Docker Hub (from 2026-11-02)"),
        "{}",
        found.text
    );
}

#[test]
fn the_age_limit_counts_from_the_real_fetch() {
    let dirs = Dirs::new();
    let rules = Rules::default().repo("alpha", answer(500, ""));
    let at_limit = day(10, 20);
    seed(
        &dirs.history(),
        day(11, 2),
        vec![(
            "alpha",
            row(
                EntryKind::Library,
                Some(archived_facts(at_limit, "ml-rust/alpha")),
            ),
        )],
    );
    let found = run(&dirs, &rules).expect("runs");
    assert_eq!(
        fact(entry(&found, "alpha"), "repo_archived").fetched,
        Some(at_limit)
    );

    let dirs = Dirs::new();
    seed(
        &dirs.history(),
        day(11, 2),
        vec![(
            "alpha",
            row(
                EntryKind::Library,
                Some(archived_facts(day(10, 19), "ml-rust/alpha")),
            ),
        )],
    );
    let found = run(&dirs, &rules).expect("runs");
    let alpha = entry(&found, "alpha");
    assert_eq!(fact(alpha, "repo_archived").value, None);
    assert_eq!(fact(alpha, "repo_archived").fetched, None);
    assert!(
        has_note(
            alpha,
            "the value from 2026-10-19 is older than 14 days: signal unknown"
        ),
        "{:?}",
        alpha.notes
    );
    assert_eq!(found.summary.fallback_count(), 0);
    assert!(
        found
            .text
            .contains("failed with no usable earlier value:\n  alpha: GitHub repo facts"),
        "{}",
        found.text
    );
    let written = todays_row(&dirs, "alpha");
    assert_eq!(written.network.and_then(|values| values.facts), None);
}

#[test]
fn a_snapshot_without_the_field_reads_as_unknown_with_a_note() {
    let dirs = Dirs::new();
    seed(
        &dirs.history(),
        day(11, 2),
        vec![("alpha", row(EntryKind::Library, None))],
    );
    let rules = Rules::default().repo("alpha", answer(500, ""));
    let found = run(&dirs, &rules).expect("runs");
    let alpha = entry(&found, "alpha");
    assert_eq!(fact(alpha, "repo_archived").value, None);
    assert!(
        has_note(
            alpha,
            "GitHub repo facts failed and no earlier snapshot holds a value: signal unknown"
        ),
        "{:?}",
        alpha.notes
    );
}

#[test]
fn a_rerun_on_the_same_day_reads_only_earlier_snapshots() {
    let dirs = Dirs::new();
    seed(
        &dirs.history(),
        today(),
        vec![(
            "alpha",
            row(
                EntryKind::Library,
                Some(archived_facts(today(), "ml-rust/alpha")),
            ),
        )],
    );
    let rules = Rules::default().repo("alpha", answer(500, ""));
    let found = run(&dirs, &rules).expect("runs");
    assert_eq!(fact(entry(&found, "alpha"), "repo_archived").value, None);
}

#[test]
fn a_value_about_another_repo_is_no_stand_in() {
    let dirs = Dirs::new();
    seed(
        &dirs.history(),
        day(11, 2),
        vec![(
            "alpha",
            row(
                EntryKind::Library,
                Some(archived_facts(day(11, 2), "ml-rust/old-alpha")),
            ),
        )],
    );
    let rules = Rules::default().repo("alpha", answer(500, ""));
    let found = run(&dirs, &rules).expect("runs");
    let alpha = entry(&found, "alpha");
    assert_eq!(fact(alpha, "repo_archived").value, None);
    assert!(
        has_note(alpha, "describes ml-rust/old-alpha"),
        "{:?}",
        alpha.notes
    );
}

#[test]
fn every_distribution_source_falls_back_with_its_own_date() {
    let dirs = Dirs::new();
    let network = NetworkValues {
        releases: dated(
            day(11, 1),
            "ml-rust/gamma",
            ReleaseValues {
                binary_downloads: 900,
                releases: 3,
                first_release: None,
                last_release: Some(day(9, 1)),
            },
        ),
        homebrew: dated(day(10, 28), "gamma", 640),
        repology: dated(day(10, 29), "gamma", 2),
        docker: dated(day(10, 30), "ml-rust/gamma", 5_000),
        ..NetworkValues::default()
    };
    seed(
        &dirs.history(),
        day(11, 2),
        vec![("gamma", row(EntryKind::App, Some(network)))],
    );
    let rules = Rules::default()
        .url(RELEASES, answer(503, ""))
        .url(HOMEBREW, answer(500, ""))
        .url(REPOLOGY, answer(429, ""))
        .url(DOCKER, answer(500, ""));
    let found = run(&dirs, &rules).expect("the run goes on");
    let gamma = entry(&found, "gamma");
    let cited = |name: &str| {
        let row = fact(gamma, name);
        (row.value.clone(), row.fetched)
    };
    assert_eq!(
        cited("binary_downloads_last3"),
        (Some("900".to_owned()), Some(day(11, 1)))
    );
    assert_eq!(
        cited("binary_downloads_per_release"),
        (Some("300".to_owned()), Some(day(11, 1)))
    );
    assert_eq!(
        cited("homebrew_on_request_365d"),
        (Some("640".to_owned()), Some(day(10, 28)))
    );
    assert_eq!(
        cited("repology_families"),
        (Some("2".to_owned()), Some(day(10, 29)))
    );
    assert_eq!(found.summary.fallback_count(), 4);
    let written = todays_row(&dirs, "gamma");
    assert_eq!(written.docker_pulls, None, "a baseline needs a fresh count");
    let values = written.network.expect("network");
    assert_eq!(values.docker.expect("docker").date, day(10, 30));
    assert_eq!(values.homebrew.expect("homebrew").date, day(10, 28));
}

#[test]
fn docker_growth_measures_a_fallback_count_from_its_own_date() {
    let dirs = Dirs::new();
    let baseline = Row {
        docker_pulls: Some(1_000),
        ..row(EntryKind::App, None)
    };
    seed(&dirs.history(), day(7, 20), vec![("gamma", baseline)]);
    let network = NetworkValues {
        docker: dated(day(11, 1), "ml-rust/gamma", 5_000),
        ..NetworkValues::default()
    };
    seed(
        &dirs.history(),
        day(11, 2),
        vec![("gamma", row(EntryKind::App, Some(network)))],
    );
    let rules = Rules::default().url(DOCKER, answer(500, ""));
    let found = run(&dirs, &rules).expect("runs");
    let growth = fact(entry(&found, "gamma"), "docker_pull_growth_90d");
    assert_eq!(growth.value.as_deref(), Some("4000"));
    assert_eq!(growth.fetched, Some(day(11, 1)));
}

fn link_values(version: &str) -> NetworkValues {
    NetworkValues {
        link: dated(
            day(10, 25),
            &format!("alpha-crate {version} in ml-rust/alpha"),
            LinkCheck::NoVcsInfo,
        ),
        ..NetworkValues::default()
    }
}

#[test]
fn a_link_check_falls_back_for_the_same_release_only() {
    let rules = Rules::default().url(PACKAGE, answer(502, ""));
    let dirs = Dirs::new();
    seed(
        &dirs.history(),
        day(11, 2),
        vec![("alpha", row(EntryKind::Library, Some(link_values("1.3.0"))))],
    );
    let found = run(&dirs, &rules).expect("runs");
    let link = fact(entry(&found, "alpha"), "repo_link_verified");
    assert_eq!(link.value.as_deref(), Some("false"));
    assert_eq!(link.fetched, Some(day(10, 25)));
    assert!(
        found
            .text
            .contains("alpha: repo link check (from 2026-10-25)"),
        "{}",
        found.text
    );

    let dirs = Dirs::new();
    seed(
        &dirs.history(),
        day(11, 2),
        vec![("alpha", row(EntryKind::Library, Some(link_values("1.2.0"))))],
    );
    let found = run(&dirs, &rules).expect("runs");
    let alpha = entry(&found, "alpha");
    assert_eq!(fact(alpha, "repo_link_verified").value, None);
    assert!(
        has_note(alpha, "describes alpha-crate 1.2.0"),
        "{:?}",
        alpha.notes
    );
}

#[test]
fn a_missing_package_never_falls_back() {
    let dirs = Dirs::new();
    seed(
        &dirs.history(),
        day(11, 2),
        vec![("alpha", row(EntryKind::Library, Some(link_values("1.3.0"))))],
    );
    let found = run(&dirs, &Rules::default()).expect("runs");
    let alpha = entry(&found, "alpha");
    assert_eq!(fact(alpha, "repo_link_verified").value, None);
    assert!(
        has_note(alpha, "repo link unchecked: "),
        "{:?}",
        alpha.notes
    );
    assert_eq!(found.summary.fallback_count(), 0);
}

#[test]
fn a_run_where_every_fetch_fails_stays_at_a_limit_of_one_and_succeeds() {
    let dirs = Dirs::new();
    seed(
        &dirs.history(),
        day(11, 2),
        vec![(
            "alpha",
            row(
                EntryKind::Library,
                Some(archived_facts(day(11, 2), "ml-rust/alpha")),
            ),
        )],
    );
    let found = run(&dirs, &Rules::everything(500)).expect("the fixture limit is 1");
    assert_eq!(found.summary.fallback_count(), 1);
    assert_eq!(found.summary.failures.len(), found.summary.fetches);
    assert!(!found.text.contains("WARNING"), "{}", found.text);
    assert!(dirs.history().join("2026-11-03.jsonl.gz").is_file());
}

/// The contributor listing of `ml-rust/beta`.
const BETA_CONTRIBUTORS: &str = "ml-rust/beta/contributors";

#[test]
fn a_contributor_count_joins_the_report_and_the_snapshot_and_counts_toward_the_tier() {
    let dirs = Dirs::new();
    let rules = Rules::default()
        .repo("beta", answer(200, &found_repo("beta", 40)))
        .url(BETA_CONTRIBUTORS, answer(200, r#"[{"login":"a"}]"#));
    let found = run(&dirs, &rules).expect("runs");
    let contributors = fact(entry(&found, "beta"), "contributors");
    assert_eq!(contributors.value.as_deref(), Some("1"));
    assert_eq!(contributors.fetched, Some(today()));
    assert!(contributors.counts_toward_tier);
    assert_eq!(
        contributors
            .source
            .as_ref()
            .map(|source| source.url.as_str()),
        Some("https://github.com/ml-rust/beta/graphs/contributors")
    );
    let written = todays_row(&dirs, "beta");
    let stored = written
        .network
        .expect("network")
        .contributors
        .expect("count");
    assert_eq!(stored.value, Contributors::Counted { count: 1 });
    assert_eq!(stored.of, "ml-rust/beta");
}

#[test]
fn a_failed_contributor_fetch_falls_back_with_the_old_date_and_a_note() {
    let dirs = Dirs::new();
    let old = day(10, 30);
    let network = NetworkValues {
        contributors: dated(old, "ml-rust/beta", Contributors::Counted { count: 38 }),
        ..NetworkValues::default()
    };
    seed(
        &dirs.history(),
        day(11, 2),
        vec![("beta", row(EntryKind::Library, Some(network)))],
    );
    let rules = Rules::default()
        .repo("beta", answer(200, &found_repo("beta", 40)))
        .url(BETA_CONTRIBUTORS, answer(500, ""));
    let found = run(&dirs, &rules).expect("the run goes on");
    let beta = entry(&found, "beta");
    let contributors = fact(beta, "contributors");
    assert_eq!(contributors.value.as_deref(), Some("38"));
    assert_eq!(contributors.fetched, Some(old));
    assert!(
        has_note(
            beta,
            "GitHub contributors failed: using the value from 2026-10-30."
        ),
        "{:?}",
        beta.notes
    );
    assert_eq!(found.summary.fallback_count(), 1);
    assert_eq!(found.summary.failures[0].source, Source::Contributors);
    assert!(
        found
            .text
            .contains("beta: GitHub contributors (from 2026-10-30)"),
        "{}",
        found.text
    );
    let written = todays_row(&dirs, "beta");
    let stored = written
        .network
        .expect("network")
        .contributors
        .expect("count");
    assert_eq!(stored.date, old, "the value keeps its first date");
}

#[test]
fn a_failed_contributor_fetch_with_no_earlier_value_reads_as_not_measured() {
    let dirs = Dirs::new();
    let rules = Rules::default()
        .repo("beta", answer(200, &found_repo("beta", 40)))
        .url(BETA_CONTRIBUTORS, answer(500, ""));
    let found = run(&dirs, &rules).expect("the run goes on");
    let contributors = fact(entry(&found, "beta"), "contributors");
    assert_eq!(contributors.value, None);
    assert_eq!(contributors.source, None);
    assert_eq!(found.summary.fallback_count(), 0);
    assert_eq!(found.summary.failures[0].source, Source::Contributors);
}

#[test]
fn a_list_too_large_reads_as_not_measured_with_a_note_and_no_fallback() {
    let dirs = Dirs::new();
    let old = day(11, 1);
    let network = NetworkValues {
        contributors: dated(old, "ml-rust/beta", Contributors::Counted { count: 38 }),
        ..NetworkValues::default()
    };
    seed(
        &dirs.history(),
        day(11, 2),
        vec![("beta", row(EntryKind::Library, Some(network)))],
    );
    let rules = Rules::default()
        .repo("beta", answer(200, &found_repo("beta", 40)))
        .url(
            BETA_CONTRIBUTORS,
            answer(
                403,
                r#"{"message":"The history or contributor list is too large"}"#,
            ),
        );
    let found = run(&dirs, &rules).expect("the run goes on");
    let beta = entry(&found, "beta");
    assert_eq!(fact(beta, "contributors").value, None);
    assert!(has_note(beta, "too large to read"), "{:?}", beta.notes);
    assert!(found.summary.failures.is_empty(), "{:?}", found.summary);
    let written = todays_row(&dirs, "beta");
    let stored = written
        .network
        .expect("network")
        .contributors
        .expect("answer");
    assert_eq!(stored.value, Contributors::TooLarge);
    assert_eq!(stored.date, today());
}

/// The README request of `ml-rust/beta`.
const BETA_README: &str = "ml-rust/beta/readme";

#[test]
fn a_readme_joins_the_report_and_stays_out_of_the_snapshot() {
    let dirs = Dirs::new();
    let rules = Rules::default()
        .repo("beta", answer(200, &found_repo("beta", 40)))
        .url(
            BETA_README,
            answer(
                200,
                r#"{"path":"docs/README.md","html_url":"https://github.com/ml-rust/beta/blob/dev/docs/README.md","encoding":"base64","content":"IyBB"}"#,
            ),
        );
    let found = run(&dirs, &rules).expect("runs");
    let readme = entry(&found, "beta").readme.as_ref().expect("a readme");
    assert_eq!(readme.text, "# A");
    assert_eq!(readme.path, "docs/README.md");
    assert_eq!(readme.branch, "dev");
    assert!(!readme.truncated);
    assert_eq!(readme.fetched, today());
    assert_eq!(
        readme.source.as_str(),
        "https://github.com/ml-rust/beta/blob/dev/docs/README.md"
    );
    assert_eq!(
        entry(&found, "gamma").readme,
        None,
        "a gone repo asks for none"
    );
    let text = serde_json::to_string(&found.report).expect("writes");
    let back: catalogue_core::TierReport = serde_json::from_str(&text).expect("reads");
    assert_eq!(back, found.report);
    let written = todays_row(&dirs, "beta");
    assert_eq!(written.network.expect("network").readme, None);
}

#[test]
fn a_repo_with_no_readme_has_none_and_no_note() {
    let dirs = Dirs::new();
    let rules = Rules::default().repo("beta", answer(200, &found_repo("beta", 40)));
    let found = run(&dirs, &rules).expect("runs");
    let beta = entry(&found, "beta");
    assert_eq!(beta.readme, None);
    assert!(!has_note(beta, "README"), "{:?}", beta.notes);
    assert!(found.summary.failures.is_empty(), "{:?}", found.summary);
}

#[test]
fn a_failed_readme_fetch_leaves_no_readme_and_the_run_goes_on() {
    let dirs = Dirs::new();
    let rules = Rules::default()
        .repo("beta", answer(200, &found_repo("beta", 40)))
        .url(BETA_README, answer(500, ""));
    let found = run(&dirs, &rules).expect("the run goes on");
    assert_eq!(entry(&found, "beta").readme, None);
    assert_eq!(found.summary.failures[0].source, Source::Readme);
}

#[test]
fn a_gone_repo_asks_for_no_contributors() {
    let dirs = Dirs::new();
    let found = run(&dirs, &Rules::default()).expect("runs");
    let alpha = entry(&found, "alpha");
    assert_eq!(fact(alpha, "contributors").value, None);
    assert!(!has_note(alpha, "contributors"), "{:?}", alpha.notes);
}

#[test]
fn a_rerun_with_the_same_answers_writes_the_same_bytes() {
    let dirs = Dirs::new();
    seed(
        &dirs.history(),
        day(11, 2),
        vec![(
            "alpha",
            row(
                EntryKind::Library,
                Some(archived_facts(day(10, 30), "ml-rust/alpha")),
            ),
        )],
    );
    let rules = Rules::default().repo("alpha", answer(500, ""));
    let path = dirs.history().join("2026-11-03.jsonl.gz");
    let first_run = run(&dirs, &rules).expect("first");
    let first = fs::read(&path).expect("reads");
    let second_run = run(&dirs, &rules).expect("second");
    let second = fs::read(&path).expect("reads");
    assert_eq!(first, second);
    assert_eq!(first_run.text, second_run.text);
}
