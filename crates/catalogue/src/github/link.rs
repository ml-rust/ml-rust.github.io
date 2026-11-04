//! The `catalogue github link` action: whether a crate was published from its repo.

use std::path::PathBuf;

use catalogue_core::Date;
use catalogue_signals::assemble::{CrateIndex, latest_release};
use catalogue_signals::fetch;
use catalogue_signals::github::{GITHUB_INTERVAL, GithubError, RepoRef};
use catalogue_signals::http::{IntervalPacer, ReqwestTransport};
use catalogue_signals::link::{CrateRelease, LinkOutcome, check_link};

use super::command::GithubCommandError;
use crate::cache_dir::resolve_cache_dir;
use crate::clock::today_utc;
use crate::dump::{DumpCommandError, read_cached};
use crate::render::or_none;
use crate::token::NoToken;

/// Checks that the latest unyanked version of `crate_name` was published from a commit in `repo`,
/// or that a manifest in `repo` declares it when the release records no commit.
///
/// The version comes from the cached dump. The token is read after the repo
/// argument and before the dump.
pub(super) fn link(
    repo: &str,
    crate_name: &str,
    cache_dir: Option<PathBuf>,
    token: Result<String, NoToken>,
) -> Result<String, GithubCommandError> {
    let repo = RepoRef::parse(repo)?;
    let token = token?;
    let cache_dir = resolve_cache_dir(cache_dir)?;
    let snapshot = read_cached(&fetch::dump_path(&cache_dir), 0)?;
    let index = CrateIndex::new(&snapshot);
    if index.id(crate_name).is_none() {
        return Err(DumpCommandError::UnknownCrates {
            names: vec![crate_name.to_owned()],
        }
        .into());
    }
    let release = latest_release(&snapshot, &index, crate_name).ok_or_else(|| {
        GithubCommandError::NoRelease {
            name: crate_name.to_owned(),
        }
    })?;
    let transport = ReqwestTransport::new().map_err(GithubError::from)?;
    let mut pacer = IntervalPacer::new(GITHUB_INTERVAL);
    let today = Date::from(today_utc());
    let outcome = check_link(
        &transport, &token, &repo, &release, &cache_dir, today, &mut pacer,
    )?;
    Ok(render_link(&release, &outcome))
}

/// The `key: value` lines of one link check, with no trailing newline.
///
/// The keys are `crate`, `version`, `sha1`, `result`, `held_by`, `manifest` and
/// `cached`. `crate` holds the name as crates.io stores it. `result` is `verified`,
/// `commit-missing`, `not-on-repo-ref`, `manifest-declares`, `no-vcs-info` or
/// `unsupported-host`. `sha1` prints as `-` when the release records no commit.
/// `held_by` names the branch or tag that holds the commit, or prints as `-`.
/// `manifest` names the manifest that declares the crate, or prints as `-`.
/// `cached` is `true` when the cache held the result.
pub fn render_link(release: &CrateRelease, outcome: &LinkOutcome) -> String {
    [
        format!("crate: {}", release.name),
        format!("version: {}", release.version),
        format!("sha1: {}", or_none(outcome.check.sha1())),
        format!("result: {}", outcome.check.label()),
        format!("held_by: {}", or_none(outcome.check.held_by())),
        format!("manifest: {}", or_none(outcome.check.manifest())),
        format!("cached: {}", outcome.cached),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use catalogue_signals::link::LinkCheck;

    use super::*;

    const SHA: &str = "a866b336f14aa57a07f0d0be9f8762746e64ecb4";

    fn release() -> CrateRelease {
        CrateRelease {
            name: "serde".to_owned(),
            version: "1.0.228".to_owned(),
        }
    }

    fn outcome(check: LinkCheck, cached: bool) -> LinkOutcome {
        LinkOutcome {
            check,
            cached,
            checked: Date::new(2026, 11, 3).expect("date"),
        }
    }

    #[test]
    fn a_verified_link_prints_every_key() {
        let check = LinkCheck::Verified {
            sha1: SHA.to_owned(),
            dirty: false,
            held_by: Some("refs/tags/v1.0.228".to_owned()),
        };
        assert_eq!(
            render_link(&release(), &outcome(check, false)),
            format!(
                "crate: serde\nversion: 1.0.228\nsha1: {SHA}\nresult: verified\nheld_by: refs/tags/v1.0.228\nmanifest: -\ncached: false"
            )
        );
    }

    #[test]
    fn a_missing_commit_keeps_the_sha_and_a_cache_hit_says_so() {
        let check = LinkCheck::CommitMissing {
            sha1: SHA.to_owned(),
            dirty: true,
        };
        let text = render_link(&release(), &outcome(check, true));
        assert!(
            text.contains(&format!("\nsha1: {SHA}\nresult: commit-missing\n")),
            "{text}"
        );
        assert!(text.ends_with("\ncached: true"), "{text}");
    }

    #[test]
    fn a_commit_that_no_ref_holds_keeps_the_sha_and_prints_its_label() {
        let check = LinkCheck::NotOnRepoRef {
            sha1: SHA.to_owned(),
            dirty: false,
        };
        let text = render_link(&release(), &outcome(check, false));
        assert!(
            text.contains(&format!(
                "\nsha1: {SHA}\nresult: not-on-repo-ref\nheld_by: -\n"
            )),
            "{text}"
        );
    }

    #[test]
    fn a_manifest_proof_prints_its_path() {
        let check = LinkCheck::ManifestDeclares {
            path: "crates/serde/Cargo.toml".to_owned(),
        };
        assert_eq!(
            render_link(&release(), &outcome(check, false)),
            "crate: serde\nversion: 1.0.228\nsha1: -\nresult: manifest-declares\nheld_by: -\nmanifest: crates/serde/Cargo.toml\ncached: false"
        );
    }

    #[test]
    fn results_without_a_commit_print_a_dash() {
        let text = render_link(&release(), &outcome(LinkCheck::NoVcsInfo, false));
        assert!(text.contains("\nsha1: -\nresult: no-vcs-info\n"), "{text}");
        let text = render_link(&release(), &outcome(LinkCheck::UnsupportedHost, false));
        assert!(
            text.contains("\nsha1: -\nresult: unsupported-host\n"),
            "{text}"
        );
    }
}
