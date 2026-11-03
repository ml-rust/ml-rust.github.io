//! The link results kept in the cache directory.
//!
//! A result is immutable for one crate, version and repo. The file
//! `<cache>/links/<canonical-crate>-<version>@<owner>__<repo>.json`, with the
//! repo lowercase, holds `{rule, repo, result, checked}`. Each repo has its own
//! file. `rule` names the verification rule that wrote the result. `checked` is
//! the UTC date of the check. A file from an older rule, with no `rule`, or
//! with no `checked` date reads as a miss.

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

/// The verification rule that writes results. Rule 2 verifies a commit only
/// when the default branch contains it.
const RULE: u32 = 2;

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
/// rule or has no check date, or the file names another repo. Repo names
/// compare without regard to ASCII case.
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
    let current = record.rule == RULE && record.repo.eq_ignore_ascii_case(repo);
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
