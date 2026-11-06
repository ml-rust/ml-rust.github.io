//! `catalogue tiers --fetch`: what each mode asks for, and what a run that reuses values records.

#[path = "common/fallback_transport.rs"]
mod fallback_transport;
#[path = "common/tiers_dump.rs"]
mod tiers_dump;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use catalogue::{
    FetchMode, Network, TiersError, TiersInputs, TiersRun, load_catalogue, tiers_report_with,
};
use catalogue_core::{Catalogue, Date, EntryKind, EntryReport, Slug, Tier};
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::github::{Contributors, RepoLicense};
use catalogue_signals::history::{
    Dated, NetworkValues, ReadmeRecord, ReadmeStore, RepoValues, Row, Snapshot, read_all, write,
};
use catalogue_signals::http::{HttpError, Response, Transport};
use catalogue_signals::link::{LinkCheck, RepoReadme};
use catalogue_signals::rustsec::Advisories;
use catalogue_signals::sandbox::BuildChecks;
use fallback_transport::{
    DOCKER, HOMEBREW, PACKAGE, RELEASES, REPOLOGY, Rules, answer, found_repo,
};
use tempfile::TempDir;
use tiers_dump::{fixture, snapshot};

/// The push time every stored fact holds, unless a test says otherwise.
const PUSHED: &str = "2026-10-20T10:00:00Z";
/// The link subject of the release the dump shows as latest.
const ALPHA_LINK: &str = "alpha-crate 1.3.0 in ml-rust/alpha";

fn day(month: i8, on: i8) -> Date {
    Date::new(2026, month, on).expect("date")
}

fn today() -> Date {
    day(11, 3)
}

/// Records each request, then answers by `rules`.
struct Recording {
    rules: Rules,
    seen: RefCell<Vec<String>>,
}

impl Recording {
    fn new(rules: Rules) -> Self {
        Self {
            rules,
            seen: RefCell::new(Vec::new()),
        }
    }

    fn asked(&self, part: &str) -> usize {
        let seen = self.seen.borrow();
        seen.iter().filter(|line| line.contains(part)).count()
    }

    fn total(&self) -> usize {
        self.seen.borrow().len()
    }
}

impl Transport for Recording {
    fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Response, HttpError> {
        self.seen.borrow_mut().push(url.to_owned());
        self.rules.get(url, headers)
    }

    fn post(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        let text = String::from_utf8_lossy(body).into_owned();
        self.seen.borrow_mut().push(text);
        self.rules.post(url, headers, body)
    }

    fn patch(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        self.rules.patch(url, headers, body)
    }

    fn delete(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        self.rules.delete(url, headers, body)
    }
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

    fn history(&self) -> PathBuf {
        self.guard.path().join("history")
    }
}

fn catalogue() -> Catalogue {
    load_catalogue(&fixture("valid"))
        .expect("fixture reads")
        .expect("fixture is valid")
}

fn run_on(
    dirs: &Dirs,
    fetch: FetchMode,
    transport: &dyn Transport,
    on: Date,
    dump: &DumpSnapshot,
) -> Result<TiersRun, TiersError> {
    let catalogue = catalogue();
    let advisories = Advisories::default();
    let build_checks = BuildChecks::default();
    let inputs = TiersInputs {
        catalogue: &catalogue,
        snapshot: dump,
        advisories: &advisories,
        build_checks: &build_checks,
        fetch,
        today: on,
    };
    let cache_dir = dirs.guard.path().join("cache");
    let network = Network::Online {
        transport,
        token: "sekret-xyz",
        cache_dir: &cache_dir,
    };
    tiers_report_with(&inputs, network, Some(&dirs.history()))
}

fn run(dirs: &Dirs, fetch: FetchMode, transport: &dyn Transport) -> Result<TiersRun, TiersError> {
    run_on(dirs, fetch, transport, today(), &snapshot())
}

fn entry<'a>(run: &'a TiersRun, slug: &str) -> &'a EntryReport {
    run.report
        .entries
        .get(&Slug::new(slug).expect("slug"))
        .expect("entry")
}

fn fetched_on(report: &EntryReport, name: &str) -> Option<Date> {
    report
        .signals
        .iter()
        .find(|fact| fact.name == name)
        .and_then(|fact| fact.fetched)
}

fn dated<T>(date: Date, of: &str, value: T) -> Option<Dated<T>> {
    Some(Dated {
        date,
        of: of.to_owned(),
        value,
    })
}

fn found_values(pushed: &str) -> RepoValues {
    RepoValues::Found {
        archived: false,
        stars: 12,
        forks: Some(3),
        commits: Some(450),
        last_human_commit: Some(day(9, 1)),
        oldest_unanswered_days: Some(3),
        maintainer_replies_365: 7,
        license: RepoLicense::Stated("MIT".to_owned()),
        topics: Vec::new(),
        pushed_at: Some(pushed.parse().expect("time")),
    }
}

/// The GraphQL answer of a repo with `pushed` as its last push.
fn repo_answer(name: &str, pushed: &str) -> fallback_transport::Answer {
    let body = format!(
        r#"{{"data":{{"repository":{{"nameWithOwner":"ml-rust/{name}","stargazerCount":40,"pushedAt":"{pushed}"}}}}}}"#
    );
    answer(200, &body)
}

/// The stored values of `alpha`, all dated `on`.
fn alpha_values(on: Date, link: &str) -> NetworkValues {
    NetworkValues {
        facts: dated(on, "ml-rust/alpha", found_values(PUSHED)),
        contributors: dated(on, "ml-rust/alpha", Contributors::Counted { count: 38 }),
        link: dated(on, link, LinkCheck::NoVcsInfo),
        ..NetworkValues::default()
    }
}

/// The stored values of `gamma`, all dated `on`.
fn gamma_values(on: Date) -> NetworkValues {
    NetworkValues {
        facts: dated(on, "ml-rust/gamma", found_values(PUSHED)),
        contributors: dated(on, "ml-rust/gamma", Contributors::Counted { count: 2 }),
        homebrew: dated(on, "gamma", 640),
        repology: dated(on, "gamma", 2),
        docker: dated(on, "ml-rust/gamma", 5_000),
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
    write(dir, &Snapshot { date: on, rows }).expect("writes");
}

fn readme(text: &str) -> RepoReadme {
    RepoReadme {
        text: text.to_owned(),
        path: "README.md".to_owned(),
        branch: "main".to_owned(),
        truncated: false,
        source: "https://github.com/ml-rust/alpha/blob/main/README.md"
            .parse()
            .expect("url"),
    }
}

fn seed_readme(dir: &Path, on: Date, slug: &str, of: &str, text: Option<&str>) {
    let mut store = ReadmeStore::read(dir).expect("reads");
    store.records.insert(
        slug.to_owned(),
        ReadmeRecord {
            date: on,
            of: of.to_owned(),
            readme: text.map(readme),
        },
    );
    store.write(dir).expect("writes");
}

/// A history where `alpha` and `gamma` have values from `on`, and `beta` has none.
fn seed_known(dirs: &Dirs, on: Date) {
    seed(
        &dirs.history(),
        on,
        vec![
            (
                "alpha",
                row(EntryKind::Library, Some(alpha_values(on, ALPHA_LINK))),
            ),
            ("gamma", row(EntryKind::App, Some(gamma_values(on)))),
        ],
    );
    seed_readme(&dirs.history(), on, "alpha", "ml-rust/alpha", Some("# A"));
    seed_readme(&dirs.history(), on, "gamma", "ml-rust/gamma", None);
}

fn row_of(dirs: &Dirs, on: Date, slug: &str) -> Row {
    let history = read_all(&dirs.history()).expect("history reads");
    let snapshot = history
        .snapshots()
        .iter()
        .find(|snapshot| snapshot.date == on)
        .expect("snapshot of the day");
    snapshot.rows.get(slug).expect("row").clone()
}

/// Answers for a quiet week: every known repo shows the stored push.
fn quiet() -> Recording {
    Recording::new(
        Rules::default()
            .repo("alpha", repo_answer("alpha", PUSHED))
            .repo("beta", repo_answer("beta", PUSHED))
            .repo("gamma", repo_answer("gamma", PUSHED)),
    )
}

#[test]
fn none_sends_no_request_and_keeps_every_true_date() {
    let dirs = Dirs::new();
    seed_known(&dirs, day(10, 25));
    let transport = quiet();
    let found = run(&dirs, FetchMode::None, &transport).expect("runs");
    assert_eq!(transport.total(), 0, "{:?}", transport.seen.borrow());
    assert_eq!(found.summary.fetches, 0);
    assert!(found.summary.failures.is_empty());
    let alpha = entry(&found, "alpha");
    assert_eq!(fetched_on(alpha, "stars"), Some(day(10, 25)));
    let readme = alpha.readme.as_ref().expect("the stored README");
    assert_eq!(readme.fetched, day(10, 25));
    let written = row_of(&dirs, today(), "alpha");
    assert!(written.reused);
    let values = written.network.expect("network");
    assert_eq!(values.facts.expect("facts").date, day(10, 25));
    assert_eq!(values.link.expect("link").date, day(10, 25));
    assert_eq!(found.report.generated, today());
}

#[test]
fn none_with_no_history_stops_and_names_the_next_step() {
    let dirs = Dirs::new();
    let error = run(&dirs, FetchMode::None, &quiet()).expect_err("no history");
    assert!(matches!(error, TiersError::NoStoredValues { .. }));
    assert!(error.to_string().contains("--fetch all"), "{error}");
}

#[test]
fn new_fetches_only_the_entry_with_no_stored_values() {
    let dirs = Dirs::new();
    seed_known(&dirs, day(10, 25));
    let transport = quiet();
    let found = run(&dirs, FetchMode::New, &transport).expect("runs");
    assert_eq!(transport.asked("\"beta\""), 1, "the facts of beta");
    assert_eq!(transport.asked("ml-rust/beta/contributors"), 1);
    assert_eq!(transport.asked("ml-rust/beta/readme"), 1);
    assert_eq!(transport.asked("alpha"), 0);
    assert_eq!(transport.asked("gamma"), 0);
    assert_eq!(fetched_on(entry(&found, "beta"), "stars"), Some(today()));
    assert_eq!(
        fetched_on(entry(&found, "alpha"), "stars"),
        Some(day(10, 25))
    );
    assert!(!row_of(&dirs, today(), "beta").reused);
    assert!(row_of(&dirs, today(), "alpha").reused);
    assert!(row_of(&dirs, today(), "gamma").reused);
    assert!(
        found.text.contains("asked about 1 of 3 entries"),
        "{}",
        found.text
    );
}

#[test]
fn a_repo_moved_in_the_data_counts_as_a_new_entry() {
    let dirs = Dirs::new();
    let values = NetworkValues {
        facts: dated(day(10, 25), "ml-rust/old-alpha", found_values(PUSHED)),
        ..NetworkValues::default()
    };
    seed(
        &dirs.history(),
        day(10, 25),
        vec![("alpha", row(EntryKind::Library, Some(values)))],
    );
    let transport = quiet();
    let found = run(&dirs, FetchMode::New, &transport).expect("runs");
    assert_eq!(transport.asked("\"alpha\""), 1);
    assert_eq!(fetched_on(entry(&found, "alpha"), "stars"), Some(today()));
}

#[test]
fn the_scan_of_a_quiet_week_sends_only_facts_and_distribution_requests() {
    let dirs = Dirs::new();
    seed_known(&dirs, day(10, 25));
    seed_readme(&dirs.history(), day(10, 25), "beta", "ml-rust/beta", None);
    let beta = NetworkValues {
        facts: dated(day(10, 25), "ml-rust/beta", found_values(PUSHED)),
        contributors: dated(
            day(10, 25),
            "ml-rust/beta",
            Contributors::Counted { count: 1 },
        ),
        ..NetworkValues::default()
    };
    let mut rows = read_all(&dirs.history())
        .expect("reads")
        .snapshots()
        .first()
        .expect("snapshot")
        .rows
        .clone();
    rows.insert("beta".to_owned(), row(EntryKind::Library, Some(beta)));
    write(
        &dirs.history(),
        &Snapshot {
            date: day(10, 25),
            rows,
        },
    )
    .expect("writes");
    let transport = quiet();
    let found = run(&dirs, FetchMode::Changed, &transport).expect("runs");
    for name in ["\"alpha\"", "\"beta\"", "\"gamma\""] {
        assert_eq!(transport.asked(name), 1, "the facts of {name}");
    }
    assert_eq!(transport.asked("/contributors"), 0);
    assert_eq!(transport.asked("/readme"), 0);
    assert_eq!(transport.asked(PACKAGE), 0);
    for source in [RELEASES, HOMEBREW, REPOLOGY, DOCKER] {
        assert_eq!(transport.asked(source), 1, "{source}");
    }
    let alpha = entry(&found, "alpha");
    assert_eq!(fetched_on(alpha, "stars"), Some(today()));
    assert_eq!(fetched_on(alpha, "contributors"), Some(day(10, 25)));
    let values = row_of(&dirs, today(), "alpha").network.expect("network");
    assert_eq!(values.link.expect("link").date, day(10, 25));
    assert_eq!(alpha.readme.as_ref().expect("readme").fetched, day(10, 25));
    assert!(
        !row_of(&dirs, today(), "alpha").reused,
        "the facts are a check"
    );
    assert!(found.summary.failures.is_empty());
}

#[test]
fn a_newer_push_refreshes_contributors_and_the_readme_of_that_repo_only() {
    let dirs = Dirs::new();
    seed_known(&dirs, day(10, 25));
    let transport = Recording::new(
        Rules::default()
            .repo("alpha", repo_answer("alpha", "2026-11-01T08:00:00Z"))
            .repo("gamma", repo_answer("gamma", PUSHED)),
    );
    run(&dirs, FetchMode::Changed, &transport).expect("runs");
    assert_eq!(transport.asked("ml-rust/alpha/contributors"), 1);
    assert_eq!(transport.asked("ml-rust/alpha/readme"), 1);
    assert_eq!(
        transport.asked(PACKAGE),
        0,
        "a push alone asks for no link check"
    );
    assert_eq!(transport.asked("ml-rust/gamma/contributors"), 0);
    assert_eq!(transport.asked("ml-rust/gamma/readme"), 0);
}

#[test]
fn a_new_version_in_the_dump_refreshes_the_on_update_sources() {
    let dirs = Dirs::new();
    let on = day(10, 25);
    let values = alpha_values(on, "alpha-crate 1.2.0 in ml-rust/alpha");
    seed(
        &dirs.history(),
        on,
        vec![("alpha", row(EntryKind::Library, Some(values)))],
    );
    seed_readme(&dirs.history(), on, "alpha", "ml-rust/alpha", Some("# A"));
    let transport = quiet();
    run(&dirs, FetchMode::Changed, &transport).expect("runs");
    assert_eq!(transport.asked(PACKAGE), 1);
    assert_eq!(transport.asked("ml-rust/alpha/contributors"), 1);
    assert_eq!(transport.asked("ml-rust/alpha/readme"), 1);
}

#[test]
fn a_new_or_none_run_keeps_a_link_check_of_an_older_version() {
    let dirs = Dirs::new();
    let on = day(10, 25);
    let values = alpha_values(on, "alpha-crate 1.2.0 in ml-rust/alpha");
    seed(
        &dirs.history(),
        on,
        vec![("alpha", row(EntryKind::Library, Some(values)))],
    );
    run(&dirs, FetchMode::None, &quiet()).expect("runs");
    let kept = row_of(&dirs, today(), "alpha").network.expect("network");
    let link = kept.link.expect("the stored check");
    assert_eq!(link.of, "alpha-crate 1.2.0 in ml-rust/alpha");
    assert_eq!(link.date, on);
}

#[test]
fn the_backstop_refreshes_a_value_past_its_own_limit_and_no_earlier() {
    // The offset of `alpha` is 19 days, so its limit is 49 days.
    let inside = day(9, 15);
    let dirs = Dirs::new();
    seed_known(&dirs, inside);
    let transport = quiet();
    run(&dirs, FetchMode::Changed, &transport).expect("runs");
    assert_eq!(transport.asked("ml-rust/alpha/contributors"), 0, "49 days");
    assert_eq!(transport.asked(PACKAGE), 0);
    let dirs = Dirs::new();
    seed_known(&dirs, day(9, 14));
    let transport = quiet();
    run(&dirs, FetchMode::Changed, &transport).expect("runs");
    assert_eq!(transport.asked("ml-rust/alpha/contributors"), 1, "50 days");
    assert_eq!(transport.asked("ml-rust/alpha/readme"), 1);
    assert_eq!(transport.asked(PACKAGE), 1);
}

#[test]
fn a_repo_with_no_readme_is_not_asked_again_until_a_reason_appears() {
    let dirs = Dirs::new();
    seed_known(&dirs, day(10, 25));
    let transport = quiet();
    run(&dirs, FetchMode::Changed, &transport).expect("runs");
    assert_eq!(transport.asked("ml-rust/gamma/readme"), 0);
}

#[test]
fn a_failed_refresh_keeps_an_on_update_value_past_the_fourteen_day_limit() {
    let dirs = Dirs::new();
    let on = day(10, 14);
    seed_known(&dirs, on);
    let transport = Recording::new(
        Rules::default()
            .repo("alpha", repo_answer("alpha", "2026-11-01T08:00:00Z"))
            .repo("gamma", repo_answer("gamma", PUSHED))
            .url("ml-rust/alpha/contributors", answer(500, ""))
            .url("ml-rust/alpha/readme", answer(500, "")),
    );
    let found = run(&dirs, FetchMode::Changed, &transport).expect("runs");
    let alpha = entry(&found, "alpha");
    assert_eq!(fetched_on(alpha, "contributors"), Some(on));
    assert_eq!(alpha.readme.as_ref().expect("readme").fetched, on);
    assert!(
        alpha
            .notes
            .iter()
            .any(|note| note.contains("using the value")),
        "{:?}",
        alpha.notes
    );
    let lost: Vec<_> = found
        .summary
        .failures
        .iter()
        .filter(|failure| failure.from.is_none())
        .collect();
    assert!(lost.is_empty(), "{lost:?}");
}

#[test]
fn reused_values_are_no_fetches_and_no_failures() {
    let dirs = Dirs::new();
    seed_known(&dirs, day(10, 25));
    let transport = Recording::new(Rules::everything(500));
    let found = run(&dirs, FetchMode::New, &transport).expect("runs");
    let slugs: Vec<&str> = found
        .summary
        .failures
        .iter()
        .map(|failure| failure.slug.as_str())
        .collect();
    assert!(!slugs.is_empty());
    assert!(slugs.iter().all(|slug| *slug == "beta"), "{slugs:?}");
    assert_eq!(
        found.summary.fetches,
        transport.total().min(found.summary.fetches)
    );
    assert!(found.summary.fetches <= 3, "{:?}", found.summary.fetches);
}

/// A date in 2029. The newest release of `alpha-crate` is dormant by then, so a gone repo makes it
/// dead.
fn dead_year(month: i8, on: i8) -> Date {
    Date::new(2029, month, on).expect("date")
}

fn gone(on: Date) -> NetworkValues {
    NetworkValues {
        facts: dated(on, "ml-rust/alpha", RepoValues::Gone),
        link: dated(on, ALPHA_LINK, LinkCheck::NoVcsInfo),
        ..NetworkValues::default()
    }
}

fn dead_row(on: Date) -> Row {
    Row {
        dead_now: true,
        ..row(EntryKind::Library, Some(gone(on)))
    }
}

#[test]
fn adding_a_project_does_not_archive_another_one() {
    let dirs = Dirs::new();
    let scan = dead_year(10, 28);
    seed(&dirs.history(), scan, vec![("alpha", dead_row(scan))]);
    let later = dead_year(11, 3);
    let found = run_on(&dirs, FetchMode::New, &quiet(), later, &snapshot()).expect("runs");
    let alpha = entry(&found, "alpha");
    assert_ne!(alpha.tier, Tier::Archived, "{:?}", alpha.reasons);
    let row = row_of(&dirs, later, "alpha");
    assert!(row.dead_now, "the rules still find it dead");
    assert!(row.reused, "the run did not check it");
    let again = run_on(
        &dirs,
        FetchMode::None,
        &quiet(),
        dead_year(12, 2),
        &snapshot(),
    )
    .expect("runs");
    assert_ne!(entry(&again, "alpha").tier, Tier::Archived);
}

#[test]
fn the_second_real_check_archives_as_before() {
    let dirs = Dirs::new();
    let scan = dead_year(10, 28);
    seed(&dirs.history(), scan, vec![("alpha", dead_row(scan))]);
    let later = dead_year(11, 3);
    let transport = Recording::new(Rules::default());
    let found = run_on(&dirs, FetchMode::Changed, &transport, later, &snapshot()).expect("runs");
    assert_eq!(entry(&found, "alpha").tier, Tier::Archived);
    assert!(!row_of(&dirs, later, "alpha").reused);
}

#[test]
fn a_run_that_reuses_values_keeps_an_archive_the_last_check_reached() {
    let dirs = Dirs::new();
    let first = dead_year(10, 28);
    let second = dead_year(11, 3);
    seed(&dirs.history(), first, vec![("alpha", dead_row(first))]);
    let transport = Recording::new(Rules::default());
    run_on(&dirs, FetchMode::Changed, &transport, second, &snapshot()).expect("runs");
    let found = run_on(
        &dirs,
        FetchMode::None,
        &quiet(),
        dead_year(11, 9),
        &snapshot(),
    )
    .expect("runs");
    assert_eq!(entry(&found, "alpha").tier, Tier::Archived);
}

#[test]
fn a_held_demotion_stays_held_across_runs_that_reuse_values() {
    let dirs = Dirs::new();
    let scan = dead_year(10, 28);
    let featured = Row {
        tier: Tier::Featured,
        ..row(EntryKind::Library, None)
    };
    seed(&dirs.history(), scan, vec![("beta", featured)]);
    for on in [dead_year(11, 3), dead_year(11, 4), dead_year(12, 2)] {
        let found = run_on(&dirs, FetchMode::None, &quiet(), on, &snapshot()).expect("runs");
        let beta = entry(&found, "beta");
        assert_eq!(beta.tier, Tier::Featured);
        assert!(beta.demotion_pending.is_some());
    }
}

#[test]
fn a_run_after_the_scan_on_the_same_day_keeps_what_the_scan_fetched() {
    let dirs = Dirs::new();
    seed_known(&dirs, day(10, 25));
    let transport = Recording::new(
        Rules::default()
            .repo("alpha", repo_answer("alpha", "2026-11-01T08:00:00Z"))
            .repo("beta", repo_answer("beta", PUSHED))
            .repo("gamma", repo_answer("gamma", PUSHED))
            .url(DOCKER, answer(200, r#"{"pull_count":7000}"#)),
    );
    let scan = run(&dirs, FetchMode::Changed, &transport).expect("the scan");
    let scanned = row_of(&dirs, today(), "gamma");
    assert_eq!(scanned.docker_pulls, Some(7_000));
    let after = run(&dirs, FetchMode::None, &quiet()).expect("the later run");
    assert_eq!(
        fetched_on(entry(&after, "alpha"), "stars"),
        fetched_on(entry(&scan, "alpha"), "stars")
    );
    assert_eq!(fetched_on(entry(&after, "alpha"), "stars"), Some(today()));
    let replaced = row_of(&dirs, today(), "gamma");
    assert!(!replaced.reused, "the day still holds a real check");
    assert_eq!(
        replaced.docker_pulls,
        Some(7_000),
        "still a growth baseline"
    );
    assert_eq!(replaced.network, scanned.network);
}

#[test]
fn a_rerun_of_a_none_run_writes_the_same_bytes() {
    let dirs = Dirs::new();
    seed_known(&dirs, day(10, 25));
    run(&dirs, FetchMode::None, &quiet()).expect("first");
    let path = dirs.history().join("2026-11-03.jsonl.gz");
    let first = fs::read(&path).expect("reads");
    run(&dirs, FetchMode::None, &quiet()).expect("second");
    assert_eq!(first, fs::read(&path).expect("reads"));
}

#[test]
fn a_fetch_all_run_writes_the_readme_store_for_later_runs() {
    let dirs = Dirs::new();
    let body = r#"{"path":"README.md","html_url":"https://github.com/ml-rust/beta/blob/dev/README.md","encoding":"base64","content":"IyBB"}"#;
    let transport = Recording::new(
        Rules::default()
            .repo("beta", answer(200, &found_repo("beta", 40)))
            .url("ml-rust/beta/readme", answer(200, body)),
    );
    run_on(&dirs, FetchMode::All, &transport, day(11, 1), &snapshot()).expect("runs");
    let quiet_run = run(&dirs, FetchMode::None, &quiet()).expect("runs");
    let readme = entry(&quiet_run, "beta").readme.as_ref().expect("readme");
    assert_eq!(readme.text, "# A");
    assert_eq!(readme.fetched, day(11, 1));
}

#[test]
fn a_reused_entry_keeps_its_fetch_notes() {
    let dirs = Dirs::new();
    let on = day(10, 25);
    let noted = Row {
        fetch_notes: vec!["Homebrew has no formula gamma".to_owned()],
        ..row(EntryKind::App, Some(gamma_values(on)))
    };
    seed(&dirs.history(), on, vec![("gamma", noted)]);
    let found = run(&dirs, FetchMode::None, &quiet()).expect("runs");
    let gamma = entry(&found, "gamma");
    assert!(
        gamma
            .notes
            .contains(&"Homebrew has no formula gamma".to_owned()),
        "{:?}",
        gamma.notes
    );
}

#[test]
fn a_stored_value_about_another_subject_is_dropped_with_a_note() {
    let dirs = Dirs::new();
    let on = day(10, 25);
    let values = NetworkValues {
        homebrew: dated(on, "old-gamma", 640),
        ..gamma_values(on)
    };
    seed(
        &dirs.history(),
        on,
        vec![("gamma", row(EntryKind::App, Some(values)))],
    );
    let found = run(&dirs, FetchMode::None, &quiet()).expect("runs");
    let gamma = entry(&found, "gamma");
    assert_eq!(fetched_on(gamma, "homebrew_on_request_365d"), None);
    assert!(
        gamma
            .notes
            .iter()
            .any(|note| note.contains("describes old-gamma")),
        "{:?}",
        gamma.notes
    );
}

#[test]
fn a_gone_repo_found_by_the_scan_drops_its_stored_count_and_readme() {
    let dirs = Dirs::new();
    seed_known(&dirs, day(10, 25));
    let transport = Recording::new(Rules::default().repo("gamma", repo_answer("gamma", PUSHED)));
    let found = run(&dirs, FetchMode::Changed, &transport).expect("runs");
    let alpha = entry(&found, "alpha");
    assert_eq!(fetched_on(alpha, "contributors"), None);
    assert_eq!(alpha.readme, None);
}
