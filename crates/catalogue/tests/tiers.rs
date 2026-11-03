#[path = "common/link_transport.rs"]
mod link_transport;

use std::fs;
use std::path::{Path, PathBuf};

use catalogue::{Network, load_catalogue, tiers_report};
use catalogue_core::Date;
use catalogue_signals::dump::{
    CrateRow, DependencyKind, DumpMeta, DumpSnapshot, IdMap, Requirement, VersionRow,
};
use catalogue_signals::rustsec::{Advisories, Advisory, AdvisoryKind};
use jiff::Timestamp;
use link_transport::{PACKAGE_URL, Router};

/// Path of the fixture data directory named `case`.
fn fixture(case: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(case)
}

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn crate_row(name: &str) -> CrateRow {
    CrateRow {
        name: name.to_owned(),
        created_at: at("2025-01-01T00:00:00Z"),
        updated_at: at("2026-09-01T00:00:00Z"),
        description: None,
        homepage: None,
        repository: None,
        deprecation: None,
        has_readme: true,
    }
}

fn version(crate_id: u32, num: &str, created_at: &str) -> VersionRow {
    VersionRow {
        crate_id,
        num: num.to_owned(),
        created_at: at(created_at),
        yanked: false,
        rust_version: None,
        edition: None,
        published_by: None,
    }
}

/// `alpha-crate` with two releases and a requirement on `tokio 0.1`, which the curated list deprecates. `beta-crate` is absent.
fn snapshot() -> DumpSnapshot {
    let crates: IdMap<CrateRow> = [(1, crate_row("alpha-crate")), (2, crate_row("tokio"))]
        .into_iter()
        .collect();
    let versions: IdMap<VersionRow> = [
        (10, version(1, "1.2.0", "2025-01-01T12:00:00Z")),
        (11, version(1, "1.3.0", "2026-09-01T12:00:00Z")),
        (20, version(2, "0.1.22", "2019-01-01T12:00:00Z")),
    ]
    .into_iter()
    .collect();
    let requirement = Requirement {
        req: "0.1".to_owned(),
        crate_id: 2,
        kind: DependencyKind::Normal,
        optional: false,
    };
    DumpSnapshot {
        meta: DumpMeta {
            timestamp: at("2026-11-02T02:00:00Z"),
            crates_io_commit: String::new(),
            directory: String::new(),
        },
        crates,
        versions,
        default_version: [(1, 11), (2, 20)].into_iter().collect(),
        edges: Vec::new(),
        default_requirements: [(1, vec![requirement])].into_iter().collect(),
        owners: IdMap::default(),
        users: IdMap::default(),
        teams: IdMap::default(),
        categories: IdMap::default(),
        keywords: IdMap::default(),
        deleted: Vec::new(),
        downloads: IdMap::default(),
        warnings: Default::default(),
    }
}

fn block<'a>(report: &'a str, slug: &str) -> &'a str {
    report
        .split("\n\n")
        .find(|block| block.starts_with(&format!("slug: {slug}\n")))
        .unwrap_or_else(|| panic!("no block for {slug} in:\n{report}"))
}

#[test]
fn offline_report_assigns_every_fixture_entry_a_tier() {
    let catalogue = load_catalogue(&fixture("valid"))
        .expect("fixture reads")
        .expect("fixture is valid");
    let today = Date::new(2026, 11, 3).expect("date");
    let report = tiers_report(
        &catalogue,
        &snapshot(),
        &Advisories::default(),
        Network::Offline,
        today,
    )
    .expect("report");
    assert!(
        report.starts_with("entries: 3\n\nslug: alpha\n"),
        "{report}"
    );
    let slugs: Vec<&str> = report
        .lines()
        .filter_map(|line| line.strip_prefix("slug: "))
        .collect();
    assert_eq!(slugs, ["alpha", "beta", "gamma"]);

    let alpha = block(&report, "alpha");
    assert!(alpha.contains("\ntier: new\n"), "{alpha}");
    assert!(
        alpha.contains(
            "\n  rot: deprecated requirement `tokio 0.1 (https://example.com/tokio-deprecation)`\n"
        ),
        "{alpha}"
    );
    assert!(
        alpha.contains("\n  new: new: 0 active users < 2\n"),
        "{alpha}"
    );
    assert!(alpha.contains("\ndemotion_pending: -\n"), "{alpha}");
    assert!(alpha.contains("repo_gone"), "{alpha}");
    assert!(alpha.ends_with("\nnotes: -"), "{alpha}");

    let beta = block(&report, "beta");
    assert!(beta.contains("\ntier: listed\n"), "{beta}");
    assert!(beta.contains("first_release"), "{beta}");
    assert!(
        beta.ends_with("\nnotes: crate beta-crate is not in the dump"),
        "{beta}"
    );

    let gamma = block(&report, "gamma");
    assert!(gamma.contains("\ntier: listed\n"), "{gamma}");
    assert!(gamma.contains("binary_downloads_last3"), "{gamma}");
    assert!(gamma.ends_with("\nnotes: -"), "{gamma}");
}

#[test]
fn an_unmaintained_advisory_on_a_dependency_fires_the_deprecated_requirement_rule() {
    let catalogue = load_catalogue(&fixture("valid"))
        .expect("fixture reads")
        .expect("fixture is valid");
    let today = Date::new(2026, 11, 3).expect("date");
    let mut dump = snapshot();
    for requirements in dump.default_requirements.values_mut() {
        for requirement in requirements {
            requirement.req = "1".to_owned();
        }
    }
    let clean = tiers_report(
        &catalogue,
        &dump,
        &Advisories::default(),
        Network::Offline,
        today,
    )
    .expect("report");
    assert!(
        !block(&clean, "alpha").contains("deprecated requirement"),
        "{clean}"
    );

    let advisory = Advisory {
        id: "RUSTSEC-2026-0001".to_owned(),
        package: "tokio".to_owned(),
        date: jiff::civil::date(2026, 1, 1),
        kind: AdvisoryKind::Unmaintained,
        withdrawn: None,
        patched: Vec::new(),
        unaffected: Vec::new(),
        url: None,
    };
    let mut advisories = Advisories::default();
    advisories
        .by_crate
        .insert("tokio".to_owned(), vec![advisory]);
    let report =
        tiers_report(&catalogue, &dump, &advisories, Network::Offline, today).expect("report");
    let alpha = block(&report, "alpha");
    assert!(
        alpha.contains(
            "\n  rot: deprecated requirement `tokio (RUSTSEC-2026-0001, https://rustsec.org/advisories/RUSTSEC-2026-0001.html)`\n"
        ),
        "{alpha}"
    );
}

#[test]
fn an_online_run_checks_the_link_of_each_library_with_a_release_and_caches_it() {
    let catalogue = load_catalogue(&fixture("valid"))
        .expect("fixture reads")
        .expect("fixture is valid");
    let today = Date::new(2026, 11, 3).expect("date");
    let cache_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("tiers-links");
    let _ = fs::remove_dir_all(&cache_dir);
    let router = Router::default();
    let network = Network::Online {
        transport: &router,
        token: "sekret-xyz",
        cache_dir: &cache_dir,
    };
    let report = tiers_report(
        &catalogue,
        &snapshot(),
        &Advisories::default(),
        network,
        today,
    )
    .expect("report");
    let alpha = block(&report, "alpha");
    assert!(alpha.contains(".cargo_vcs_info.json"), "{alpha}");
    assert!(!block(&report, "beta").contains(".cargo_vcs_info.json"));
    let package_requests = router
        .urls()
        .iter()
        .filter(|url| *url == PACKAGE_URL)
        .count();
    assert_eq!(package_requests, 1);
    assert!(
        cache_dir
            .join("links")
            .join("alpha-crate-1.3.0@ml-rust__alpha.json")
            .is_file()
    );
    let _ = fs::remove_dir_all(&cache_dir);
}
