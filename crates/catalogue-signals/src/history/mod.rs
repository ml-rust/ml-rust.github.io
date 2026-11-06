//! Snapshots of every entry's tier and the signals that need a past value.
//!
//! [`write`](fn@write) stores one [`Snapshot`] per run date as `<dir>/YYYY-MM-DD.jsonl.gz`.
//! [`ReadmeStore`] keeps the newest README of each entry in `<dir>/readmes.jsonl.gz`.
//! [`read_all`] loads every snapshot of a directory into a [`History`].
//! [`read_snapshot`] reads one snapshot from its bytes, such as a fetched file. A
//! `History` answers the questions the tier rules ask about earlier checks:
//! the previous tier, the run of dead months and the Docker pull baseline.
//! [`History::tier_changes`] lists every [`TierChange`] the snapshots record.
//! [`History::movement`] measures new entries and changes in active users.
//! [`NetworkValues`] holds what a row keeps of each network fetch, for the next
//! check to fall back on or to reuse.

mod changes;
mod error;
mod model;
mod movement;
mod network;
mod query;
mod read;
mod readmes;
mod write;

pub use changes::TierChange;
pub use error::HistoryError;
pub use model::{Row, Snapshot};
pub use network::{Dated, NetworkValues, ReleaseValues, RepoValues};
pub use query::{GROWTH_MAX_AGE_DAYS, GROWTH_WINDOW_DAYS, History};
pub use read::{read_all, read_snapshot, snapshot_date};
pub use readmes::{READMES_FILE, ReadmeRecord, ReadmeStore};
pub use write::write;
