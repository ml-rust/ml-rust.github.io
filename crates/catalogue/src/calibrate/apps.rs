//! The `catalogue calibrate apps` report.

use std::path::{Path, PathBuf};

use catalogue_signals::calibrate::{Distribution, app_samples};
use catalogue_signals::distribution::{DistributionError, ReleaseDownloads, release_downloads};
use catalogue_signals::github::{GITHUB_INTERVAL, RepoRef};
use catalogue_signals::http::{IntervalPacer, Pacer, ReqwestTransport, Transport};

use super::command::CalibrateError;
use super::render::{points_for, render_distributions};
use crate::batch;
use crate::discover::{leads, search};
use crate::token::NoToken;

/// The signal name. It sums the latest 3 releases.
const SIGNAL: &str = "binary_downloads_last3";

/// The outcome of the release fetch for one lead.
enum Lookup {
    /// GitHub answered. `None` means the repo is unknown.
    Fetched(String, Option<ReleaseDownloads>),
    /// The fetch failed with an error that does not stop the run.
    Skipped,
}

/// Searches GitHub, fetches the releases of the leads and renders the signal.
pub(super) fn calibrate_apps(
    data: &Path,
    cache_dir: Option<PathBuf>,
    percentile: f64,
    limit: Option<usize>,
    token: Result<String, NoToken>,
) -> Result<String, CalibrateError> {
    let token = token?;
    let searched = search(data, cache_dir, &token)?;
    let leads = leads(&searched.found, &searched.linked);
    let names: Vec<&str> = leads
        .iter()
        .take(limit.unwrap_or(leads.len()))
        .map(|(item, _)| item.repo.full_name.as_str())
        .collect();
    let transport = ReqwestTransport::new().map_err(DistributionError::from)?;
    let mut pacer = IntervalPacer::new(GITHUB_INTERVAL);
    let lookups = batch::each(&names, &mut pacer, |name, pacer| {
        fetch_one(&transport, &token, name, pacer)
    })?;
    let mut downloads = Vec::new();
    let mut skipped = 0;
    for lookup in lookups {
        match lookup {
            Lookup::Fetched(name, found) => downloads.push((name, found)),
            Lookup::Skipped => skipped += 1,
        }
    }
    Ok(build_apps_report(
        leads.len(),
        &downloads,
        skipped,
        percentile,
    ))
}

/// Fetches the releases of one repo.
///
/// A rejected token (401) is the only error that stops the run. Any other
/// error, or a name that is not `owner/name`, skips the repo.
fn fetch_one(
    transport: &dyn Transport,
    token: &str,
    name: &str,
    pacer: &mut dyn Pacer,
) -> Result<Lookup, DistributionError> {
    let Ok(repo) = RepoRef::parse(name) else {
        return Ok(Lookup::Skipped);
    };
    match release_downloads(transport, token, &repo, pacer) {
        Ok(found) => Ok(Lookup::Fetched(name.to_owned(), found)),
        Err(error @ DistributionError::Status { status: 401, .. }) => Err(error),
        Err(_) => Ok(Lookup::Skipped),
    }
}

/// Renders the lead counts and the percentiles of the release downloads.
///
/// `leads` is the number of leads before `--limit`. `downloads` holds the repos
/// that were fetched, including repos that GitHub does not know. `skipped`
/// counts the repos whose fetch failed.
fn build_apps_report(
    leads: usize,
    downloads: &[(String, Option<ReleaseDownloads>)],
    skipped: usize,
    percentile: f64,
) -> String {
    let samples = app_samples(downloads);
    let distribution = Distribution::from_samples(SIGNAL, &samples, &points_for(percentile));
    format!(
        "calibration: GitHub leads from discover github with at least one release, binary downloads of the latest 3 releases\n\
         leads: {leads}\n\
         fetched: {}\n\
         skipped: {skipped}\n\n{}",
        downloads.len(),
        render_distributions(&[distribution], percentile)
    )
}

#[cfg(test)]
mod tests {
    use catalogue_signals::distribution::ReleaseSummary;

    use super::*;

    fn found(total: u64) -> Option<ReleaseDownloads> {
        Some(ReleaseDownloads {
            releases: vec![ReleaseSummary {
                tag: "v1".to_owned(),
                published: None,
                prerelease: false,
                binary_downloads: total,
            }],
            total_binary_downloads: total,
        })
    }

    #[test]
    fn the_report_counts_leads_fetches_and_skips() {
        let downloads = vec![
            ("a/a".to_owned(), found(10)),
            ("b/b".to_owned(), None),
            ("c/c".to_owned(), found(30)),
        ];
        let text = build_apps_report(7, &downloads, 2, 90.0);
        let expected = "leads: 7\nfetched: 3\nskipped: 2\n\n\
            signal: binary_downloads_last3\npopulation: 2\n\
            p50: 10.00\np75: 30.00\np90: 30.00\np95: 30.00\np99: 30.00\nat p90: 30.00";
        assert!(text.ends_with(expected), "{text}");
        assert!(text.starts_with("calibration: GitHub leads"), "{text}");
    }

    #[test]
    fn no_downloads_print_an_empty_population() {
        let text = build_apps_report(0, &[], 0, 90.0);
        assert!(text.contains("population: 0\np50: -"), "{text}");
    }
}
