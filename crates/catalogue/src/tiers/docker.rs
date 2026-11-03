//! The Docker Hub pull counts of app entries.

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, Entry, EntryKind, Slug};
use catalogue_signals::distribution::{DOCKER_INTERVAL, docker_pulls};
use catalogue_signals::http::{IntervalPacer, Transport};

use super::error::TiersError;
use crate::batch;
use crate::distribution::split_docker_name;

/// The pull count of one app and the notes about its images.
#[derive(Debug, Default)]
pub(super) struct DockerCount {
    /// The pulls summed over the images Docker Hub knows. `None` when it knows none.
    pub(super) pulls: Option<u64>,
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
/// - One pacer spaces all requests.
pub(super) fn fetch_docker(
    catalogue: &Catalogue,
    transport: &dyn Transport,
) -> Result<BTreeMap<Slug, DockerCount>, TiersError> {
    let mut out: BTreeMap<Slug, DockerCount> = BTreeMap::new();
    let mut requests: Vec<(&Slug, &str, &str, &str)> = Vec::new();
    for (slug, entry) in catalogue.entries() {
        for image in images(entry) {
            match split_docker_name(image) {
                Ok((namespace, repo)) => requests.push((slug, image.as_str(), namespace, repo)),
                Err(error) => out
                    .entry(slug.clone())
                    .or_default()
                    .notes
                    .push(error.to_string()),
            }
        }
    }
    let mut pacer = IntervalPacer::new(DOCKER_INTERVAL);
    let found = batch::each(&requests, &mut pacer, |&(_, _, namespace, repo), pacer| {
        docker_pulls(transport, namespace, repo, pacer)
    })?;
    for ((slug, image, _, _), found) in requests.iter().zip(found) {
        let count = out.entry((*slug).clone()).or_default();
        match found {
            Some(found) => {
                count.pulls = Some(count.pulls.unwrap_or(0).saturating_add(found.pull_count));
            }
            None => count.notes.push(format!("Docker Hub has no image {image}")),
        }
    }
    Ok(out)
}
