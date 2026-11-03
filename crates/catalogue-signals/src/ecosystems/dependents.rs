//! Reads the dependent counts of a crate.

use serde::Deserialize;

use super::error::EcosystemsError;
use super::types::CrateDependents;
use crate::http::{Paced, Pacer, Transport, percent_encode};

/// The package endpoint of the crates.io registry.
const ENDPOINT: &str = "https://packages.ecosyste.ms/api/v1/registries/crates.io/packages";

/// The package fields the lookup reads. A missing field is `None`.
#[derive(Deserialize)]
struct Package {
    dependent_packages_count: Option<u64>,
    dependent_repos_count: Option<u64>,
    docker_dependents_count: Option<u64>,
    docker_downloads_count: Option<u64>,
}

/// Reads the dependent counts of the crates.io package `name`.
///
/// Returns `None` when ecosyste.ms does not know the package. A 403 or 429
/// answer with a wait hint is retried after the `pacer` waits.
///
/// # Errors
///
/// Returns an [`EcosystemsError`] when no response arrives, the status is not
/// 200 or 404, the body breaks off or is not the expected JSON, or the rate
/// limit stays after the retries.
pub fn crate_dependents(
    transport: &dyn Transport,
    name: &str,
    pacer: &mut dyn Pacer,
) -> Result<Option<CrateDependents>, EcosystemsError> {
    let url = format!("{ENDPOINT}/{}", percent_encode(name));
    let headers = [("Accept", "application/json".to_owned())];
    let mut paced = Paced::new(pacer);
    let Some(bytes) = paced.fetch_optional(&url, || transport.get(&url, &headers))? else {
        return Ok(None);
    };
    let package: Package =
        serde_json::from_slice(&bytes).map_err(|source| EcosystemsError::Json {
            url: url.clone(),
            source,
        })?;
    Ok(Some(package.into()))
}

impl From<Package> for CrateDependents {
    fn from(package: Package) -> Self {
        let mut fields_missing = Vec::new();
        let mut take = |name: &'static str, value: Option<u64>| {
            value.unwrap_or_else(|| {
                fields_missing.push(name);
                0
            })
        };
        let packages = take("dependent_packages_count", package.dependent_packages_count);
        let repos = take("dependent_repos_count", package.dependent_repos_count);
        let images = take("docker_dependents_count", package.docker_dependents_count);
        let downloads = take("docker_downloads_count", package.docker_downloads_count);
        Self {
            dependent_packages: saturate(packages),
            dependent_repos: saturate(repos),
            docker_dependents: saturate(images),
            docker_downloads: downloads,
            fields_missing,
        }
    }
}

/// A count as `u32`, capped at the largest value.
fn saturate(count: u64) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_above_u32_are_capped() {
        assert_eq!(saturate(5), 5);
        assert_eq!(saturate(u64::MAX), u32::MAX);
    }
}
