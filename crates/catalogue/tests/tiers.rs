#[path = "common/link_transport.rs"]
mod link_transport;
#[path = "common/tiers_dump.rs"]
mod tiers_dump;

use std::fs;
use std::path::Path;

use catalogue::{Network, load_catalogue, tiers_report};
use catalogue_core::Date;
use catalogue_signals::rustsec::{Advisories, Advisory, AdvisoryKind};
use catalogue_signals::sandbox::BuildChecks;
use link_transport::{PACKAGE_URL, Router};
use tiers_dump::{fixture, snapshot};

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
        &BuildChecks::default(),
        Network::Offline,
        None,
        today,
    )
    .expect("report")
    .text;
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
    // An active entry lists no unknown rot fact: rot counts only when dormant.
    assert!(!alpha.contains("repo_gone"), "{alpha}");
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
        &BuildChecks::default(),
        Network::Offline,
        None,
        today,
    )
    .expect("report")
    .text;
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
    let report = tiers_report(
        &catalogue,
        &dump,
        &advisories,
        &BuildChecks::default(),
        Network::Offline,
        None,
        today,
    )
    .expect("report")
    .text;
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
        &BuildChecks::default(),
        network,
        None,
        today,
    )
    .expect("report")
    .text;
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
