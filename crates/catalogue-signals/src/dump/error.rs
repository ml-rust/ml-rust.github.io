//! Errors from reading a crates.io database dump.

use thiserror::Error;

/// A dump the reader rejects.
#[derive(Debug, Error)]
pub enum DumpError {
    /// The archive stream or one of its entries cannot be read.
    #[error(
        "cannot read `{entry}` in the dump archive: {source}. Fetch db-dump.tar.gz again and retry"
    )]
    Io {
        /// The entry path, or `archive` for the stream itself.
        entry: String,
        /// The read error.
        source: std::io::Error,
    },
    /// A table is not valid CSV or not valid UTF-8.
    #[error(
        "table `{table}` in the dump archive is not valid CSV: {source}. Fetch db-dump.tar.gz again and retry"
    )]
    Csv {
        /// The table name.
        table: &'static str,
        /// The CSV error, with its position.
        source: csv::Error,
    },
    /// `metadata.json` is absent, repeated, or lacks a field.
    #[error("metadata.json in the dump archive is invalid: {reason}")]
    Metadata {
        /// What is wrong, with the offending input.
        reason: String,
    },
    /// A table lacks a column the reader needs.
    #[error(
        "table `{table}` has no column `{column}`. The dump schema changed: update the `{table}` column list in catalogue-signals"
    )]
    MissingColumn {
        /// The table name.
        table: &'static str,
        /// The missing column.
        column: &'static str,
    },
    /// A field does not parse as its column type.
    #[error(
        "table `{table}`, record {record}, column `{column}`: expected {expected}, found `{value}`"
    )]
    BadValue {
        /// The table name.
        table: &'static str,
        /// The column name.
        column: &'static str,
        /// The 1-based CSV record number, not counting the header.
        record: u64,
        /// The field text.
        value: String,
        /// The accepted form.
        expected: &'static str,
    },
    /// An id does not fit in `u32`.
    #[error(
        "table `{table}`, record {record}, column `{column}`: id {value} does not fit in 32 bits. Widen the dump id type in catalogue-signals"
    )]
    IdOverflow {
        /// The table name.
        table: &'static str,
        /// The column name.
        column: &'static str,
        /// The 1-based CSV record number, not counting the header.
        record: u64,
        /// The id.
        value: u64,
    },
    /// A needed table is absent from the archive.
    #[error(
        "the dump archive holds no `data/{table}.csv`. Check that the input is a complete crates.io db-dump.tar.gz"
    )]
    MissingTable {
        /// The table name.
        table: &'static str,
    },
    /// A needed table appears twice in the archive.
    #[error(
        "the dump archive holds `data/{table}.csv` twice. Check that the input is a single crates.io db-dump.tar.gz"
    )]
    DuplicateTable {
        /// The table name.
        table: &'static str,
    },
    /// A table comes before an entry it joins against.
    #[error(
        "`{table}` comes before `{needs}` in the dump archive, and reading `{table}` needs `{needs}` first. The dump layout changed: update the reader in catalogue-signals"
    )]
    OrderViolation {
        /// The table that came too early.
        table: &'static str,
        /// The entry it needs first.
        needs: &'static str,
    },
}
