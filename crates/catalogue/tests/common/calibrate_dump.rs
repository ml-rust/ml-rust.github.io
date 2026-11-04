//! The in-memory dump of the `catalogue calibrate` tests.

use catalogue_signals::discover::Rules;
use catalogue_signals::dump::{
    CrateRow, DependencyEdge, DependencyKind, DumpMeta, DumpSnapshot, IdMap, Owner, OwnerKind,
    VersionRow,
};
use jiff::Timestamp;

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

fn version(crate_id: u32, num: &str, day: &str, yanked: bool, by: Option<u32>) -> VersionRow {
    VersionRow {
        crate_id,
        num: num.to_owned(),
        created_at: at(&format!("{day}T12:00:00Z")),
        yanked,
        rust_version: None,
        edition: None,
        published_by: by,
        has_lib: Some(true),
        has_bin: false,
    }
}

fn uses(version_id: u32, crate_id: u32) -> DependencyEdge {
    DependencyEdge {
        version_id,
        crate_id,
        kind: DependencyKind::Normal,
        optional: false,
    }
}

fn owner(id: u32) -> Vec<Owner> {
    vec![Owner {
        id,
        kind: OwnerKind::User,
    }]
}

/// Rules that select the crates with the keyword `llm` or `ml`.
pub fn rules() -> Rules {
    Rules {
        keywords: vec!["llm".to_owned(), "ml".to_owned()],
        ..Rules::default()
    }
}

/// Rules that select no crate of the dump.
pub fn rules_without_match() -> Rules {
    Rules {
        keywords: vec!["absent".to_owned()],
        ..Rules::default()
    }
}

/// A dump taken on 2026-11-02 with three candidates:
///
/// - `core-lib` (keyword `llm`): one unyanked and one yanked release, used by `app-a`, `app-b` and `app-c`.
/// - `other` (keyword `ml`): two unyanked releases, used by `app-a`.
/// - `lonely` (keyword `llm`): one release and no user.
pub fn snapshot() -> DumpSnapshot {
    let crates: IdMap<CrateRow> = [
        (1, "core-lib"),
        (2, "app-a"),
        (3, "app-b"),
        (4, "app-c"),
        (5, "other"),
        (6, "lonely"),
    ]
    .into_iter()
    .map(|(id, name)| (id, crate_row(name)))
    .collect();
    let versions: IdMap<VersionRow> = [
        (10, version(1, "1.0.0", "2025-01-01", false, Some(7))),
        (11, version(1, "1.1.0", "2025-02-01", true, Some(7))),
        (20, version(2, "1.0.0", "2026-10-01", false, None)),
        (30, version(3, "1.0.0", "2026-10-01", false, None)),
        (40, version(4, "1.0.0", "2026-10-01", false, None)),
        (50, version(5, "1.0.0", "2025-01-01", false, Some(8))),
        (51, version(5, "1.1.0", "2025-06-01", false, Some(8))),
        (60, version(6, "1.0.0", "2025-01-01", false, Some(9))),
    ]
    .into_iter()
    .collect();
    let edges = vec![uses(20, 1), uses(30, 1), uses(40, 1), uses(20, 5)];
    let owners: IdMap<Vec<Owner>> = [
        (1, owner(7)),
        (2, owner(20)),
        (3, owner(21)),
        (4, owner(22)),
        (5, owner(8)),
        (6, owner(9)),
    ]
    .into_iter()
    .collect();
    let keywords: IdMap<Vec<String>> = [
        (1, vec!["llm".to_owned()]),
        (5, vec!["ml".to_owned()]),
        (6, vec!["llm".to_owned()]),
    ]
    .into_iter()
    .collect();
    DumpSnapshot {
        meta: DumpMeta {
            timestamp: at("2026-11-02T02:00:00Z"),
            crates_io_commit: String::new(),
            directory: String::new(),
        },
        crates,
        versions,
        default_version: [(1, 10), (5, 51), (6, 60)].into_iter().collect(),
        edges,
        default_requirements: IdMap::default(),
        details: IdMap::default(),
        owners,
        users: IdMap::default(),
        teams: IdMap::default(),
        categories: IdMap::default(),
        keywords,
        deleted: Vec::new(),
        downloads: IdMap::default(),
        warnings: Default::default(),
    }
}
