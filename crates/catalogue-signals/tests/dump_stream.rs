//! Reads in-memory `db-dump.tar.gz` archives through the public API.

mod common;

use catalogue_core::deprecation::Field;
use catalogue_signals::dump::{
    self, DependencyEdge, DependencyKind, DumpError, DumpSnapshot, Owner, OwnerKind, Requirement,
};
use jiff::{Timestamp, civil::date};

use common::{DIRECTORY, valid};

const WINDOW_DAYS: u16 = 3;

fn snapshot() -> DumpSnapshot {
    dump::read(valid().build().as_slice(), WINDOW_DAYS).expect("valid dump")
}

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn error_of(builder: common::DumpBuilder) -> DumpError {
    dump::read(builder.build().as_slice(), WINDOW_DAYS).expect_err("invalid dump")
}

#[test]
fn reads_metadata() {
    let meta = snapshot().meta;
    assert_eq!(meta.timestamp, at("2026-03-02T02:00:17.123456Z"));
    assert_eq!(meta.crates_io_commit, "0123abc");
    assert_eq!(meta.directory, DIRECTORY);
}

#[test]
fn reads_crates_with_multi_line_readmes_and_null_fields() {
    let snapshot = snapshot();
    assert_eq!(snapshot.crates.len(), 3);
    let numr = snapshot.crates.get(&1).expect("numr");
    assert_eq!(numr.name, "numr");
    assert_eq!(numr.created_at, at("2025-01-01T10:00:00.5Z"));
    assert_eq!(numr.updated_at, at("2026-02-01T00:00:00Z"));
    assert_eq!(numr.description.as_deref(), Some("Tensors, FFT and linalg"));
    assert_eq!(numr.homepage, None, "NULL homepage");
    assert_eq!(numr.repository, None, "empty-string repository");
    assert_eq!(numr.deprecation, None);
    let helper = snapshot.crates.get(&3).expect("helper");
    assert_eq!(helper.description, None, "empty-string description");
}

#[test]
fn finds_a_readme_deprecation_notice() {
    let snapshot = snapshot();
    let old = snapshot.crates.get(&2).expect("oldcrate");
    assert_eq!(old.homepage.as_deref(), Some("https://old.example"));
    let notice = old.deprecation.as_ref().expect("notice");
    assert_eq!(notice.field, Field::Readme);
    assert_eq!(notice.phrase, "> This crate is no longer maintained.");
}

#[test]
fn reads_versions_and_defaults() {
    let snapshot = snapshot();
    assert_eq!(snapshot.versions.len(), 4);
    let first = snapshot.versions.get(&10).expect("10");
    assert_eq!((first.crate_id, first.num.as_str()), (1, "0.1.0"));
    assert_eq!(first.rust_version.as_deref(), Some("1.80"));
    assert_eq!(first.edition, Some(2021));
    let second = snapshot.versions.get(&11).expect("11");
    assert_eq!(second.rust_version, None);
    let yanked = snapshot.versions.get(&20).expect("20");
    assert!(yanked.yanked);
    assert!(!first.yanked);
    assert_eq!(yanked.edition, None);
    assert_eq!(first.published_by, Some(7));
    let helper = snapshot.versions.get(&30).expect("30");
    assert_eq!(helper.published_by, None, "NULL publisher");
    assert_eq!(snapshot.default_version.get(&1), Some(&11));
    assert_eq!(snapshot.default_version.len(), 3);
}

#[test]
fn keeps_every_edge_and_the_default_version_requirements() {
    let snapshot = snapshot();
    assert_eq!(snapshot.edges.len(), 4);
    assert!(snapshot.edges.contains(&DependencyEdge {
        version_id: 30,
        crate_id: 2,
        kind: DependencyKind::Dev,
        optional: true,
    }));
    assert_eq!(
        snapshot.default_requirements.get(&3).map(Vec::as_slice),
        Some(
            &[
                Requirement {
                    req: "^0.2".to_owned(),
                    crate_id: 1,
                    kind: DependencyKind::Normal,
                    optional: false,
                },
                Requirement {
                    req: "=1.0.0".to_owned(),
                    crate_id: 2,
                    kind: DependencyKind::Dev,
                    optional: true,
                },
            ][..]
        )
    );
    assert_eq!(
        snapshot.default_requirements.get(&1).map(Vec::as_slice),
        Some(
            &[Requirement {
                req: "1".to_owned(),
                crate_id: 2,
                kind: DependencyKind::Build,
                optional: false,
            }][..]
        )
    );
    assert_eq!(snapshot.default_requirements.get(&2), None);
}

#[test]
fn reads_owners_and_logins() {
    let snapshot = snapshot();
    assert_eq!(
        snapshot.owners.get(&1).map(Vec::as_slice),
        Some(
            &[
                Owner {
                    id: 7,
                    kind: OwnerKind::User
                },
                Owner {
                    id: 3,
                    kind: OwnerKind::Team
                },
            ][..]
        )
    );
    assert_eq!(snapshot.users.get(&7).map(String::as_str), Some("alice"));
    assert_eq!(
        snapshot.teams.get(&3).map(String::as_str),
        Some("github:ml-rust:core")
    );
}

#[test]
fn resolves_categories_and_keywords() {
    let snapshot = snapshot();
    assert_eq!(
        snapshot.categories.get(&1).map(Vec::as_slice),
        Some(&["science::ml".to_owned(), "science".to_owned()][..])
    );
    assert_eq!(
        snapshot.keywords.get(&1).map(Vec::as_slice),
        Some(&["tensor".to_owned()][..])
    );
    assert_eq!(snapshot.categories.get(&2), None);
    assert_eq!(snapshot.warnings.unknown_category, 1);
    assert_eq!(snapshot.warnings.unknown_keyword, 1);
}

#[test]
fn reads_deleted_crates() {
    let deleted = snapshot().deleted;
    assert_eq!(deleted.len(), 1);
    let gone = deleted.first().expect("gone");
    assert_eq!(gone.name, "gone");
    assert_eq!(gone.deleted_at, at("2026-02-01T12:00:00Z"));
}

#[test]
fn sums_downloads_inside_the_window_only() {
    let snapshot = snapshot();
    let numr = snapshot.downloads.get(&1).expect("numr");
    assert_eq!(numr.start, date(2026, 2, 28));
    assert_eq!(numr.days, [1, 0, 12]);
    let helper = snapshot.downloads.get(&3).expect("helper");
    assert_eq!(helper.days, [0, 2, 0]);
    assert_eq!(snapshot.downloads.get(&2), None);
    assert_eq!(snapshot.warnings.unknown_download_version, 1);
}

#[test]
fn zero_day_window_keeps_no_downloads() {
    let snapshot = dump::read(valid().build().as_slice(), 0).expect("valid dump");
    assert!(snapshot.downloads.is_empty());
    assert_eq!(snapshot.warnings.unknown_download_version, 0);
}

#[test]
fn missing_column_names_table_and_column() {
    let versions = "id,crate_id,num,created_at,rust_version,edition\n";
    let error = error_of(valid().table("versions", versions));
    assert!(
        matches!(
            error,
            DumpError::MissingColumn {
                table: "versions",
                column: "yanked"
            }
        ),
        "{error}"
    );
}

#[test]
fn missing_table_is_named() {
    let error = error_of(valid().without("data/teams.csv"));
    assert!(
        matches!(error, DumpError::MissingTable { table: "teams" }),
        "{error}"
    );
    assert!(error.to_string().contains("data/teams.csv"), "{error}");
}

#[test]
fn missing_metadata_is_an_error() {
    let error = error_of(valid().without("metadata.json"));
    assert!(
        matches!(
            error,
            DumpError::OrderViolation {
                needs: "metadata.json",
                ..
            }
        ),
        "{error}"
    );
    let error = error_of(valid().moved_last("metadata.json"));
    assert!(
        matches!(
            error,
            DumpError::OrderViolation {
                needs: "metadata.json",
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn bad_boolean_names_table_column_and_record() {
    let versions = "id,crate_id,num,created_at,yanked,rust_version,edition,published_by\n\
        10,1,0.1.0,2025-01-01 10:00:00+00,f,,,\n\
        11,1,0.2.0,2026-02-01 00:00:00+00,maybe,,,\n";
    let error = error_of(valid().table("versions", versions));
    match &error {
        DumpError::BadValue {
            table,
            column,
            record,
            value,
            ..
        } => assert_eq!(
            (*table, *column, *record, value.as_str()),
            ("versions", "yanked", 2, "maybe")
        ),
        other => panic!("unexpected error: {other}"),
    }
    assert!(error.to_string().contains("`maybe`"), "{error}");
}

#[test]
fn dependencies_before_default_versions_is_an_order_violation() {
    let error = error_of(valid().moved_last("data/default_versions.csv"));
    assert!(
        matches!(
            error,
            DumpError::OrderViolation {
                table: "dependencies",
                needs: "default_versions"
            }
        ),
        "{error}"
    );
}

#[test]
fn repeated_table_is_rejected() {
    let error = error_of(valid().appended("data/teams.csv", "id,login\n"));
    assert!(
        matches!(error, DumpError::DuplicateTable { table: "teams" }),
        "{error}"
    );
}

#[test]
fn input_that_is_not_gzip_is_an_io_error() {
    let error = dump::read(b"plain text".as_slice(), WINDOW_DAYS).expect_err("rejects");
    assert!(matches!(error, DumpError::Io { .. }), "{error}");
}
