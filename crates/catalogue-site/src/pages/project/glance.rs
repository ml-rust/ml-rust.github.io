//! The short list near the top of an entry page.

use catalogue_core::{
    ACTIVE_USERS_SIGNAL, CONTRIBUTORS_SIGNAL, DEPENDENTS_SIGNAL, Entry, EntryReport,
    LICENSE_SIGNAL, NATIVE_CODE_SIGNAL, PYTHON_SIGNAL, RUNS_ON_SIGNAL,
};

use crate::pages::shared::{InstallLine, TierBadge, fact_text, install_line, whole_text};

/// The data the "At a glance" list of `project.html` reads.
///
/// Every value but the tier and the Python statement reads as not measured when unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Glance {
    /// The tier badge. `None` without a report: no page shows a tier then.
    pub tier: Option<TierBadge>,
    /// The license.
    pub license: String,
    /// The backends.
    pub runs_on: String,
    /// The native code statement.
    pub native_code: String,
    /// The Python bindings statement. `None` when not measured.
    pub python: Option<String>,
    /// The active users, as a whole number.
    pub active_users: String,
    /// The dependents, as a whole number.
    pub dependents: String,
    /// The contributors, as a whole number.
    pub contributors: String,
    /// How to get the project.
    pub install: InstallLine,
}

impl Glance {
    /// The list for `entry`, with the facts of `record` when given.
    pub(crate) fn new(entry: &Entry, record: Option<&EntryReport>) -> Self {
        Self {
            tier: record.map(|record| TierBadge::of(record.tier)),
            license: fact_text(record, LICENSE_SIGNAL),
            runs_on: fact_text(record, RUNS_ON_SIGNAL),
            native_code: fact_text(record, NATIVE_CODE_SIGNAL),
            python: record
                .and_then(|record| record.value_of(PYTHON_SIGNAL))
                .map(str::to_owned),
            active_users: whole_text(record, ACTIVE_USERS_SIGNAL),
            dependents: whole_text(record, DEPENDENTS_SIGNAL),
            contributors: whole_text(record, CONTRIBUTORS_SIGNAL),
            install: install_line(entry),
        }
    }
}
