//! Computes adoption signals over a hand-built snapshot through the public API.

use catalogue_signals::dump::{
    CrateRow, DependencyEdge, DependencyKind, DumpMeta, DumpSnapshot, IdMap, IdSet, Owner,
    OwnerKind, VersionRow, utc_date,
};
use catalogue_signals::usage::{self, Usage};
use jiff::Timestamp;
use jiff::civil::{Date, date};

const NUMR: u32 = 1;
const APPLE: u32 = 2;
const PEAR: u32 = 3;
const MINE: u32 = 4;
const BUS: u32 = 5;
const GONE: u32 = 6;
const BOBS: u32 = 7;

fn at(text: &str) -> Timestamp {
    format!("{text}T12:00:00Z").parse().expect("timestamp")
}

fn crate_row(name: &str) -> CrateRow {
    CrateRow {
        name: name.to_owned(),
        created_at: at("2024-01-01"),
        updated_at: at("2024-01-01"),
        description: None,
        homepage: None,
        repository: None,
        deprecation: None,
    }
}

fn version(crate_id: u32, num: &str, day: &str, yanked: bool, by: Option<u32>) -> VersionRow {
    VersionRow {
        crate_id,
        num: num.to_owned(),
        created_at: at(day),
        yanked,
        rust_version: None,
        edition: None,
        published_by: by,
    }
}

fn users(ids: &[u32]) -> Vec<Owner> {
    ids.iter()
        .map(|&id| Owner {
            id,
            kind: OwnerKind::User,
        })
        .collect()
}

fn edge(version_id: u32, kind: DependencyKind) -> DependencyEdge {
    DependencyEdge {
        version_id,
        crate_id: NUMR,
        kind,
        optional: kind == DependencyKind::Dev,
    }
}

/// `numr` and its dependers:
///
/// - `apple`: three stable releases that all use `numr`, one owner.
/// - `pear`: one `0.1.0` dev-dependency release, a user and a team owner.
/// - `mine`: owned by `numr`'s owner.
/// - `bus`: owned by `rust-bus`.
/// - `bobs`: owned by a user who published a `numr` version.
/// - `gone`: dropped `numr`, then a yanked release uses it again.
fn snapshot() -> DumpSnapshot {
    let crates: IdMap<CrateRow> = [
        (NUMR, "numr"),
        (APPLE, "apple"),
        (PEAR, "pear"),
        (MINE, "mine"),
        (BUS, "bus"),
        (GONE, "gone"),
        (BOBS, "bobs"),
    ]
    .into_iter()
    .map(|(id, name)| (id, crate_row(name)))
    .collect();
    let versions: IdMap<VersionRow> = [
        (100, version(NUMR, "1.0.0", "2024-01-01", false, Some(8))),
        (101, version(NUMR, "1.1.0", "2024-02-01", false, Some(7))),
        (200, version(APPLE, "1.0.0", "2025-06-10", false, None)),
        (201, version(APPLE, "1.1.0", "2025-12-01", false, None)),
        (202, version(APPLE, "1.2.0", "2026-02-01", false, None)),
        (300, version(PEAR, "0.1.0", "2026-01-15", false, None)),
        (400, version(MINE, "1.0.0", "2025-01-01", false, None)),
        (500, version(BUS, "1.0.0", "2026-02-10", false, None)),
        (600, version(GONE, "1.0.0", "2025-03-05", false, None)),
        (601, version(GONE, "1.1.0", "2025-08-20", false, None)),
        (602, version(GONE, "1.2.0", "2025-09-01", true, None)),
        (700, version(BOBS, "1.0.0", "2026-02-10", false, None)),
    ]
    .into_iter()
    .collect();
    let edges = vec![
        edge(101, DependencyKind::Normal),
        edge(200, DependencyKind::Normal),
        edge(201, DependencyKind::Build),
        edge(202, DependencyKind::Normal),
        edge(300, DependencyKind::Dev),
        edge(400, DependencyKind::Normal),
        edge(500, DependencyKind::Normal),
        edge(600, DependencyKind::Normal),
        edge(602, DependencyKind::Normal),
        edge(700, DependencyKind::Normal),
    ];
    let mut pear = users(&[21]);
    pear.push(Owner {
        id: 30,
        kind: OwnerKind::Team,
    });
    let owners: IdMap<Vec<Owner>> = [
        (NUMR, users(&[7])),
        (APPLE, users(&[20])),
        (PEAR, pear),
        (MINE, users(&[7])),
        (BUS, users(&[99])),
        (GONE, users(&[22])),
        (BOBS, users(&[8])),
    ]
    .into_iter()
    .collect();
    let logins: IdMap<String> = [
        (7, "alice"),
        (8, "bob"),
        (20, "apple-dev"),
        (21, "pear-dev"),
        (22, "gone-dev"),
        (99, "rust-bus"),
    ]
    .into_iter()
    .map(|(id, login)| (id, login.to_owned()))
    .collect();
    DumpSnapshot {
        meta: DumpMeta {
            timestamp: "2026-03-02T02:00:17Z".parse().expect("timestamp"),
            crates_io_commit: "abc".to_owned(),
            directory: "2026-03-02-020017".to_owned(),
        },
        crates,
        versions,
        default_version: IdMap::default(),
        edges,
        default_requirements: IdMap::default(),
        owners,
        users: logins,
        teams: [(30, "github:pear:core".to_owned())].into_iter().collect(),
        categories: IdMap::default(),
        keywords: IdMap::default(),
        deleted: Vec::new(),
        downloads: IdMap::default(),
        warnings: Default::default(),
    }
}

fn compute(targets: &[u32]) -> IdMap<Usage> {
    let snapshot = snapshot();
    let targets: IdSet = targets.iter().copied().collect();
    usage::compute(&snapshot, &targets, utc_date(snapshot.meta.timestamp))
}

#[test]
fn counts_external_users_end_to_end() {
    let found = compute(&[NUMR]);
    let numr = found.get(&NUMR).expect("numr");
    assert_eq!(numr.current, 2.0, "apple and pear; the rest are excluded");
    assert_eq!(numr.new_adopters, 2.0);
    assert_eq!(numr.peak, 2.0);
    assert_eq!(numr.former_glory, 1.0);
    assert_eq!(
        (numr.removed, numr.expired),
        (1, 1),
        "gone removed, mine expired"
    );
    assert!(!numr.lost_users());
    assert_eq!(numr.growth, (2.0 + 2.0 + 25.0) / (1.0 + 1.0 + 25.0));
}

#[test]
fn history_runs_from_the_first_use_to_the_dump_date() {
    let found = compute(&[NUMR]);
    let history = &found.get(&NUMR).expect("numr").history;
    let expected: Vec<(Date, f64)> = vec![
        (date(2025, 1, 31), 0.0),
        (date(2025, 2, 28), 0.0),
        (date(2025, 3, 31), 1.0),
        (date(2025, 4, 30), 1.0),
        (date(2025, 5, 31), 1.0),
        (date(2025, 6, 30), 2.0),
        (date(2025, 7, 31), 2.0),
        (date(2025, 8, 31), 1.0),
        (date(2025, 9, 30), 1.0),
        (date(2025, 10, 31), 1.0),
        (date(2025, 11, 30), 1.0),
        (date(2025, 12, 31), 1.0),
        (date(2026, 1, 31), 2.0),
        (date(2026, 2, 28), 2.0),
        (date(2026, 3, 2), 2.0),
    ];
    assert_eq!(history, &expected);
}

#[test]
fn a_crate_without_dependers_gets_neutral_signals() {
    let found = compute(&[APPLE]);
    let apple = found.get(&APPLE).expect("apple");
    assert_eq!(
        (apple.current, apple.peak, apple.former_glory, apple.growth),
        (0.0, 0.0, 1.0, 1.0)
    );
    assert!(apple.history.is_empty());
}

#[test]
fn unknown_targets_are_skipped() {
    let found = compute(&[NUMR, 999]);
    assert_eq!(found.len(), 1);
    assert!(found.contains_key(&NUMR));
}

#[test]
fn results_are_bit_identical_across_runs() {
    assert_eq!(compute(&[NUMR, APPLE]), compute(&[APPLE, NUMR]));
}
