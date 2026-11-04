//! The dump and the fixture data directory of the `catalogue tiers` tests.

use std::path::PathBuf;

use catalogue_signals::dump::{
    CrateRow, DependencyKind, DumpMeta, DumpSnapshot, IdMap, Requirement, VersionRow,
};
use jiff::Timestamp;

/// Path of the fixture data directory named `case`.
pub fn fixture(case: &str) -> PathBuf {
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
        has_lib: Some(true),
        has_bin: false,
    }
}

/// `alpha-crate` with two releases and a requirement on `tokio 0.1`, which the curated list deprecates. `beta-crate` is absent.
pub fn snapshot() -> DumpSnapshot {
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
