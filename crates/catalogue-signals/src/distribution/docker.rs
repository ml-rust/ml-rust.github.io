//! Reads the pull count of a Docker Hub repo.

use jiff::Timestamp;
use serde::Deserialize;

use super::error::DistributionError;
use super::json::{parse_json, parse_timestamp};
use crate::http::{Paced, Pacer, Transport, percent_encode};

/// The API name used in errors.
const API: &str = "Docker Hub";

/// The repository endpoint of the Docker Hub API.
const ENDPOINT: &str = "https://hub.docker.com/v2/repositories";

/// The repository fields the lookup reads. Unknown fields are ignored.
#[derive(Deserialize)]
struct Repository {
    pull_count: u64,
    last_updated: Option<String>,
}

/// The pull signals of a Docker Hub repo.
///
/// Growth needs two snapshots. One lookup gives the current count only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockerPulls {
    /// The pulls since the repo exists.
    pub pull_count: u64,
    /// When the repo last changed. `None` when Docker Hub omits the field.
    pub last_updated: Option<Timestamp>,
}

/// Reads the pull count of the Docker Hub repo `namespace/repo`.
///
/// Returns `None` when Docker Hub does not know the repo.
///
/// # Errors
///
/// Returns a [`DistributionError`] when no response arrives, the status is not
/// 200 or 404, the body breaks off or is not the expected JSON, `last_updated`
/// is not a timestamp, or the rate limit stays after the retries.
pub fn docker_pulls(
    transport: &dyn Transport,
    namespace: &str,
    repo: &str,
    pacer: &mut dyn Pacer,
) -> Result<Option<DockerPulls>, DistributionError> {
    let url = format!(
        "{ENDPOINT}/{}/{}/",
        percent_encode(namespace),
        percent_encode(repo)
    );
    let headers = [("Accept", "application/json".to_owned())];
    let mut paced = Paced::new(pacer);
    let Some(bytes) = paced
        .fetch_optional(&url, || transport.get(&url, &headers))
        .map_err(|error| DistributionError::paced(API, error))?
    else {
        return Ok(None);
    };
    let repository: Repository = parse_json(API, &url, &bytes)?;
    let last_updated = repository
        .last_updated
        .as_deref()
        .map(|text| parse_timestamp(API, &url, "last_updated", text))
        .transpose()?;
    Ok(Some(DockerPulls {
        pull_count: repository.pull_count,
        last_updated,
    }))
}
