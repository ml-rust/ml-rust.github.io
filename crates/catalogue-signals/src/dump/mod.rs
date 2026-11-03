//! Streaming reader for the crates.io database dump, `db-dump.tar.gz`.
//!
//! [`read`] walks the archive once and keeps a [`DumpSnapshot`]. It reads
//! each table by header name and ignores extra columns and other tables.

mod error;
mod names;
mod parse;
mod read;
mod row;
mod snapshot;
mod table;
mod tables;

pub use error::DumpError;
pub use parse::utc_date;
pub use read::read;
pub use snapshot::{
    CrateRow, DailyDownloads, DeletedCrate, DependencyEdge, DependencyKind, DumpMeta, DumpSnapshot,
    DumpWarnings, IdMap, IdSet, Owner, OwnerKind, Requirement, VersionRow,
};
