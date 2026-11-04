//! The link results kept in the cache directory.
//!
//! A result is immutable for one crate, version and repo. The file
//! `<cache>/links/<canonical-crate>-<version>@<owner>__<repo>.json`, with the
//! repo lowercase, holds `{rule, repo, result, checked}`. Each repo has its own
//! file. `rule` names the verification rule that wrote the result. `checked` is
//! the UTC date of the check. A file from an older rule, with no `rule`, or
//! with no `checked` date reads as a miss. A commit result from the rule before
//! the manifest proof stays valid. That rule's other results read as a miss, so
//! they get the proof.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use catalogue_core::date_text::plain;
use catalogue_core::{Date, canonical_crate_name};
use serde::{Deserialize, Serialize};

use super::error::LinkError;
use super::types::{CrateRelease, LinkCheck, LinkOutcome};
use crate::atomic::write_atomic;

/// The directory under `cache_dir` that holds the link results.
pub fn links_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("links")
}

/// The verification rule that writes results. Rule 4 adds the manifest proof
/// for a release with no commit and for a repo off GitHub.
const RULE: u32 = 4;

/// The rule before the manifest proof. It verifies a commit when a branch or tag
/// of the repo holds it, and its commit results stay valid.
const COMMIT_RULE: u32 = 3;

/// One cache file.
#[derive(Serialize, Deserialize)]
struct Record {
    /// Absent in a file from the first rule, which reads as rule 0.
    #[serde(default)]
    rule: u32,
    repo: String,
    result: LinkCheck,
    /// The date of the check. A file without it reads as a miss.
    #[serde(serialize_with = "plain")]
    checked: Date,
}

fn file_path(cache_dir: &Path, release: &CrateRelease, repo: &str) -> PathBuf {
    let name = canonical_crate_name(&release.name);
    let repo = repo.to_ascii_lowercase().replace('/', "__");
    links_dir(cache_dir).join(format!("{name}-{}@{repo}.json", release.version))
}

/// The cached result for `release` and `repo` under `cache_dir`.
///
/// A hit has `cached` set and the stored check date. Returns `None` when no
/// file exists, the file is not a link record, the file comes from an older
/// rule or has no check date, or the file names another repo. A file from the
/// rule before the manifest proof hits only when its result holds a commit. Repo
/// names compare without regard to ASCII case.
///
/// # Errors
///
/// Returns [`LinkError::Cache`] when the file exists and cannot be read.
pub fn cached_check(
    cache_dir: &Path,
    release: &CrateRelease,
    repo: &str,
) -> Result<Option<LinkOutcome>, LinkError> {
    let path = file_path(cache_dir, release, repo);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(LinkError::Cache { path, source }),
    };
    let Ok(record) = serde_json::from_slice::<Record>(&bytes) else {
        return Ok(None);
    };
    let rule_holds =
        record.rule == RULE || (record.rule == COMMIT_RULE && record.result.sha1().is_some());
    let current = rule_holds && record.repo.eq_ignore_ascii_case(repo);
    Ok(current.then_some(LinkOutcome {
        check: record.result,
        cached: true,
        checked: record.checked,
    }))
}

/// Stores `result` for `release` and `repo` under `cache_dir`, checked on `checked`.
///
/// The write goes to a temporary file that is renamed into place, so a reader
/// never sees a partial file and a failed write leaves no temporary file.
///
/// # Errors
///
/// Returns [`LinkError::Cache`] when the directory or the file cannot be written.
pub fn store_check(
    cache_dir: &Path,
    release: &CrateRelease,
    repo: &str,
    result: &LinkCheck,
    checked: Date,
) -> Result<(), LinkError> {
    let path = file_path(cache_dir, release, repo);
    let record = Record {
        rule: RULE,
        repo: repo.to_ascii_lowercase(),
        result: result.clone(),
        checked,
    };
    let bytes = serde_json::to_vec(&record).map_err(|error| LinkError::Cache {
        path: path.clone(),
        source: io::Error::other(error),
    })?;
    write_atomic(&path, |file| file.write_all(&bytes)).map_err(|error| LinkError::Cache {
        path: error.path,
        source: error.source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::test_host::temp_cache;

    fn release() -> CrateRelease {
        CrateRelease {
            name: "arrow".to_owned(),
            version: "1.0.0".to_owned(),
        }
    }

    fn date() -> Date {
        Date::new(2026, 11, 4).expect("date")
    }

    fn write_record(dir: &Path, rule: u32, result: &str) {
        fs::create_dir_all(links_dir(dir)).expect("dir");
        let record =
            format!(r#"{{"rule":{rule},"repo":"a/b","result":{result},"checked":"2026-10-01"}}"#);
        fs::write(file_path(dir, &release(), "a/b"), record).expect("file");
    }

    #[test]
    fn a_result_with_no_commit_from_the_commit_rule_misses() {
        let dir = temp_cache();
        for kind in ["no-vcs-info", "unsupported-host"] {
            write_record(dir.path(), COMMIT_RULE, &format!(r#"{{"kind":"{kind}"}}"#));
            let found = cached_check(dir.path(), &release(), "a/b").expect("reads");
            assert_eq!(found, None, "{kind}");
        }
    }

    #[test]
    fn a_commit_result_from_the_commit_rule_hits() {
        let dir = temp_cache();
        let sha = "a866b336f14aa57a07f0d0be9f8762746e64ecb4";
        write_record(
            dir.path(),
            COMMIT_RULE,
            &format!(r#"{{"kind":"verified","sha1":"{sha}","dirty":false}}"#),
        );
        let found = cached_check(dir.path(), &release(), "a/b")
            .expect("reads")
            .expect("hit");
        assert_eq!(found.check.sha1(), Some(sha));
        assert!(found.cached);
    }

    #[test]
    fn every_result_from_the_current_rule_hits() {
        let dir = temp_cache();
        for check in [
            LinkCheck::NoVcsInfo,
            LinkCheck::UnsupportedHost,
            LinkCheck::ManifestDeclares {
                path: "Cargo.toml".to_owned(),
            },
        ] {
            store_check(dir.path(), &release(), "a/b", &check, date()).expect("stores");
            let found = cached_check(dir.path(), &release(), "a/b")
                .expect("reads")
                .expect("hit");
            assert_eq!(found.check, check);
        }
    }

    #[test]
    fn a_forge_repo_gets_its_own_file() {
        let dir = temp_cache();
        let forge = "codeberg.org/A/B";
        store_check(dir.path(), &release(), forge, &LinkCheck::NoVcsInfo, date()).expect("stores");
        assert!(
            links_dir(dir.path())
                .join("arrow-1.0.0@codeberg.org__a__b.json")
                .is_file()
        );
        let found = cached_check(dir.path(), &release(), "a/b").expect("reads");
        assert_eq!(found, None);
        let found = cached_check(dir.path(), &release(), forge).expect("reads");
        assert!(found.is_some());
    }
}
