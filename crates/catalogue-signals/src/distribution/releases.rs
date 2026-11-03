//! Reads the binary download counts of the latest GitHub releases.

use catalogue_core::Date;
use jiff::tz::TimeZone;
use serde::Deserialize;

use super::assets::is_binary_asset;
use super::error::DistributionError;
use super::json::{parse_json, parse_timestamp};
use crate::github::{RepoRef, rest_headers};
use crate::http::{Paced, Pacer, Transport, percent_encode};

/// The API name used in errors.
const API: &str = "GitHub Releases";

/// The repos endpoint of the GitHub API.
const ENDPOINT: &str = "https://api.github.com/repos";

/// The most non-draft releases the lookup sums.
pub const RELEASE_SAMPLE: usize = 3;

/// The release fields the lookup reads. Unknown fields are ignored.
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

/// The asset fields the lookup reads.
#[derive(Deserialize)]
struct Asset {
    name: String,
    #[serde(default)]
    download_count: u64,
}

/// One release and its binary downloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseSummary {
    /// The git tag of the release.
    pub tag: String,
    /// The UTC day GitHub published the release. `None` when GitHub gives none.
    pub published: Option<Date>,
    /// Whether the release is a prerelease.
    pub prerelease: bool,
    /// The downloads of its binary assets, which exclude checksum and
    /// signature files.
    pub binary_downloads: u64,
}

/// The binary downloads of the latest releases of a repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseDownloads {
    /// The latest non-draft releases, newest first as GitHub lists them.
    pub releases: Vec<ReleaseSummary>,
    /// The sum of `binary_downloads` over `releases`.
    pub total_binary_downloads: u64,
}

/// Reads the binary downloads of the latest [`RELEASE_SAMPLE`] non-draft
/// releases of `repo`.
///
/// `token` is sent as a bearer token. The request asks for
/// [`RELEASE_SAMPLE`] releases, so a draft in that page leaves fewer. A 403 or
/// 429 answer with a wait hint is retried after the `pacer` waits. Returns
/// `None` when GitHub answers 404.
///
/// # Errors
///
/// Returns a [`DistributionError`] when no response arrives, the status is not
/// 200 or 404, the body breaks off or is not the expected JSON, a publish
/// time is not a timestamp, or the rate limit stays after the retries.
pub fn release_downloads(
    transport: &dyn Transport,
    token: &str,
    repo: &RepoRef,
    pacer: &mut dyn Pacer,
) -> Result<Option<ReleaseDownloads>, DistributionError> {
    let url = format!(
        "{ENDPOINT}/{}/{}/releases?per_page={RELEASE_SAMPLE}",
        percent_encode(&repo.owner),
        percent_encode(&repo.name)
    );
    let headers = rest_headers(token, "application/vnd.github+json");
    let mut paced = Paced::new(pacer);
    let Some(bytes) = paced
        .fetch_optional(&url, || transport.get(&url, &headers))
        .map_err(|error| DistributionError::paced(API, error))?
    else {
        return Ok(None);
    };
    let listed: Vec<Release> = parse_json(API, &url, &bytes)?;
    summarize(&url, listed).map(Some)
}

/// Sums the binary downloads of the first non-draft releases.
fn summarize(url: &str, listed: Vec<Release>) -> Result<ReleaseDownloads, DistributionError> {
    let mut releases = Vec::new();
    let mut total: u64 = 0;
    for release in listed.into_iter().filter(|r| !r.draft).take(RELEASE_SAMPLE) {
        let binary_downloads = release
            .assets
            .iter()
            .filter(|asset| is_binary_asset(&asset.name))
            .fold(0u64, |sum, asset| sum.saturating_add(asset.download_count));
        total = total.saturating_add(binary_downloads);
        releases.push(ReleaseSummary {
            published: release
                .published_at
                .as_deref()
                .map(|text| day(url, text))
                .transpose()?,
            tag: release.tag_name,
            prerelease: release.prerelease,
            binary_downloads,
        });
    }
    Ok(ReleaseDownloads {
        releases,
        total_binary_downloads: total,
    })
}

/// The UTC day of the timestamp `text`.
fn day(url: &str, text: &str) -> Result<Date, DistributionError> {
    let date = parse_timestamp(API, url, "published_at", text)?
        .to_zoned(TimeZone::UTC)
        .date();
    Date::new(date.year(), date.month(), date.day()).map_err(|_| DistributionError::BadField {
        api: API,
        url: url.to_owned(),
        field: "published_at",
        value: text.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, draft: bool, assets: &[(&str, u64)]) -> Release {
        Release {
            tag_name: tag.to_owned(),
            published_at: Some("2026-07-15T23:59:59Z".to_owned()),
            draft,
            prerelease: false,
            assets: assets
                .iter()
                .map(|(name, download_count)| Asset {
                    name: (*name).to_owned(),
                    download_count: *download_count,
                })
                .collect(),
        }
    }

    #[test]
    fn the_day_is_the_utc_date() {
        let date = day("u", "2026-07-15T23:59:59Z").expect("date");
        assert_eq!(date, Date::new(2026, 7, 15).expect("date"));
        assert!(day("u", "yesterday").is_err());
    }

    #[test]
    fn drafts_are_skipped_before_the_sample_is_taken() {
        let listed = vec![
            release("v5", true, &[("a.zip", 9)]),
            release("v4", false, &[("a.zip", 4)]),
            release("v3", false, &[("a.zip", 3)]),
            release("v2", false, &[("a.zip", 2)]),
            release("v1", false, &[("a.zip", 1)]),
        ];
        let found = summarize("u", listed).expect("summary");
        let tags: Vec<&str> = found.releases.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(tags, ["v4", "v3", "v2"]);
        assert_eq!(found.total_binary_downloads, 9);
    }

    #[test]
    fn sums_cap_instead_of_overflowing() {
        let listed = vec![release("v1", false, &[("a.zip", u64::MAX), ("b.zip", 1)])];
        assert_eq!(
            summarize("u", listed)
                .expect("summary")
                .total_binary_downloads,
            u64::MAX
        );
    }
}
