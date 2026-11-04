//! The facts a repo query yields.

use jiff::Timestamp;
use jiff::civil;
use serde::{Deserialize, Serialize};

use super::repo_ref::RepoRef;

/// The `spdxId` values GitHub gives a license it cannot name. Compared without regard to case.
const UNNAMED_LICENSES: [&str; 2] = ["NOASSERTION", "other"];

/// The license GitHub states for a repo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepoLicense {
    /// An SPDX identifier.
    Stated(String),
    /// GitHub finds no license file.
    NoneStated,
    /// GitHub finds a license file and cannot name it, or a snapshot predates this field.
    #[default]
    NotMeasured,
}

impl RepoLicense {
    /// The license of an answer. `info` is the `licenseInfo` object, read as its `spdxId`.
    pub(super) fn of(info: Option<Option<&str>>) -> Self {
        match info {
            None => Self::NoneStated,
            Some(None) => Self::NotMeasured,
            Some(Some(spdx)) => {
                let unnamed = spdx.trim().is_empty()
                    || UNNAMED_LICENSES
                        .iter()
                        .any(|value| spdx.eq_ignore_ascii_case(value));
                if unnamed {
                    Self::NotMeasured
                } else {
                    Self::Stated(spdx.to_owned())
                }
            }
        }
    }
}

/// The combined state of the checks on the default branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiState {
    /// Every check passes.
    Success,
    /// A check fails.
    Failure,
    /// A check still runs.
    Pending,
    /// A check ends in an error.
    Error,
    /// A required check has not started.
    Expected,
}

impl CiState {
    /// The lowercase name printed in reports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Pending => "pending",
            Self::Error => "error",
            Self::Expected => "expected",
        }
    }
}

/// The answer for one requested repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoFacts {
    /// GitHub holds no repo under the requested name.
    Gone {
        /// The requested repo.
        requested: RepoRef,
    },
    /// The repo exists.
    Found(Facts),
}

/// The facts of an existing repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// The current `owner/name` as GitHub spells it.
    pub name_with_owner: String,
    /// Whether the current name differs from the requested one, ignoring case.
    pub moved: bool,
    /// Whether the repo is archived.
    pub archived: bool,
    /// Whether the repo is a fork.
    pub fork: bool,
    /// The time of the last push. `None` for a repo with no push.
    pub pushed_at: Option<Timestamp>,
    /// The star count.
    pub stars: u32,
    /// The fork count. `None` when the answer lacks it.
    pub forks: Option<u32>,
    /// The total commits on the default branch. `None` when the repo has no head commit or the
    /// answer lacks the count.
    pub commits: Option<u64>,
    /// The check state of the default branch head. `None` when it has no checks.
    pub ci: Option<CiState>,
    /// The license GitHub states.
    pub license: RepoLicense,
    /// The date of the newest human commit, however old. `None` when the
    /// repo has no human commit or when [`Facts::last_human_commit_beyond_sample`] is set.
    pub last_human_commit: Option<civil::Date>,
    /// Whether the newest 30 commits are all bot commits and the window holds
    /// no human commit, so the newest human commit lies beyond the sample.
    pub last_human_commit_beyond_sample: bool,
    /// The human commits among the newest 100 of the last 365 days.
    pub human_commits_365: u32,
    /// Whether the window holds more than 100 commits, so the count is a lower bound.
    pub human_commits_capped: bool,
    /// The age in days of the oldest open item from a non-member that no
    /// maintainer answered.
    pub oldest_unanswered_days: Option<u32>,
    /// The maintainer comments from the last 365 days across the sampled items.
    pub maintainer_replies_365: u32,
    /// The median days from creation to the first maintainer comment, over
    /// non-member items created in the last 365 days.
    pub median_first_response_days: Option<u32>,
    /// The issues and pull requests sampled: the newest 50 of each.
    pub items_sampled: u32,
}
