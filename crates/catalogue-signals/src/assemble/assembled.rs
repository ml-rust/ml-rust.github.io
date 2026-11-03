//! The result of assembling one entry's signals.

use catalogue_core::Signals;

/// One entry's signals and the notes about what stayed unresolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Assembled {
    /// The signals for the tier rules.
    pub signals: Signals,
    /// One line per input the assembly leaves unresolved, such as a listed
    /// crate missing from the dump.
    pub notes: Vec<String>,
}
