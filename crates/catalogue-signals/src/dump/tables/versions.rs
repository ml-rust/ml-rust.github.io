//! The `versions` and `default_versions` tables.

use std::io::Read;

use crate::dump::error::DumpError;
use crate::dump::names::TableName;
use crate::dump::row::{Col, Row};
use crate::dump::snapshot::{IdMap, VersionRow};
use crate::dump::table::for_each_row;

const VERSIONS: &[&str; 7] = &[
    "id",
    "crate_id",
    "num",
    "created_at",
    "yanked",
    "rust_version",
    "edition",
];

const DEFAULT_VERSIONS: &[&str; 2] = &["crate_id", "version_id"];

/// Reads `versions`.
pub(crate) fn read_versions(input: impl Read) -> Result<IdMap<VersionRow>, DumpError> {
    let mut out = IdMap::default();
    for_each_row(
        input,
        TableName::Versions,
        VERSIONS,
        |row, &[id, crate_id, num, created_at, yanked, rust_version, edition]| {
            let version = VersionRow {
                crate_id: row.id(crate_id)?,
                num: row.text(num)?.to_owned(),
                created_at: row.timestamp(created_at)?,
                yanked: row.flag(yanked)?,
                rust_version: row.optional_string(rust_version)?,
                edition: year(row, edition)?,
            };
            out.insert(row.id(id)?, version);
            Ok(())
        },
    )?;
    Ok(out)
}

/// An optional four-digit edition year such as `2021`.
fn year(row: &Row<'_>, col: Col) -> Result<Option<u16>, DumpError> {
    let Some(text) = row.optional_text(col)? else {
        return Ok(None);
    };
    let digits = text.as_bytes();
    let year = (digits.len() == 4 && digits.iter().all(u8::is_ascii_digit))
        .then(|| text.parse::<u16>().ok())
        .flatten();
    year.map(Some)
        .ok_or_else(|| row.bad(col, text, "an edition year such as `2021`"))
}

/// Reads `default_versions` into a map from crate id to version id.
pub(crate) fn read_default_versions(input: impl Read) -> Result<IdMap<u32>, DumpError> {
    let mut out = IdMap::default();
    for_each_row(
        input,
        TableName::DefaultVersions,
        DEFAULT_VERSIONS,
        |row, &[crate_id, version_id]| {
            out.insert(row.id(crate_id)?, row.id(version_id)?);
            Ok(())
        },
    )?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_versions() {
        let csv = "checksum,crate_id,created_at,edition,id,num,rust_version,yanked\n\
            ab,1,2026-01-01 00:00:00+00,2021,10,1.0.0,1.70,f\n\
            cd,1,2026-02-01 00:00:00.123+00,,11,1.1.0,\"\",t\n";
        let versions = read_versions(csv.as_bytes()).expect("reads");
        let first = versions.get(&10).expect("10");
        assert_eq!(
            (first.crate_id, first.num.as_str(), first.yanked),
            (1, "1.0.0", false)
        );
        assert_eq!(first.edition, Some(2021));
        assert_eq!(first.rust_version.as_deref(), Some("1.70"));
        let second = versions.get(&11).expect("11");
        assert!(second.yanked);
        assert_eq!(
            (second.edition, second.rust_version.as_deref()),
            (None, None)
        );
    }

    #[test]
    fn bad_boolean_is_rejected() {
        let csv = "id,crate_id,num,created_at,yanked,rust_version,edition\n\
            10,1,1.0.0,2026-01-01 00:00:00+00,,,\n";
        let error = read_versions(csv.as_bytes()).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::BadValue {
                table: "versions",
                column: "yanked",
                record: 1,
                ..
            }
        ));
    }

    #[test]
    fn bad_edition_is_rejected() {
        for bad in ["21", "20211", "20x1", "-021", " 2021", "2021 "] {
            let csv = format!(
                "id,crate_id,num,created_at,yanked,rust_version,edition\n\
                10,1,1.0.0,2026-01-01 00:00:00+00,f,,{bad}\n"
            );
            let error = read_versions(csv.as_bytes()).expect_err(bad);
            assert!(
                matches!(
                    error,
                    DumpError::BadValue {
                        table: "versions",
                        column: "edition",
                        record: 1,
                        expected: "an edition year such as `2021`",
                        ..
                    }
                ),
                "{bad}: {error}"
            );
        }
    }

    #[test]
    fn reads_default_versions() {
        let csv = "crate_id,num_versions,version_id\n1,2,11\n";
        let defaults = read_default_versions(csv.as_bytes()).expect("reads");
        assert_eq!(defaults.get(&1), Some(&11));
    }
}
