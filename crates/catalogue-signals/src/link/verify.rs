//! The link check of one crate release.

use std::path::Path;

use catalogue_core::Date;

use super::cache::{cached_check, store_check};
use super::check::{CommitStanding, commit_standing};
use super::error::LinkError;
use super::types::{CrateRelease, LinkCheck, LinkOutcome};
use super::vcs_info::read_vcs_info;
use crate::github::RepoRef;
use crate::http::{Pacer, Transport};

/// Where crates.io serves the `.crate` packages.
const PACKAGE_HOST: &str = "https://static.crates.io/crates";

/// Checks that `release` was published from a commit that a branch or tag of
/// `repo` holds.
///
/// A result in the link cache under `cache_dir` for the same repo returns with
/// no request. Otherwise the check downloads the package, which needs no
/// authentication, reads `.cargo_vcs_info.json` from its stream and stops
/// there. A package without a `git.sha1` gives [`LinkCheck::NoVcsInfo`]. A
/// commit gets the GitHub lookup of [`commit_standing`] with `token`. A commit
/// that only a fork holds gives [`LinkCheck::NotOnRepoRef`]. The result
/// goes into the cache with `today` as its check date. An error is never cached.
///
/// # Errors
///
/// Returns a [`LinkError`] when the name or version is unusable, a request
/// fails, the download status is not 200, the package is unreadable, the
/// GitHub lookup fails, or a cache file cannot be read or written.
pub fn check_link(
    transport: &dyn Transport,
    token: &str,
    repo: &RepoRef,
    release: &CrateRelease,
    cache_dir: &Path,
    today: Date,
    pacer: &mut dyn Pacer,
) -> Result<LinkOutcome, LinkError> {
    if let Some(outcome) = cached_link(cache_dir, repo, release)? {
        return Ok(outcome);
    }
    let check = match package_commit(transport, release)? {
        (Some(sha1), dirty) => {
            match commit_standing(transport, token, repo, release, &sha1, pacer)? {
                CommitStanding::Held { held_by } => LinkCheck::Verified {
                    sha1,
                    dirty,
                    held_by: Some(held_by),
                },
                CommitStanding::NotHeld => LinkCheck::NotOnRepoRef { sha1, dirty },
                CommitStanding::Missing => LinkCheck::CommitMissing { sha1, dirty },
            }
        }
        (None, _) => LinkCheck::NoVcsInfo,
    };
    store_check(cache_dir, release, &repo.to_string(), &check, today)?;
    Ok(LinkOutcome {
        check,
        cached: false,
        checked: today,
    })
}

/// The cached result for `release` and `repo`, with no request.
///
/// Returns `None` on a cache miss. A hit has `cached` set and the stored check date.
///
/// # Errors
///
/// Returns a [`LinkError`] when the name or version is unusable, or the cache
/// file exists and cannot be read.
pub fn cached_link(
    cache_dir: &Path,
    repo: &RepoRef,
    release: &CrateRelease,
) -> Result<Option<LinkOutcome>, LinkError> {
    for part in [&release.name, &release.version] {
        if !is_usable(part) {
            return Err(LinkError::UnsafeName {
                value: part.clone(),
            });
        }
    }
    cached_check(cache_dir, release, &repo.to_string())
}

/// The commit and dirty flag in the package of `release`. The commit is `None` without commit data.
fn package_commit(
    transport: &dyn Transport,
    release: &CrateRelease,
) -> Result<(Option<String>, bool), LinkError> {
    let CrateRelease { name, version } = release;
    let url = format!("{PACKAGE_HOST}/{name}/{name}-{version}.crate");
    let response = transport.get(&url, &[])?;
    if response.status != 200 {
        return Err(LinkError::Download {
            url,
            status: response.status,
        });
    }
    let info = read_vcs_info(response.body, name, version)
        .map_err(|source| LinkError::Package { url, source })?;
    Ok(info.map_or((None, false), |info| (info.sha1, info.dirty)))
}

/// Whether `part` is a name or version that a URL and a file name carry as is.
fn is_usable(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && part
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_names_and_versions_are_usable() {
        for good in ["serde", "Foo_bar-2", "1.0.0-rc.1+build.5"] {
            assert!(is_usable(good), "{good}");
        }
        for bad in ["", ".", "..", "a/b", "a b", "a?x", "é", "a\\b"] {
            assert!(!is_usable(bad), "{bad}");
        }
    }
}
