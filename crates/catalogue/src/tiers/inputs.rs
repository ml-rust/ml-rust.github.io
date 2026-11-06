//! The result, inputs and outputs of one `catalogue tiers` run.

use std::path::Path;

use catalogue_core::{Catalogue, Date, TierReport};
use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::fallback::FetchSummary;
use catalogue_signals::rustsec::Advisories;
use catalogue_signals::sandbox::BuildChecks;

use super::cadence::FetchMode;

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
pub struct Inputs<'a> {
    /// The checked catalogue.
    pub catalogue: &'a Catalogue,
    /// The crates.io dump.
    pub snapshot: &'a DumpSnapshot,
    /// The RustSec archive.
    pub advisories: &'a Advisories,
    /// The sandbox results that rot signal R6 reads.
    pub build_checks: &'a BuildChecks,
    /// What the run may fetch.
    pub fetch: FetchMode,
    /// The run date.
    pub today: Date,
}

/// Where one run writes.
pub(super) struct Outputs<'a> {
    /// The directory of snapshots.
    pub(super) history_dir: Option<&'a Path>,
    /// The file of the JSON tier report.
    pub(super) json: Option<&'a Path>,
}
