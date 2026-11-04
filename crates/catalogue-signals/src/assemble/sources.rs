//! The inputs every entry with crates shares on one check.

use catalogue_core::{Deprecated, Thresholds};

use super::index::CrateIndex;
use crate::dump::{DumpSnapshot, IdMap};
use crate::rustsec::Advisories;
use crate::sandbox::BuildChecks;
use crate::usage::Usage;

/// The inputs every entry with crates shares on one check. An app reads only `dump` and `index`.
#[derive(Debug, Clone, Copy)]
pub struct LibrarySources<'a> {
    /// The crates.io dump.
    pub dump: &'a DumpSnapshot,
    /// The name and version index of `dump`.
    pub index: &'a CrateIndex,
    /// Adoption signals by crate id, from one [`usage::compute`](crate::usage::compute) call.
    pub usage: &'a IdMap<Usage>,
    /// The RustSec advisories.
    pub advisories: &'a Advisories,
    /// The deprecated requirements list.
    pub deprecated: &'a Deprecated,
    /// The published thresholds. Supplies the advisory grace window.
    pub thresholds: &'a Thresholds,
    /// The sandbox build check results. Empty when the run reads none.
    pub build_checks: &'a BuildChecks,
}
