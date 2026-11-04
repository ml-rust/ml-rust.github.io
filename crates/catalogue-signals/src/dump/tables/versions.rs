//! The `versions` and `default_versions` tables.

use std::collections::BTreeMap;
use std::io::Read;

use crate::dump::error::DumpError;
use crate::dump::names::TableName;
use crate::dump::row::{Col, Row};
use crate::dump::snapshot::{CrateDetails, IdMap, IdSet, VersionRow};
use crate::dump::table::for_each_row;

const VERSIONS: &[&str; 10] = &[
    "id",
    "crate_id",
    "num",
    "created_at",
    "yanked",
    "rust_version",
    "edition",
    "published_by",
    "has_lib",
    "bin_names",
];

/// [`VERSIONS`], then the columns of [`CrateDetails`].
const VERSIONS_WITH_DETAILS: &[&str; 13] = &[
    "id",
    "crate_id",
    "num",
    "created_at",
    "yanked",
    "rust_version",
    "edition",
    "published_by",
    "has_lib",
    "bin_names",
    "license",
    "links",
    "features",
];

/// Most bytes of an invalid `features` field that an error carries.
const FEATURES_PREVIEW: usize = 120;

/// How `bin_names` reads when the version names no binary target.
const NO_BINARIES: &[&str] = &["", "{}"];

const DEFAULT_VERSIONS: &[&str; 2] = &["crate_id", "version_id"];

/// Reads `versions`.
pub(crate) fn read_versions(input: impl Read) -> Result<IdMap<VersionRow>, DumpError> {
    let mut out = IdMap::default();
    for_each_row(input, TableName::Versions, VERSIONS, |row, cols| {
        let (id, version) = version_row(row, cols)?;
        out.insert(id, version);
        Ok(())
    })?;
    Ok(out)
}

/// The rows of `versions`, and the [`CrateDetails`] by version id of each version of `kept`.
///
/// A row of any other crate allocates nothing beyond what [`read_versions`] allocates.
pub(crate) fn read_versions_with_details(
    input: impl Read,
    kept: &IdSet,
) -> Result<(IdMap<VersionRow>, IdMap<CrateDetails>), DumpError> {
    let mut out = IdMap::default();
    let mut details = IdMap::default();
    for_each_row(
        input,
        TableName::Versions,
        VERSIONS_WITH_DETAILS,
        |row,
         &[
            id,
            crate_id,
            num,
            created_at,
            yanked,
            rust_version,
            edition,
            published_by,
            has_lib,
            bin_names,
            license,
            links,
            features,
        ]| {
            let cols = [
                id,
                crate_id,
                num,
                created_at,
                yanked,
                rust_version,
                edition,
                published_by,
                has_lib,
                bin_names,
            ];
            let (version_id, version) = version_row(row, &cols)?;
            if kept.contains(&version.crate_id) {
                details.insert(version_id, crate_details(row, license, links, features)?);
            }
            out.insert(version_id, version);
            Ok(())
        },
    )?;
    Ok((out, details))
}

/// The version id and the row of one record, read through the [`VERSIONS`] columns.
fn version_row(row: &Row<'_>, cols: &[Col; 10]) -> Result<(u32, VersionRow), DumpError> {
    let &[
        id,
        crate_id,
        num,
        created_at,
        yanked,
        rust_version,
        edition,
        published_by,
        has_lib,
        bin_names,
    ] = cols;
    let version = VersionRow {
        crate_id: row.id(crate_id)?,
        num: row.text(num)?.to_owned(),
        created_at: row.timestamp(created_at)?,
        yanked: row.flag(yanked)?,
        rust_version: row.optional_string(rust_version)?,
        edition: year(row, edition)?,
        published_by: row.optional_id(published_by)?,
        has_lib: row.optional_flag(has_lib)?,
        has_bin: !NO_BINARIES.contains(&row.text(bin_names)?),
    };
    Ok((row.id(id)?, version))
}

/// The license, the `links` key and the feature map of one record.
fn crate_details(
    row: &Row<'_>,
    license: Col,
    links: Col,
    features: Col,
) -> Result<CrateDetails, DumpError> {
    let text = row.optional_text(features)?.unwrap_or("{}");
    let features_map: BTreeMap<String, Vec<String>> = serde_json::from_str(text).map_err(|_| {
        let preview = text.get(..FEATURES_PREVIEW).unwrap_or(text);
        row.bad(
            features,
            preview,
            "a JSON map from feature name to a list of names",
        )
    })?;
    Ok(CrateDetails {
        license: row.optional_string(license)?,
        links: row.optional_string(links)?,
        features: features_map,
        renames: Vec::new(),
    })
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
        let csv = "bin_names,checksum,crate_id,created_at,edition,has_lib,id,num,published_by,rust_version,yanked\n\
            {tool},ab,1,2026-01-01 00:00:00+00,2021,f,10,1.0.0,7,1.70,f\n\
            {},cd,1,2026-02-01 00:00:00.123+00,,,11,1.1.0,,\"\",t\n";
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
        assert_eq!(first.published_by, Some(7));
        assert_eq!(second.published_by, None, "NULL publisher");
        assert_eq!((first.has_lib, first.has_bin), (Some(false), true));
        assert_eq!(
            (second.has_lib, second.has_bin),
            (None, false),
            "NULL has_lib"
        );
    }

    const DETAILED: &str = "bin_names,crate_id,created_at,edition,features,has_lib,id,license,links,num,published_by,rust_version,yanked\n\
        {},1,2026-01-01 00:00:00+00,,\"{\"\"cuda\"\":[\"\"dep:cudarc\"\"],\"\"default\"\":[]}\",t,10,MIT OR Apache-2.0,openssl,1.0.0,,,f\n\
        {},1,2026-01-02 00:00:00+00,,{},t,11,,,1.1.0,,,f\n\
        {},2,2026-01-03 00:00:00+00,,\"not json is fine here\",t,20,MIT,,1.0.0,,,f\n";

    #[test]
    fn details_are_kept_only_for_the_named_crates() {
        let kept: IdSet = [1].into_iter().collect();
        let (versions, details) =
            read_versions_with_details(DETAILED.as_bytes(), &kept).expect("reads");
        assert_eq!(versions.len(), 3, "every version row stays");
        assert_eq!(details.len(), 2, "no details of the other crate");
        let first = details.get(&10).expect("10");
        assert_eq!(first.license.as_deref(), Some("MIT OR Apache-2.0"));
        assert_eq!(first.links.as_deref(), Some("openssl"));
        assert_eq!(
            first.features.get("cuda").map(Vec::as_slice),
            Some(&["dep:cudarc".to_owned()][..])
        );
        let second = details.get(&11).expect("11");
        assert_eq!(
            (second.license.as_deref(), second.links.as_deref()),
            (None, None)
        );
        assert!(second.features.is_empty());
    }

    #[test]
    fn a_feature_map_that_is_not_json_is_rejected_for_a_named_crate() {
        let kept: IdSet = [2].into_iter().collect();
        let error = read_versions_with_details(DETAILED.as_bytes(), &kept).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::BadValue {
                table: "versions",
                column: "features",
                record: 3,
                ..
            }
        ));
    }

    #[test]
    fn a_dump_without_the_license_column_is_rejected_when_details_are_asked_for() {
        let csv = "id,crate_id,num,created_at,yanked,rust_version,edition,published_by,has_lib,bin_names,links,features\n";
        let kept: IdSet = [1].into_iter().collect();
        let error = read_versions_with_details(csv.as_bytes(), &kept).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::MissingColumn {
                table: "versions",
                column: "license"
            }
        ));
    }

    #[test]
    fn missing_library_column_is_rejected() {
        let csv = "id,crate_id,num,created_at,yanked,rust_version,edition,published_by,bin_names\n";
        let error = read_versions(csv.as_bytes()).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::MissingColumn {
                table: "versions",
                column: "has_lib"
            }
        ));
    }

    #[test]
    fn bad_library_flag_is_rejected() {
        let csv = "id,crate_id,num,created_at,yanked,rust_version,edition,published_by,has_lib,bin_names\n\
            10,1,1.0.0,2026-01-01 00:00:00+00,f,,,,maybe,{}\n";
        let error = read_versions(csv.as_bytes()).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::BadValue {
                table: "versions",
                column: "has_lib",
                record: 1,
                ..
            }
        ));
    }

    #[test]
    fn bad_publisher_is_rejected() {
        let csv = "id,crate_id,num,created_at,yanked,rust_version,edition,published_by,has_lib,bin_names\n\
            10,1,1.0.0,2026-01-01 00:00:00+00,f,,,alice,t,{}\n";
        let error = read_versions(csv.as_bytes()).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::BadValue {
                table: "versions",
                column: "published_by",
                record: 1,
                ..
            }
        ));
    }

    #[test]
    fn missing_publisher_column_is_rejected() {
        let csv = "id,crate_id,num,created_at,yanked,rust_version,edition\n";
        let error = read_versions(csv.as_bytes()).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::MissingColumn {
                table: "versions",
                column: "published_by"
            }
        ));
    }

    #[test]
    fn bad_boolean_is_rejected() {
        let csv = "id,crate_id,num,created_at,yanked,rust_version,edition,published_by,has_lib,bin_names\n\
            10,1,1.0.0,2026-01-01 00:00:00+00,,,,,t,{}\n";
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
                "id,crate_id,num,created_at,yanked,rust_version,edition,published_by,has_lib,bin_names\n\
                10,1,1.0.0,2026-01-01 00:00:00+00,f,,{bad},,t,{{}}\n"
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
