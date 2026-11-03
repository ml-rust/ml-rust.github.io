#[path = "common/docker_transport.rs"]
mod docker_transport;
#[path = "common/tiers_dump.rs"]
mod tiers_dump;

use std::fs;
use std::path::{Path, PathBuf};

use catalogue::{Network, load_catalogue, tiers_report};
use catalogue_core::{Catalogue, Date, EntryKind, Tier};
use catalogue_signals::history::{Row, Snapshot, read_all, write};
use catalogue_signals::rustsec::Advisories;
use catalogue_signals::sandbox::BuildChecks;
use docker_transport::DockerRouter;
use tiers_dump::{fixture, snapshot};

fn date(year: i16, month: i8, day: i8) -> Date {
    Date::new(year, month, day).expect("date")
}

fn catalogue() -> Catalogue {
    load_catalogue(&fixture("valid"))
        .expect("fixture reads")
        .expect("fixture is valid")
}

/// An empty history directory under the build directory.
fn history_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn row(kind: EntryKind, tier: Tier, docker_pulls: Option<u64>) -> Row {
    Row {
        kind,
        tier,
        dead_now: false,
        dormant: None,
        active_users: None,
        docker_pulls,
    }
}

fn seed(dir: &Path, on: Date, rows: &[(&str, Row)]) {
    let rows = rows.iter().map(|(slug, row)| ((*slug).to_owned(), *row));
    write(
        dir,
        &Snapshot {
            date: on,
            rows: rows.collect(),
        },
    )
    .expect("seed snapshot");
}

fn report(dir: &Path, network: Network<'_>, today: Date) -> String {
    tiers_report(
        &catalogue(),
        &snapshot(),
        &Advisories::default(),
        &BuildChecks::default(),
        network,
        Some(dir),
        today,
    )
    .expect("report")
    .text
}

fn block<'a>(report: &'a str, slug: &str) -> &'a str {
    report
        .split("\n\n")
        .find(|block| block.starts_with(&format!("slug: {slug}\n")))
        .unwrap_or_else(|| panic!("no block for {slug} in:\n{report}"))
}

#[test]
fn a_first_run_writes_the_snapshot_of_the_day() {
    let dir = history_dir("history-first-run");
    let cache_dir = history_dir("history-first-run-cache");
    let router = DockerRouter::new(5_000);
    let network = Network::Online {
        transport: &router,
        token: "sekret-xyz",
        cache_dir: &cache_dir,
    };
    let text = report(&dir, network, date(2026, 11, 3));
    assert!(text.contains("slug: gamma\n"), "{text}");
    assert!(dir.join("2026-11-03.jsonl.gz").is_file());
    let history = read_all(&dir).expect("history reads");
    let [written] = history.snapshots() else {
        panic!("expected one snapshot: {:?}", history.snapshots());
    };
    assert_eq!(written.date, date(2026, 11, 3));
    let rows: Vec<(&str, Option<u64>)> = written
        .rows
        .iter()
        .map(|(slug, row)| (slug.as_str(), row.docker_pulls))
        .collect();
    assert_eq!(
        rows,
        [("alpha", None), ("beta", None), ("gamma", Some(5_000))]
    );
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&cache_dir);
}

#[test]
fn a_later_run_reads_the_snapshot_and_sets_the_previous_tier() {
    let dir = history_dir("history-later-run");
    seed(
        &dir,
        date(2026, 10, 3),
        &[("beta", row(EntryKind::Library, Tier::Featured, None))],
    );
    let first = report(&dir, Network::Offline, date(2026, 11, 3));
    let beta = block(&first, "beta");
    assert!(beta.contains("\ntier: featured\n"), "{beta}");
    assert!(beta.contains("\n  demotion-pending: "), "{beta}");
    assert!(!block(&first, "alpha").contains("demotion-pending"));
    let written = read_all(&dir).expect("history reads");
    assert_eq!(written.snapshots().len(), 2);
    assert_eq!(
        written.previous_tier("beta", date(2026, 11, 4)),
        Some(Tier::Featured)
    );
    let again = report(&dir, Network::Offline, date(2026, 11, 3));
    assert_eq!(again, first);
    assert_eq!(read_all(&dir).expect("history reads").snapshots().len(), 2);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_fallen_pull_count_notes_zero_growth_against_the_baseline() {
    let dir = history_dir("history-docker");
    let cache_dir = history_dir("history-docker-cache");
    seed(
        &dir,
        date(2026, 7, 26),
        &[("gamma", row(EntryKind::App, Tier::Listed, Some(100_000)))],
    );
    let router = DockerRouter::new(5_000);
    let network = Network::Online {
        transport: &router,
        token: "sekret-xyz",
        cache_dir: &cache_dir,
    };
    let text = report(&dir, network, date(2026, 11, 3));
    let gamma = block(&text, "gamma");
    assert!(
        gamma.contains("Docker pulls fell from 100000 on 2026-07-26 to 5000: growth reads as 0"),
        "{gamma}"
    );
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&cache_dir);
}
