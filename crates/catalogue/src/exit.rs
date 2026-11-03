//! Process exit codes shared by every command.

/// The data, the dump, or the request is invalid.
pub(crate) const INVALID: u8 = 1;
/// A file, directory, or network read failed.
pub(crate) const IO: u8 = 2;
