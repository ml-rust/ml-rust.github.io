//! Daily snapshots of every entry's tier and the signals that need a past value.
//!
//! [`write`](fn@write) stores one [`Snapshot`] per run date as `<dir>/YYYY-MM-DD.jsonl.gz`.
//! [`read_all`] loads every snapshot of a directory into a [`History`]. A
//! `History` answers the questions the tier rules ask about earlier checks:
//! the previous tier, the run of dead months and the Docker pull baseline.

mod error;
mod model;
mod query;
mod read;
mod write;

pub use error::HistoryError;
pub use model::{Row, Snapshot};
pub use query::{GROWTH_MAX_AGE_DAYS, GROWTH_WINDOW_DAYS, History};
pub use read::read_all;
pub use write::write;
