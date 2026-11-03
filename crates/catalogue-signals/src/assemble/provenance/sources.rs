//! The pages and data dates that back each cited signal.

use catalogue_core::{CRATES_IO_PAGE, Date, Entry};
use url::Url;

use crate::github::RepoRef;

/// The source name of every value read from the crates.io database dump.
pub const DUMP_SOURCE: &str = "crates.io database dump";

/// The crates.io page that describes the database dump.
pub const DUMP_PAGE: &str = "https://crates.io/data-access";

/// The start of a crate's page on the RustSec site.
pub const ADVISORY_PAGE: &str = "https://rustsec.org/packages/";

/// The workflow page of the sandbox build check.
pub const BUILD_CHECK_PAGE: &str =
    "https://github.com/ml-rust/ml-rust.github.io/actions/workflows/sandbox-check.yml";

/// The GitHub site, the base of every repo page.
const GITHUB: &str = "https://github.com";

/// What a check needs to cite every signal of one entry.
///
/// Pages come from the entry. Dates come from the run: the date of the data
/// each source supplied. A `None` date means the source supplied nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactSources {
    /// The date of the crates.io database dump.
    pub dump: Date,
    /// The crates.io page of the primary crate. `None` for an entry without a crate.
    pub crate_page: Option<Url>,
    /// The RustSec page of the primary crate. `None` for an entry without a crate.
    pub advisory_page: Option<Url>,
    /// The GitHub page of the repo. `None` when the repo is not on GitHub.
    pub repo_page: Option<Url>,
    /// The GitHub releases page of the repo. `None` when the repo is not on GitHub.
    pub releases_page: Option<Url>,
    /// The Homebrew page of the formula the entry names.
    pub homebrew_page: Option<Url>,
    /// The Repology page of the project the entry names.
    pub repology_page: Option<Url>,
    /// The Docker Hub page of the first image the entry lists.
    pub docker_page: Option<Url>,
    /// The date of the data behind each fetched source.
    pub dates: FetchDates,
}

/// The date of the data each source supplied on one check.
///
/// A fresh fetch carries the run date. A cached result carries the date it was read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FetchDates {
    /// The date of the RustSec advisory archive.
    pub rustsec: Option<Date>,
    /// The date of the GitHub repo facts.
    pub github: Option<Date>,
    /// The date of the repo link check.
    pub link: Option<Date>,
    /// The date of the GitHub release counts.
    pub releases: Option<Date>,
    /// The date of the Homebrew install counts.
    pub homebrew: Option<Date>,
    /// The date of the Repology packaging count.
    pub repology: Option<Date>,
    /// The date of the current Docker Hub pull count.
    pub docker: Option<Date>,
    /// The snapshot date and pull count that Docker growth measures against.
    pub docker_baseline: Option<(Date, u64)>,
}

impl FactSources {
    /// The sources of `entry`, whose GitHub repo is `repo`, on a dump dated `dump`.
    pub fn new(entry: &Entry, repo: Option<&RepoRef>, dump: Date, dates: FetchDates) -> Self {
        let primary = entry.crates.first().map(|name| name.as_str());
        let repo_page =
            repo.and_then(|repo| page(GITHUB, &[repo.owner.as_str(), repo.name.as_str()]));
        let releases_page = repo.and_then(|repo| {
            page(
                GITHUB,
                &[repo.owner.as_str(), repo.name.as_str(), "releases"],
            )
        });
        let distribution = entry.distribution.as_ref();
        let homebrew = distribution.and_then(|found| found.homebrew.as_deref());
        let repology = distribution.and_then(|found| found.repology.as_deref());
        let docker = distribution.and_then(|found| found.docker.first());
        Self {
            dump,
            crate_page: primary
                .and_then(|name| Url::parse(&format!("{CRATES_IO_PAGE}{name}")).ok()),
            advisory_page: primary
                .and_then(|name| Url::parse(&format!("{ADVISORY_PAGE}{name}.html")).ok()),
            repo_page,
            releases_page,
            homebrew_page: homebrew
                .and_then(|name| page("https://formulae.brew.sh/formula", &[name])),
            repology_page: repology
                .and_then(|name| page("https://repology.org/project", &[name, "versions"])),
            docker_page: docker.and_then(|image| {
                let parts: Vec<&str> = image.split('/').collect();
                page("https://hub.docker.com/r", &parts)
            }),
            dates,
        }
    }
}

/// `base` with each of `segments` appended as one encoded path segment.
fn page(base: &str, segments: &[&str]) -> Option<Url> {
    let mut url = Url::parse(base).ok()?;
    url.path_segments_mut()
        .ok()?
        .pop_if_empty()
        .extend(segments);
    Some(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::fixtures::library;

    fn day(on: i8) -> Date {
        Date::new(2026, 11, on).expect("date")
    }

    fn repo() -> RepoRef {
        RepoRef::parse("ml-rust/tool").expect("repo")
    }

    #[test]
    fn library_pages_come_from_the_primary_crate() {
        let entry = library(&["main", "helper"]);
        let sources = FactSources::new(&entry, Some(&repo()), day(2), FetchDates::default());
        assert_eq!(
            sources.crate_page.as_ref().map(Url::as_str),
            Some("https://crates.io/crates/main")
        );
        assert_eq!(
            sources.advisory_page.as_ref().map(Url::as_str),
            Some("https://rustsec.org/packages/main.html")
        );
        assert_eq!(sources.homebrew_page, None);
    }
}
