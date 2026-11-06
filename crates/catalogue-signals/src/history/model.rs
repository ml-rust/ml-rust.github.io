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
    /// Whether the run reused stored values for this entry and sent it no request. Such a row is
    /// no check: it never advances the run of dead months. False in snapshots written before the
    /// field existed, because every one of those runs fetched.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reused: bool,
    /// The notes of the entry's fetches, for a later run that reuses its values and asks nothing.
    /// Empty in snapshots written before the field existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fetch_notes: Vec<String>,
}

/// Every entry's row for one run date.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// The run date.
    pub date: Date,
    /// The rows by entry slug.
    pub rows: BTreeMap<String, Row>,
}
