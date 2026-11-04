//! The inputs and the result of a lead run.

use std::collections::BTreeMap;

use catalogue_core::Slug;

use super::super::types::{Drafts, ExistingIndex, Mappings};
use crate::dump::DumpSnapshot;
use crate::http::Pacer;

/// What a lead run reads without a network.
#[derive(Debug, Clone, Copy)]
pub struct LeadInputs<'a> {
    /// The crates.io dump.
    pub snapshot: &'a DumpSnapshot,
    /// The topic and category mappings.
    pub mappings: Mappings<'a>,
    /// The entries that already exist.
    pub existing: &'a ExistingIndex,
    /// The active-user floor of the dump flow. It only marks a draft below it.
    pub min_active_users: f64,
}

/// One pacer per API a lead run calls.
pub struct LeadPacers<'a> {
    /// Holds back the GitHub requests.
    pub github: &'a mut dyn Pacer,
    /// Holds back the Homebrew requests.
    pub homebrew: &'a mut dyn Pacer,
    /// Holds back the Docker Hub requests.
    pub docker: &'a mut dyn Pacer,
    /// Holds back the Repology requests.
    pub repology: &'a mut dyn Pacer,
}

/// What a reviewer sees beside one draft.
#[derive(Debug, Clone, PartialEq)]
pub struct DraftDetail {
    /// The lead that made the draft, as the list writes it.
    pub lead: String,
    /// The most active users on one family crate. 0 without a crate.
    pub users: f64,
    /// Short `key` or `key:value` labels for the reviewer.
    pub notes: Vec<String>,
}

/// The result of one lead run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LeadDrafts {
    /// The drafted entries, sorted by slug, and the skips with their reasons, sorted by name.
    pub drafts: Drafts,
    /// Leads an existing entry already covers, as `(lead, where)`, sorted by lead.
    pub listed: Vec<(String, String)>,
    /// The detail of each draft.
    pub details: BTreeMap<Slug, DraftDetail>,
    /// Channel lookups that failed, each with an empty channel, and language list fetches that
    /// failed, each reading as no Rust.
    pub warnings: Vec<String>,
}
