//! The `crates` and `deleted_crates` tables.

use std::io::Read;

use catalogue_core::deprecation;

use crate::dump::error::DumpError;
use crate::dump::names::TableName;
use crate::dump::snapshot::{CrateRow, DeletedCrate, IdMap};
use crate::dump::table::for_each_row;

const CRATES: &[&str] = &[
    "id",
    "name",
    "created_at",
    "updated_at",
    "description",
    "homepage",
    "repository",
    "readme",
];

const DELETED: &[&str] = &["name", "deleted_at"];

/// Reads `crates`. The README feeds the deprecation scan and is then dropped.
pub(crate) fn read_crates(input: impl Read) -> Result<IdMap<CrateRow>, DumpError> {
    let mut out = IdMap::default();
    for_each_row(input, TableName::Crates, CRATES, |row| {
        let description = row.optional_text("description")?;
        let readme = row.optional_text("readme")?;
        let crate_row = CrateRow {
            name: row.text("name")?.to_owned(),
            created_at: row.timestamp("created_at")?,
            updated_at: row.timestamp("updated_at")?,
            description: description.map(str::to_owned),
            homepage: row.optional_string("homepage")?,
            repository: row.optional_string("repository")?,
            deprecation: deprecation::scan(description, readme),
        };
        out.insert(row.id("id")?, crate_row);
        Ok(())
    })?;
    Ok(out)
}

/// Reads `deleted_crates`.
pub(crate) fn read_deleted(input: impl Read) -> Result<Vec<DeletedCrate>, DumpError> {
    let mut out = Vec::new();
    for_each_row(input, TableName::DeletedCrates, DELETED, |row| {
        out.push(DeletedCrate {
            name: row.text("name")?.to_owned(),
            deleted_at: row.timestamp("deleted_at")?,
        });
        Ok(())
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use catalogue_core::deprecation::Field;
    use jiff::Timestamp;

    use super::*;

    fn at(text: &str) -> Timestamp {
        text.parse().expect("timestamp")
    }

    #[test]
    fn reads_crates_and_scans_the_readme() {
        let csv = "readme,id,name,updated_at,created_at,description,homepage,repository,downloads\n\
            \"# foo\n\nThis crate is deprecated, \"\"really\"\".\",1,foo,2026-01-02 00:00:00+00,2025-01-01 10:00:00.5+00,A parser,,\"\",9\n\
            ,2,bar,2026-01-02 00:00:00+00,2026-01-01 00:00:00+00,Deprecated,https://bar.rs,https://github.com/x/bar,0\n";
        let crates = read_crates(csv.as_bytes()).expect("reads");
        let foo = crates.get(&1).expect("foo");
        assert_eq!(foo.name, "foo");
        assert_eq!(foo.created_at, at("2025-01-01T10:00:00.5Z"));
        assert_eq!(foo.description.as_deref(), Some("A parser"));
        assert_eq!(foo.homepage, None);
        assert_eq!(foo.repository, None);
        let notice = foo.deprecation.as_ref().expect("readme notice");
        assert_eq!(notice.field, Field::Readme);
        assert_eq!(notice.phrase, "This crate is deprecated, \"really\".");
        let bar = crates.get(&2).expect("bar");
        assert_eq!(bar.repository.as_deref(), Some("https://github.com/x/bar"));
        let notice = bar.deprecation.as_ref().expect("description notice");
        assert_eq!(notice.field, Field::Description);
    }

    #[test]
    fn crate_without_notice() {
        let csv = "id,name,created_at,updated_at,description,homepage,repository,readme\n\
            3,baz,2026-01-01 00:00:00+00,2026-01-01 00:00:00+00,Tensors,,,\"# baz\"\n";
        let crates = read_crates(csv.as_bytes()).expect("reads");
        assert_eq!(crates.get(&3).and_then(|c| c.deprecation.as_ref()), None);
    }

    #[test]
    fn bad_timestamp_is_rejected() {
        let csv = "id,name,created_at,updated_at,description,homepage,repository,readme\n\
            3,baz,yesterday,2026-01-01 00:00:00+00,,,,\n";
        let error = read_crates(csv.as_bytes()).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::BadValue {
                table: "crates",
                column: "created_at",
                record: 1,
                ..
            }
        ));
    }

    #[test]
    fn reads_deleted_crates() {
        let csv = "id,name,deleted_at,message\n1,gone,2026-03-04 05:06:07+00,\n";
        let deleted = read_deleted(csv.as_bytes()).expect("reads");
        assert_eq!(
            deleted,
            [DeletedCrate {
                name: "gone".to_owned(),
                deleted_at: at("2026-03-04T05:06:07Z"),
            }]
        );
    }
}
