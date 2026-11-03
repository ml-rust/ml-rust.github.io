//! The machine-readable tier report: each entry's tier, reasons and signal provenance.

mod entry;
mod fact;
mod tier_report;

pub use entry::{EntryReport, ReasonRecord};
pub use fact::{SignalFact, SourceRef};
pub use tier_report::{REPORT_SCHEMA, ReportMismatch, TierReport};
