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
    /// The default branch of the repo contains the release commit.
    Verified {
        /// The commit the release records, as 40 lowercase hex digits.
        sha1: String,
        /// Whether the release was packaged from a dirty working tree.
        #[serde(default)]
        dirty: bool,
    },
    /// The release commit is absent from the repo.
    CommitMissing {
        /// The commit the release records, as 40 lowercase hex digits.
        sha1: String,
        /// Whether the release was packaged from a dirty working tree.
        #[serde(default)]
        dirty: bool,
    },
    /// The release commit exists on GitHub, but the default branch does not
    /// contain it. A commit that only a fork holds gives this result.
    NotOnDefaultBranch {
        /// The commit the release records, as 40 lowercase hex digits.
        sha1: String,
        /// Whether the release was packaged from a dirty working tree.
        #[serde(default)]
        dirty: bool,
    },
    /// The release records no commit, so the check cannot run.
    NoVcsInfo,
    /// The repo is not on GitHub, so the check cannot run.
    UnsupportedHost,
}

impl LinkCheck {
    /// The `repo_link_verified` signal: `Some(true)` for a found commit,
    /// `Some(false)` for a missing commit, a commit off the default branch or a
    /// release without commit data,
    /// `None` when the host is unsupported.
    pub fn verified(&self) -> Option<bool> {
        match self {
            Self::Verified { .. } => Some(true),
            Self::CommitMissing { .. } | Self::NotOnDefaultBranch { .. } | Self::NoVcsInfo => {
                Some(false)
            }
            Self::UnsupportedHost => None,
        }
    }

    /// The commit the release records, when it records one.
    pub fn sha1(&self) -> Option<&str> {
        match self {
            Self::Verified { sha1, .. }
            | Self::CommitMissing { sha1, .. }
            | Self::NotOnDefaultBranch { sha1, .. } => Some(sha1),
            Self::NoVcsInfo | Self::UnsupportedHost => None,
        }
    }

    /// Whether the release was packaged from a dirty working tree.
    pub fn dirty(&self) -> bool {
        matches!(
            self,
            Self::Verified { dirty: true, .. }
                | Self::CommitMissing { dirty: true, .. }
                | Self::NotOnDefaultBranch { dirty: true, .. }
        )
    }

    /// The result name: `verified`, `commit-missing`, `not-on-default-branch`,
    /// `no-vcs-info` or `unsupported-host`.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Verified { .. } => "verified",
            Self::CommitMissing { .. } => "commit-missing",
            Self::NotOnDefaultBranch { .. } => "not-on-default-branch",
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
        }
    }

    #[test]
    fn the_signal_is_false_for_a_missing_commit_and_unknown_for_an_unsupported_host() {
        let missing = LinkCheck::CommitMissing {
            sha1: SHA.to_owned(),
            dirty: false,
        };
        let off_branch = LinkCheck::NotOnDefaultBranch {
            sha1: SHA.to_owned(),
            dirty: false,
        };
        assert_eq!(found(false).verified(), Some(true));
        assert_eq!(missing.verified(), Some(false));
        assert_eq!(off_branch.verified(), Some(false));
        assert_eq!(off_branch.label(), "not-on-default-branch");
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
    }

    #[test]
    fn a_result_round_trips_through_json() {
        for check in [
            found(true),
            LinkCheck::NotOnDefaultBranch {
                sha1: SHA.to_owned(),
                dirty: true,
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
