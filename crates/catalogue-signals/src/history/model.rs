//! The rows of a snapshot.

use std::collections::BTreeMap;

use catalogue_core::{Date, EntryKind, Tier};
use serde::{Deserialize, Serialize};

use super::network::NetworkValues;

/// The file name suffix of a snapshot.
pub(super) const SUFFIX: &str = ".jsonl.gz";

/// What one check recorded for one entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// Library or app.
    pub kind: EntryKind,
    /// The tier the check published.
    pub tier: Tier,
    /// Whether the check found the entry dead.
    pub dead_now: bool,
    /// Whether the check found the entry dormant. `None` when unknown, and in
    /// snapshots written before the field existed.
    #[serde(default)]
    pub dormant: Option<bool>,
    /// Active external users. `None` when unknown.
    pub active_users: Option<f64>,
    /// Docker Hub pulls summed over the entry's images, fetched on the run date.
    /// `None` when unknown, and when the count in use fell back to an older
    /// value: a growth baseline needs a count from the snapshot's own date.
    pub docker_pulls: Option<u64>,
    /// The network-fetched values with their data dates, for a later check whose
    /// fetch fails. `None` when the check holds none, and in snapshots written
    /// before the field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkValues>,
}

/// Every entry's row for one run date.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// The run date.
    pub date: Date,
    /// The rows by entry slug.
    pub rows: BTreeMap<String, Row>,
}
