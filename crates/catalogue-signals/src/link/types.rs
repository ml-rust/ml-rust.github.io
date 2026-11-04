//! The types the link check takes and returns.

use catalogue_core::Date;
use serde::{Deserialize, Serialize};

/// One published version of a crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateRelease {
    /// The crate name as crates.io stores it.
    pub name: String,
    /// The version number.
    pub version: String,
}

/// What the link check finds for one crate release and repo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LinkCheck {
    /// A branch or tag of the repo holds the release commit.
    Verified {
        /// The commit the release records, as 40 lowercase hex digits.
        sha1: String,
        /// Whether the release was packaged from a dirty working tree.
        #[serde(default)]
        dirty: bool,
        /// The full name of the ref that holds the commit. A result from an
        /// older rule has none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        held_by: Option<String>,
    },
    /// The release commit is absent from the repo.
    CommitMissing {
        /// The commit the release records, as 40 lowercase hex digits.
        sha1: String,
        /// Whether the release was packaged from a dirty working tree.
        #[serde(default)]
        dirty: bool,
    },
    /// The release commit exists on GitHub, but no branch or tag of the repo
    /// holds it. A commit that only a fork holds gives this result. A history
    /// snapshot from an older rule names it `not-on-default-branch`.
    #[serde(alias = "not-on-default-branch")]
    NotOnRepoRef {
        /// The commit the release records, as 40 lowercase hex digits.
        sha1: String,
        /// Whether the release was packaged from a dirty working tree.
        #[serde(default)]
        dirty: bool,
    },
    /// No commit proof applies, and a `Cargo.toml` on the repo's default branch
    /// declares the crate.
    ManifestDeclares {
        /// The manifest path in the repo.
        path: String,
    },
    /// The release records no commit, and no manifest in the repo declares the crate.
    NoVcsInfo,
    /// The repo is not on GitHub, and no manifest proof finds the crate there.
    UnsupportedHost,
}

impl LinkCheck {
    /// The `repo_link_verified` signal: `Some(true)` for a found commit or a
    /// manifest that declares the crate, `Some(false)` for a missing commit, a
    /// commit that no ref of the repo holds or a release with neither commit data
    /// nor a manifest, `None` for a repo off GitHub with no manifest.
    pub fn verified(&self) -> Option<bool> {
        match self {
            Self::Verified { .. } | Self::ManifestDeclares { .. } => Some(true),
            Self::CommitMissing { .. } | Self::NotOnRepoRef { .. } | Self::NoVcsInfo => Some(false),
            Self::UnsupportedHost => None,
        }
    }

    /// The full name of the ref that holds the commit, when the check names one.
    pub fn held_by(&self) -> Option<&str> {
        match self {
            Self::Verified { held_by, .. } => held_by.as_deref(),
            _ => None,
        }
    }

    /// The path of the manifest that declares the crate, when the check names one.
    pub fn manifest(&self) -> Option<&str> {
        match self {
            Self::ManifestDeclares { path } => Some(path),
            _ => None,
        }
    }

    /// The commit the release records, when the check read one.
    pub fn sha1(&self) -> Option<&str> {
        match self {
            Self::Verified { sha1, .. }
            | Self::CommitMissing { sha1, .. }
            | Self::NotOnRepoRef { sha1, .. } => Some(sha1),
            Self::ManifestDeclares { .. } | Self::NoVcsInfo | Self::UnsupportedHost => None,
        }
    }

    /// Whether the release was packaged from a dirty working tree.
    pub fn dirty(&self) -> bool {
        matches!(
            self,
            Self::Verified { dirty: true, .. }
                | Self::CommitMissing { dirty: true, .. }
                | Self::NotOnRepoRef { dirty: true, .. }
        )
    }

    /// The result name: `verified`, `commit-missing`, `not-on-repo-ref`,
    /// `manifest-declares`, `no-vcs-info` or `unsupported-host`.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Verified { .. } => "verified",
            Self::CommitMissing { .. } => "commit-missing",
            Self::NotOnRepoRef { .. } => "not-on-repo-ref",
            Self::ManifestDeclares { .. } => "manifest-declares",
            Self::NoVcsInfo => "no-vcs-info",
            Self::UnsupportedHost => "unsupported-host",
        }
    }
}

/// A link check and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkOutcome {
    /// The result.
    pub check: LinkCheck,
    /// Whether the cache held the result, so no request was sent.
    pub cached: bool,
    /// The UTC date the check ran: the run date for a new check, the stored date for a cached one.
    pub checked: Date,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "a866b336f14aa57a07f0d0be9f8762746e64ecb4";

    fn found(dirty: bool) -> LinkCheck {
        LinkCheck::Verified {
            sha1: SHA.to_owned(),
            dirty,
            held_by: None,
        }
    }

    #[test]
    fn the_signal_is_false_for_a_missing_commit_and_unknown_for_an_unsupported_host() {
        let missing = LinkCheck::CommitMissing {
            sha1: SHA.to_owned(),
            dirty: false,
        };
        let not_held = LinkCheck::NotOnRepoRef {
            sha1: SHA.to_owned(),
            dirty: false,
        };
        assert_eq!(found(false).verified(), Some(true));
        assert_eq!(missing.verified(), Some(false));
        assert_eq!(not_held.verified(), Some(false));
        assert_eq!(not_held.label(), "not-on-repo-ref");
        assert_eq!(LinkCheck::NoVcsInfo.verified(), Some(false));
        assert_eq!(LinkCheck::UnsupportedHost.verified(), None);
    }

    #[test]
    fn accessors_report_the_commit_the_flag_and_the_label() {
        assert_eq!(found(true).sha1(), Some(SHA));
        assert!(found(true).dirty() && !found(false).dirty());
        assert_eq!(LinkCheck::NoVcsInfo.sha1(), None);
        assert!(!LinkCheck::UnsupportedHost.dirty());
        assert_eq!(found(false).label(), "verified");
        assert_eq!(LinkCheck::NoVcsInfo.label(), "no-vcs-info");
        assert_eq!(found(false).held_by(), None);
        let on_tag = LinkCheck::Verified {
            sha1: SHA.to_owned(),
            dirty: false,
            held_by: Some("refs/tags/v1.0.0".to_owned()),
        };
        assert_eq!(on_tag.held_by(), Some("refs/tags/v1.0.0"));
        assert_eq!(LinkCheck::NoVcsInfo.held_by(), None);
    }

    #[test]
    fn a_manifest_proof_verifies_and_names_its_path() {
        let check = LinkCheck::ManifestDeclares {
            path: "crates/a/Cargo.toml".to_owned(),
        };
        assert_eq!(check.verified(), Some(true));
        assert_eq!(check.label(), "manifest-declares");
        assert_eq!(check.manifest(), Some("crates/a/Cargo.toml"));
        assert_eq!(check.sha1(), None);
        assert_eq!(check.held_by(), None);
        assert!(!check.dirty());
        assert_eq!(found(false).manifest(), None);
        assert_eq!(
            serde_json::to_string(&check).expect("serializes"),
            r#"{"kind":"manifest-declares","path":"crates/a/Cargo.toml"}"#
        );
    }

    #[test]
    fn a_snapshot_from_an_older_rule_reads_as_the_new_result() {
        let old: LinkCheck = serde_json::from_str(&format!(
            r#"{{"kind":"not-on-default-branch","sha1":"{SHA}","dirty":false}}"#
        ))
        .expect("alias");
        assert_eq!(
            old,
            LinkCheck::NotOnRepoRef {
                sha1: SHA.to_owned(),
                dirty: false,
            }
        );
    }

    #[test]
    fn a_result_round_trips_through_json() {
        for check in [
            found(true),
            LinkCheck::Verified {
                sha1: SHA.to_owned(),
                dirty: false,
                held_by: Some("refs/heads/release/0.22".to_owned()),
            },
            LinkCheck::NotOnRepoRef {
                sha1: SHA.to_owned(),
                dirty: true,
            },
            LinkCheck::ManifestDeclares {
                path: "Cargo.toml".to_owned(),
            },
            LinkCheck::NoVcsInfo,
            LinkCheck::UnsupportedHost,
        ] {
            let text = serde_json::to_string(&check).expect("serializes");
            assert_eq!(
                serde_json::from_str::<LinkCheck>(&text).expect("parses"),
                check
            );
        }
        let old: LinkCheck =
            serde_json::from_str(&format!(r#"{{"kind":"verified","sha1":"{SHA}"}}"#))
                .expect("dirty defaults");
        assert_eq!(old, found(false));
    }
}
