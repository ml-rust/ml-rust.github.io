//! The `catalogue calibrate crates` report.

use std::path::{Path, PathBuf};

use catalogue_signals::calibrate::crate_samples;
use catalogue_signals::discover::{self, Rules};
use catalogue_signals::dump::{DumpSnapshot, IdSet, utc_date};
use catalogue_signals::fetch;
use catalogue_signals::usage;

use super::command::CalibrateError;
use super::render::{points_for, render_distributions};
use crate::cache_dir::resolve_cache_dir;
use crate::discover::{crate_rules, load_catalogue};
use crate::dump::{DumpCommandError, read_cached};

/// Days of download history read from the dump. The signals need none.
const WINDOW_DAYS: u16 = 1;

/// Checks the data, reads the cached dump and renders the crate signals.
pub(super) fn calibrate_crates(
    data: &Path,
    cache_dir: Option<PathBuf>,
    percentile: f64,
) -> Result<String, CalibrateError> {
    let catalogue = load_catalogue(data)?;
    let rules = crate_rules(&catalogue, data)?;
    let cache_dir = resolve_cache_dir(cache_dir).map_err(DumpCommandError::from)?;
    let snapshot = read_cached(&fetch::dump_path(&cache_dir), WINDOW_DAYS)?;
    Ok(build_crates_report(&snapshot, &rules, percentile))
}

/// Renders the percentiles of the crate signals of `snapshot`.
///
/// The first line states the population and the dump date. A block for each
/// signal follows. The population is the crates that match `rules` and have at
/// least one active user. `percentile` sets the `at` line of each block.
pub fn build_crates_report(snapshot: &DumpSnapshot, rules: &Rules, percentile: f64) -> String {
    let found = discover::candidates(snapshot, rules);
    let targets: IdSet = found.candidates.iter().map(|c| c.crate_id).collect();
    let today = utc_date(snapshot.meta.timestamp);
    let signals = usage::compute(snapshot, &targets, today);
    let samples = crate_samples(snapshot, &found.candidates, &signals);
    let distributions = samples.distributions(&points_for(percentile));
    format!(
        "calibration: crates.io discovery candidates with at least one active user (current >= 1), dump {today}\n\n{}",
        render_distributions(&distributions, percentile)
    )
}
