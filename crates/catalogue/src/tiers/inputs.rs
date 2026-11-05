//! The result, inputs and outputs of one `catalogue tiers` run.

use std::path::Path;

use catalogue_core::{Catalogue, Date, TierReport};
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::fallback::FetchSummary;
use catalogue_signals::rustsec::Advisories;
use catalogue_signals::sandbox::BuildChecks;

/// The outcome of one `catalogue tiers` check.
#[derive(Debug, Clone, PartialEq)]
pub struct TiersRun {
    /// The text report.
    pub text: String,
    /// The tier report, with the source and data date of each signal.
    pub report: TierReport,
    /// The network fetches of the run and the ones that failed. Empty offline.
    pub summary: FetchSummary,
}

/// What one run reads.
pub(super) struct Inputs<'a> {
    pub(super) catalogue: &'a Catalogue,
    pub(super) snapshot: &'a DumpSnapshot,
    pub(super) advisories: &'a Advisories,
    pub(super) build_checks: &'a BuildChecks,
    pub(super) today: Date,
}

/// Where one run writes.
pub(super) struct Outputs<'a> {
    /// The directory of daily snapshots.
    pub(super) history_dir: Option<&'a Path>,
    /// The file of the JSON tier report.
    pub(super) json: Option<&'a Path>,
}
