//! The link check of one crate release.

use std::path::Path;

use catalogue_core::Date;

use super::cache::{cached_check, store_check};
use super::check::{CommitStanding, commit_standing};
use super::error::LinkError;
use super::manifest::{RepoSite, declaring_manifest};
use super::types::{CrateRelease, LinkCheck, LinkOutcome};
use super::vcs_info::read_vcs_info;
use crate::github::RepoRef;
use crate::http::{Pacer, Transport};

/// Where crates.io serves the `.crate` packages.
const PACKAGE_HOST: &str = "https://static.crates.io/crates";

/// Checks that `release` was published from a commit that a branch or tag of
/// `repo` holds, or that the repo declares it in a manifest when the release
/// records no commit.
///
/// A result in the link cache under `cache_dir` for the same repo returns with
/// no request. Otherwise the check downloads the package, which needs no
/// authentication, reads `.cargo_vcs_info.json` from its stream and stops
/// there. A package without a `git.sha1` gets the manifest proof with `token`.
/// A `Cargo.toml` on the default branch that declares the crate gives
/// [`LinkCheck::ManifestDeclares`], and no such file gives [`LinkCheck::NoVcsInfo`].
/// A commit gets the GitHub lookup of [`commit_standing`] with `token` and no
/// manifest proof. A commit that only a fork holds gives
/// [`LinkCheck::NotOnRepoRef`]. The result goes into the cache with `today` as
/// its check date. An error is never cached.
///
/// # Errors
///
/// Returns a [`LinkError`] when the name or version is unusable, a request
/// fails, the download status is not 200, the package is unreadable, the
/// GitHub lookup or the manifest proof fails, or a cache file cannot be read or
/// written.
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
        (None, _) => {
            let site = RepoSite::Github { repo, token };
            match declaring_manifest(transport, &site, &release.name, pacer)? {
                Some(path) => LinkCheck::ManifestDeclares { path },
                None => LinkCheck::NoVcsInfo,
            }
        }
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
    check_release_names(release)?;
    cached_check(cache_dir, release, &repo.to_string())
}

/// Refuses a crate name or version that a URL or a file name cannot carry.
pub(super) fn check_release_names(release: &CrateRelease) -> Result<(), LinkError> {
    for part in [&release.name, &release.version] {
        if !is_usable(part) {
            return Err(LinkError::UnsafeName {
                value: part.clone(),
            });
        }
    }
    Ok(())
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
    use std::fs;

    use super::*;
    use crate::github::{GRAPHQL_ENDPOINT, GithubError};
    use crate::link::cache::links_dir;
    use crate::link::test_host::{Host, NoWait, Reply, package, temp_cache};

    const SHA: &str = "a866b336f14aa57a07f0d0be9f8762746e64ecb4";
    const PACKAGE: &str = "https://static.crates.io/crates/datafusion/datafusion-1.0.0.crate";
    const ROOT: &str = "https://api.github.com/repos/apache/datafusion";
    const CACHE_FILE: &str = "datafusion-1.0.0@apache__datafusion.json";

    fn today() -> Date {
        Date::new(2026, 11, 4).expect("date")
    }

    fn release() -> CrateRelease {
        CrateRelease {
            name: "datafusion".to_owned(),
            version: "1.0.0".to_owned(),
        }
    }

    fn without_commit() -> Reply {
        Reply::bytes(package("datafusion", "1.0.0", None))
    }

    /// Serves a package with no commit and a repo whose nested manifest declares the crate.
    fn proving_host() -> Host {
        Host::default()
            .route(PACKAGE, without_commit())
            .route(ROOT, Reply::ok(r#"{"default_branch":"main"}"#))
            .route(
                &format!("{ROOT}/git/trees/main?recursive=1"),
                Reply::ok(r#"{"tree":[{"path":"datafusion/core/Cargo.toml","type":"blob"}]}"#),
            )
            .route(
                &format!("{ROOT}/contents/datafusion/core/Cargo.toml?ref=main"),
                Reply::ok("[package]\nname = \"datafusion\"\n"),
            )
    }

    fn declared() -> LinkCheck {
        LinkCheck::ManifestDeclares {
            path: "datafusion/core/Cargo.toml".to_owned(),
        }
    }

    fn run(host: &Host, dir: &Path) -> Result<LinkOutcome, LinkError> {
        let repo = RepoRef::parse("apache/datafusion").expect("repo");
        check_link(host, "t0k", &repo, &release(), dir, today(), &mut NoWait)
    }

    /// Writes a cache file as the rule before the manifest proof wrote it.
    fn stored_before_the_proof(dir: &Path, result: &str) {
        fs::create_dir_all(links_dir(dir)).expect("dir");
        let record = format!(
            r#"{{"rule":3,"repo":"apache/datafusion","result":{result},"checked":"2026-10-01"}}"#
        );
        fs::write(links_dir(dir).join(CACHE_FILE), record).expect("file");
    }

    #[test]
    fn a_release_without_a_commit_gets_the_manifest_proof_and_stores_the_result() {
        let dir = temp_cache();
        let host = proving_host();
        let outcome = run(&host, dir.path()).expect("checks");
        assert_eq!(outcome.check, declared());
        assert_eq!(outcome.check.verified(), Some(true));
        assert!(!outcome.cached);

        let second = Host::default();
        let hit = run(&second, dir.path()).expect("hit");
        assert!(hit.cached);
        assert_eq!(hit.check, declared());
        assert!(second.urls().is_empty());
    }

    #[test]
    fn no_manifest_keeps_the_no_vcs_info_result_and_a_new_rule_result_stays_a_hit() {
        let dir = temp_cache();
        let host = Host::default().route(PACKAGE, without_commit());
        let outcome = run(&host, dir.path()).expect("checks");
        assert_eq!(outcome.check, LinkCheck::NoVcsInfo);
        assert_eq!(outcome.check.verified(), Some(false));
        assert_eq!(host.urls(), [PACKAGE, ROOT]);

        let second = Host::default();
        assert!(run(&second, dir.path()).expect("hit").cached);
        assert!(second.urls().is_empty());
    }

    #[test]
    fn a_release_with_a_commit_never_reaches_the_manifest_step() {
        let dir = temp_cache();
        let vcs = format!(r#"{{"git":{{"sha1":"{SHA}","dirty":false}},"path_in_vcs":""}}"#);
        let host = proving_host()
            .route(
                PACKAGE,
                Reply::bytes(package("datafusion", "1.0.0", Some(&vcs))),
            )
            .route(
                GRAPHQL_ENDPOINT,
                Reply::ok(r#"{"data":{"repository":{"object":null}}}"#),
            );
        let outcome = run(&host, dir.path()).expect("checks");
        let expected = LinkCheck::CommitMissing {
            sha1: SHA.to_owned(),
            dirty: false,
        };
        assert_eq!(outcome.check, expected);
        assert_eq!(outcome.check.verified(), Some(false));
        assert_eq!(host.urls(), [PACKAGE, GRAPHQL_ENDPOINT]);
    }

    #[test]
    fn a_cached_no_vcs_info_reruns_the_proof_and_stores_the_new_result() {
        let dir = temp_cache();
        stored_before_the_proof(dir.path(), r#"{"kind":"no-vcs-info"}"#);
        let host = proving_host();
        let outcome = run(&host, dir.path()).expect("checks");
        assert!(!outcome.cached);
        assert_eq!(outcome.check, declared());
        assert_eq!(outcome.checked, today());

        let hit = run(&Host::default(), dir.path()).expect("hit");
        assert!(hit.cached);
        assert_eq!(hit.check, declared());
    }

    #[test]
    fn a_cached_commit_result_sends_no_request() {
        for result in [
            format!(r#"{{"kind":"verified","sha1":"{SHA}","dirty":false}}"#),
            format!(r#"{{"kind":"commit-missing","sha1":"{SHA}","dirty":false}}"#),
            format!(r#"{{"kind":"not-on-repo-ref","sha1":"{SHA}","dirty":false}}"#),
        ] {
            let dir = temp_cache();
            stored_before_the_proof(dir.path(), &result);
            let host = Host::default();
            let hit = run(&host, dir.path()).expect("hit");
            assert!(hit.cached, "{result}");
            assert!(host.urls().is_empty(), "{result}");
        }
    }

    #[test]
    fn a_request_error_during_the_proof_is_not_cached_and_does_not_verify() {
        let dir = temp_cache();
        let host = Host::default()
            .route(PACKAGE, without_commit())
            .route(ROOT, Reply::status(503));
        let error = run(&host, dir.path()).expect_err("rejected");
        assert!(
            matches!(
                error,
                LinkError::Github(GithubError::Status { status: 503, .. })
            ),
            "{error}"
        );
        assert!(!links_dir(dir.path()).join(CACHE_FILE).exists());
    }

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
