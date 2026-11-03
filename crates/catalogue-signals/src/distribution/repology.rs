//! Reads which distribution repos package a project.

use std::collections::BTreeSet;

use serde::Deserialize;

use super::error::DistributionError;
use super::family::family;
use super::json::parse_json;
use crate::http::{Paced, Pacer, Transport, percent_encode};

/// The API name used in errors.
const API: &str = "Repology";

/// The project endpoint of the Repology API.
const ENDPOINT: &str = "https://repology.org/api/v1/project";

/// The package fields the lookup reads. Unknown fields are ignored.
#[derive(Deserialize)]
struct Package {
    repo: String,
}

/// Where a project is packaged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepologyPackaging {
    /// The distinct repo names that hold a package, sorted.
    pub repos: Vec<String>,
    /// The distinct families of those repos, as [`family`] defines them.
    /// Repos in [`NON_DISTRIBUTION_REPOS`] do not count.
    pub families: u32,
}

/// Repology repos that are not distributions: language package registries and
/// metadata sources. A crate on crates.io or a Wikidata record is not a
/// distribution package, so these repos add no family.
pub const NON_DISTRIBUTION_REPOS: [&str; 7] = [
    "cpan",
    "crates_io",
    "hackage",
    "npm",
    "pypi",
    "rubygems",
    "wikidata",
];

/// Reads the packaging of the Repology project `project`.
///
/// Returns `None` when Repology answers 404 or with an empty list, which is
/// how it answers for an unknown project.
///
/// # Errors
///
/// Returns a [`DistributionError`] when no response arrives, the status is not
/// 200 or 404, the body breaks off or is not the expected JSON, or the rate
/// limit stays after the retries.
pub fn repology_packaging(
    transport: &dyn Transport,
    project: &str,
    pacer: &mut dyn Pacer,
) -> Result<Option<RepologyPackaging>, DistributionError> {
    let url = format!("{ENDPOINT}/{}", percent_encode(project));
    let headers = [("Accept", "application/json".to_owned())];
    let mut paced = Paced::new(pacer);
    let Some(bytes) = paced
        .fetch_optional(&url, || transport.get(&url, &headers))
        .map_err(|error| DistributionError::paced(API, error))?
    else {
        return Ok(None);
    };
    let packages: Vec<Package> = parse_json(API, &url, &bytes)?;
    Ok(summarize(&packages))
}

/// The packaging of a package list, or `None` for an empty list.
fn summarize(packages: &[Package]) -> Option<RepologyPackaging> {
    let repos: BTreeSet<&str> = packages.iter().map(|p| p.repo.as_str()).collect();
    if repos.is_empty() {
        return None;
    }
    let families: BTreeSet<&str> = repos
        .iter()
        .filter(|repo| !NON_DISTRIBUTION_REPOS.contains(repo))
        .map(|repo| family(repo))
        .collect();
    Some(RepologyPackaging {
        families: u32::try_from(families.len()).unwrap_or(u32::MAX),
        repos: repos.into_iter().map(str::to_owned).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packages(repos: &[&str]) -> Vec<Package> {
        repos
            .iter()
            .map(|repo| Package {
                repo: (*repo).to_owned(),
            })
            .collect()
    }

    #[test]
    fn repos_are_distinct_and_sorted_and_families_group_versions() {
        let found = summarize(&packages(&[
            "debian_unstable",
            "homebrew",
            "debian_12",
            "debian_12",
            "ubuntu_24_04",
            "fedora_39",
        ]))
        .expect("packaged");
        assert_eq!(
            found.repos,
            [
                "debian_12",
                "debian_unstable",
                "fedora_39",
                "homebrew",
                "ubuntu_24_04"
            ]
        );
        assert_eq!(found.families, 4);
    }

    #[test]
    fn non_distribution_repos_count_as_repos_but_add_no_family() {
        let found =
            summarize(&packages(&["crates_io", "debian_12", "wikidata"])).expect("packaged");
        assert_eq!(found.repos, ["crates_io", "debian_12", "wikidata"]);
        assert_eq!(found.families, 1);
    }

    #[test]
    fn an_empty_list_is_none() {
        assert_eq!(summarize(&[]), None);
    }
}
