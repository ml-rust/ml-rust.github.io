//! The machine-readable tier report: each entry's tier, reasons and signal provenance.

mod entry;
mod fact;
mod tier_report;

pub use entry::{EntryReport, ReasonRecord};
pub use fact::{
    ACTIVE_USERS_SIGNAL, CONTRIBUTORS_SIGNAL, CPU_ONLY, DEPENDENTS_SIGNAL, LICENSE_SIGNAL,
    NATIVE_CODE_SIGNAL, NONE_STATED, NOT_MEASURED, PYTHON_SIGNAL, RUNS_ON_EVIDENCE_SIGNAL,
    RUNS_ON_SEPARATOR, RUNS_ON_SIGNAL, STARS_SIGNAL, SignalFact, SourceRef, never_counts,
};
pub use tier_report::{REPORT_SCHEMA, ReportMismatch, TierReport};
