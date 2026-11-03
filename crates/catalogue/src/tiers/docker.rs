//! The Docker Hub pull counts of app entries.

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, Entry, EntryKind, Slug};
use catalogue_signals::distribution::{DOCKER_INTERVAL, docker_pulls};
use catalogue_signals::fallback::{Failed, Source, attempt, distribution_failure};
use catalogue_signals::http::{IntervalPacer, Transport};

use super::error::TiersError;
use crate::batch;
use crate::distribution::split_docker_name;

/// The pull count of one app and the notes about its images.
#[derive(Debug, Default)]
pub(super) struct DockerCount {
    /// The images the app lists, as one text: what the count describes.
    pub(super) of: String,
    /// Whether any request went out.
    pub(super) requested: bool,
    /// The pulls summed over the images Docker Hub knows. `None` when it knows none.
    pub(super) pulls: Option<u64>,
    /// The first error that leaves the sum short. A sum without one image
    /// understates the count, so the whole count falls back.
    pub(super) error: Option<String>,
    /// One note per image that has no count.
    pub(super) notes: Vec<String>,
}

/// The Docker Hub images an app entry lists. Empty for a library.
fn images(entry: &Entry) -> &[String] {
    match (&entry.kind, &entry.distribution) {
        (EntryKind::App, Some(distribution)) => &distribution.docker,
        _ => &[],
    }
}

/// Fetches the pull count of every app that lists Docker images, keyed by slug.
///
/// - The count sums the pulls of the listed images that Docker Hub knows.
/// - An image Docker Hub does not know, or whose name is not `namespace/repo`,
///   becomes a note and adds nothing.
/// - An app whose images are all unknown has no count.
/// - A fetch error on any image sets [`DockerCount::error`], unless it is fatal.
/// - One pacer spaces all requests.
pub(super) fn fetch_docker(
    catalogue: &Catalogue,
    transport: &dyn Transport,
) -> Result<BTreeMap<Slug, DockerCount>, TiersError> {
    let mut out: BTreeMap<Slug, DockerCount> = BTreeMap::new();
    let mut requests: Vec<(&Slug, &str, &str, &str)> = Vec::new();
    for (slug, entry) in catalogue.entries() {
        for image in images(entry) {
            let count = out.entry(slug.clone()).or_default();
            count.of = images(entry).join(", ");
            match split_docker_name(image) {
                Ok((namespace, repo)) => {
                    count.requested = true;
                    requests.push((slug, image.as_str(), namespace, repo));
                }
                Err(error) => count.notes.push(error.to_string()),
            }
        }
    }
    let mut pacer = IntervalPacer::new(DOCKER_INTERVAL);
    let found = batch::each(&requests, &mut pacer, |&(_, _, namespace, repo), pacer| {
        attempt(docker_pulls(transport, namespace, repo, pacer), |error| {
            distribution_failure(error, Source::Docker)
        })
    })?;
    for ((slug, image, _, _), found) in requests.iter().zip(found) {
        let count = out.entry((*slug).clone()).or_default();
        match found {
            Ok(Some(found)) => {
                count.pulls = Some(count.pulls.unwrap_or(0).saturating_add(found.pull_count));
            }
            Ok(None) => count.notes.push(format!("Docker Hub has no image {image}")),
            Err(Failed::Definitive(error)) => count.notes.push(error),
            Err(Failed::Fallback(error)) => {
                count.error.get_or_insert(error);
            }
        }
    }
    Ok(out)
}
