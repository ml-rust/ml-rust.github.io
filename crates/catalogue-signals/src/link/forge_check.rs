//! The link check of a crate whose repo is on a host other than GitHub.

use std::path::Path;

use catalogue_core::Date;

use super::cache::{cached_check, store_check};
use super::error::LinkError;
use super::forge::ForgeRepo;
use super::manifest::{RepoSite, declaring_manifest};
use super::types::{CrateRelease, LinkCheck, LinkOutcome};
use super::verify::check_release_names;
use crate::http::{Pacer, Transport};

/// Checks that a manifest on the default branch of `repo` declares `release`.
///
/// A result in the link cache under `cache_dir` for the same repo returns with no request.
/// Otherwise the check runs the manifest proof with no token and sends no package download. A
/// match gives [`LinkCheck::ManifestDeclares`]. No match gives [`LinkCheck::UnsupportedHost`],
/// which leaves the signal unknown. The result goes into the cache with `today` as its check
/// date. An error is never cached.
///
/// # Errors
///
/// Returns a [`LinkError`] when the name or version is unusable, a request fails, or a cache file
/// cannot be read or written.
pub fn check_forge_link(
    transport: &dyn Transport,
    repo: &ForgeRepo,
    release: &CrateRelease,
    cache_dir: &Path,
    today: Date,
    pacer: &mut dyn Pacer,
) -> Result<LinkOutcome, LinkError> {
    if let Some(outcome) = cached_forge_link(cache_dir, repo, release)? {
        return Ok(outcome);
    }
    let site = RepoSite::Forge(repo);
    let check = match declaring_manifest(transport, &site, &release.name, pacer)? {
        Some(path) => LinkCheck::ManifestDeclares { path },
        None => LinkCheck::UnsupportedHost,
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
/// Returns a [`LinkError`] when the name or version is unusable, or the cache file exists and
/// cannot be read.
pub fn cached_forge_link(
    cache_dir: &Path,
    repo: &ForgeRepo,
    release: &CrateRelease,
) -> Result<Option<LinkOutcome>, LinkError> {
    check_release_names(release)?;
    cached_check(cache_dir, release, &repo.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::test_host::{Host, NoWait, Reply, temp_cache};

    const ROOT: &str = "https://codeberg.org/api/v1/repos/tazz4843/whisper-rs";

    fn today() -> Date {
        Date::new(2026, 11, 4).expect("date")
    }

    fn release() -> CrateRelease {
        CrateRelease {
            name: "whisper-rs".to_owned(),
            version: "0.13.2".to_owned(),
        }
    }

    fn repo() -> ForgeRepo {
        ForgeRepo::parse("https://codeberg.org/tazz4843/whisper-rs").expect("repo")
    }

    fn proving_host() -> Host {
        Host::default()
            .route(ROOT, Reply::ok(r#"{"default_branch":"main"}"#))
            .route(
                &format!("{ROOT}/git/trees/main?recursive=true"),
                Reply::ok(r#"{"tree":[{"path":"Cargo.toml","type":"blob"}]}"#),
            )
            .route(
                &format!("{ROOT}/raw/Cargo.toml?ref=main"),
                Reply::ok("[package]\nname = \"whisper-rs\"\n"),
            )
    }

    fn run(host: &Host, dir: &Path) -> Result<LinkOutcome, LinkError> {
        check_forge_link(host, &repo(), &release(), dir, today(), &mut NoWait)
    }

    #[test]
    fn a_found_manifest_verifies_and_the_second_run_sends_nothing() {
        let dir = temp_cache();
        let host = proving_host();
        let outcome = run(&host, dir.path()).expect("checks");
        assert_eq!(
            outcome.check,
            LinkCheck::ManifestDeclares {
                path: "Cargo.toml".to_owned()
            }
        );
        assert_eq!(outcome.check.verified(), Some(true));
        assert!(!outcome.cached);
        assert!(host.urls().iter().all(|url| !url.contains("crates.io")));
        assert!(host.headers().iter().all(Vec::is_empty));

        let second = Host::default();
        let hit = run(&second, dir.path()).expect("hit");
        assert!(hit.cached);
        assert!(second.urls().is_empty());
    }

    #[test]
    fn no_manifest_keeps_the_unsupported_host_result() {
        let dir = temp_cache();
        let host = Host::default().route(ROOT, Reply::status(404));
        let outcome = run(&host, dir.path()).expect("checks");
        assert_eq!(outcome.check, LinkCheck::UnsupportedHost);
        assert_eq!(outcome.check.verified(), None);
    }

    #[test]
    fn a_request_error_is_not_cached() {
        let dir = temp_cache();
        let host = Host::default().route(ROOT, Reply::status(503));
        assert!(matches!(
            run(&host, dir.path()),
            Err(LinkError::Forge { .. })
        ));
        let again = proving_host();
        assert!(!run(&again, dir.path()).expect("checks").cached);
    }

    #[test]
    fn an_unsafe_crate_name_sends_no_request() {
        let dir = temp_cache();
        let host = Host::default();
        let bad = CrateRelease {
            name: "../x".to_owned(),
            version: "1.0.0".to_owned(),
        };
        let outcome = check_forge_link(&host, &repo(), &bad, dir.path(), today(), &mut NoWait);
        assert!(matches!(outcome, Err(LinkError::UnsafeName { .. })));
        assert!(host.urls().is_empty());
    }
}
