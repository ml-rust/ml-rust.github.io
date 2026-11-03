//! The network-fetched values one check records, each with its data date.
//!
//! A later check reads them when the same fetch fails. A value keeps its first
//! date through every snapshot it passes through, so the age limit counts from
//! the real fetch.

use catalogue_core::Date;
use catalogue_core::date_text::{plain, plain_option};
use serde::{Deserialize, Serialize};

use crate::distribution::ReleaseDownloads;
use crate::github::{Facts, RepoFacts};
use crate::link::LinkCheck;

/// A fetched value with the date of its data and what it describes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dated<T> {
    /// The date of the data. It stays the same when a later check reuses the value.
    #[serde(serialize_with = "plain")]
    pub date: Date,
    /// What the fetch asked about, such as `owner/name`. A later check reuses
    /// the value only for the same subject.
    pub of: String,
    /// The value.
    pub value: T,
}

/// The repo facts the tier rules and the entry page read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RepoValues {
    /// GitHub holds no repo under the requested name.
    Gone,
    /// The repo exists.
    Found {
        /// Whether the repo is archived.
        archived: bool,
        /// The star count. Display only: no tier rule reads it.
        stars: u32,
        /// The date of the newest human commit, when the sample holds one.
        #[serde(serialize_with = "plain_option")]
        last_human_commit: Option<Date>,
        /// The age in days of the oldest unanswered outside item. `None` when
        /// no such item exists.
        oldest_unanswered_days: Option<u32>,
        /// The maintainer comments over the reply window.
        maintainer_replies_365: u32,
    },
}

impl RepoValues {
    /// The star count of an existing repo.
    pub fn stars(&self) -> Option<u64> {
        match self {
            Self::Found { stars, .. } => Some(u64::from(*stars)),
            Self::Gone => None,
        }
    }
}

impl From<&Facts> for RepoValues {
    fn from(facts: &Facts) -> Self {
        Self::Found {
            archived: facts.archived,
            stars: facts.stars,
            last_human_commit: facts.last_human_commit.map(Date::from),
            oldest_unanswered_days: facts.oldest_unanswered_days,
            maintainer_replies_365: facts.maintainer_replies_365,
        }
    }
}

impl From<&RepoFacts> for RepoValues {
    fn from(facts: &RepoFacts) -> Self {
        match facts {
            RepoFacts::Found(found) => Self::from(found),
            RepoFacts::Gone { .. } => Self::Gone,
        }
    }
}

/// The GitHub release counts the tier rules read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseValues {
    /// The binary downloads over the sampled releases.
    pub binary_downloads: u64,
    /// The sampled releases.
    pub releases: u32,
    /// The newest publish day among them.
    #[serde(serialize_with = "plain_option")]
    pub last_release: Option<Date>,
}

impl From<&ReleaseDownloads> for ReleaseValues {
    fn from(found: &ReleaseDownloads) -> Self {
        Self {
            binary_downloads: found.total_binary_downloads,
            releases: u32::try_from(found.releases.len()).unwrap_or(u32::MAX),
            last_release: found.releases.iter().filter_map(|r| r.published).max(),
        }
    }
}

/// Every network-fetched value of one entry on one check.
///
/// `None` means the source has no value. The source was not asked, gave a
/// definite "no such name", or failed with nothing to fall back on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkValues {
    /// The GitHub repo facts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facts: Option<Dated<RepoValues>>,
    /// The repo link check. `of` names the crate release and the repo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Dated<LinkCheck>>,
    /// The GitHub release counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub releases: Option<Dated<ReleaseValues>>,
    /// The Homebrew installs on request over 365 days.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homebrew: Option<Dated<u64>>,
    /// The Repology repository families.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repology: Option<Dated<u32>>,
    /// The Docker Hub pulls summed over the listed images.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docker: Option<Dated<u64>>,
}

impl NetworkValues {
    /// Whether no source has a value.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distribution::ReleaseSummary;

    fn day(on: i8) -> Date {
        Date::new(2026, 10, on).expect("date")
    }

    fn dated<T>(value: T) -> Option<Dated<T>> {
        Some(Dated {
            date: day(2),
            of: "a/b".to_owned(),
            value,
        })
    }

    fn full() -> NetworkValues {
        NetworkValues {
            facts: dated(RepoValues::Found {
                archived: true,
                stars: 12,
                last_human_commit: Some(day(1)),
                oldest_unanswered_days: None,
                maintainer_replies_365: 7,
            }),
            link: dated(LinkCheck::NoVcsInfo),
            releases: dated(ReleaseValues {
                binary_downloads: 900,
                releases: 3,
                last_release: Some(day(1)),
            }),
            homebrew: dated(640),
            repology: dated(2),
            docker: dated(5_000),
        }
    }

    #[test]
    fn every_source_round_trips_with_plain_dates() {
        let text = serde_json::to_string(&full()).expect("serializes");
        assert!(text.contains(r#""date":"2026-10-02""#), "{text}");
        assert!(
            text.contains(r#""last_human_commit":"2026-10-01""#),
            "{text}"
        );
        let back: NetworkValues = serde_json::from_str(&text).expect("parses");
        assert_eq!(back, full());
    }

    #[test]
    fn a_gone_repo_round_trips_and_has_no_stars() {
        let values = NetworkValues {
            facts: dated(RepoValues::Gone),
            ..NetworkValues::default()
        };
        let text = serde_json::to_string(&values).expect("serializes");
        assert_eq!(
            text,
            r#"{"facts":{"date":"2026-10-02","of":"a/b","value":{"state":"gone"}}}"#
        );
        let back: NetworkValues = serde_json::from_str(&text).expect("parses");
        assert_eq!(back, values);
        assert_eq!(RepoValues::Gone.stars(), None);
    }

    #[test]
    fn a_source_without_a_value_takes_no_space() {
        assert!(NetworkValues::default().is_empty());
        assert!(!full().is_empty());
        let text = serde_json::to_string(&NetworkValues::default()).expect("serializes");
        assert_eq!(text, "{}");
    }

    #[test]
    fn release_values_sum_the_sample() {
        let summary = |published| ReleaseSummary {
            tag: "v1".to_owned(),
            published,
            prerelease: false,
            binary_downloads: 1,
        };
        let found = ReleaseDownloads {
            releases: vec![summary(Some(day(1))), summary(Some(day(3))), summary(None)],
            total_binary_downloads: 3,
        };
        let values = ReleaseValues::from(&found);
        assert_eq!(values.binary_downloads, 3);
        assert_eq!(values.releases, 3);
        assert_eq!(values.last_release, Some(day(3)));
    }
}
