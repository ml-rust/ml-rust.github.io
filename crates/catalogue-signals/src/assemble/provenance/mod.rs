//! The provenance of each signal: raw value, source link and the date of the data behind it.

mod choice;
mod facts;
mod sources;

pub use choice::choice_signal_facts;
pub use facts::signal_facts;
pub use sources::{
    ADVISORY_PAGE, BUILD_CHECK_PAGE, DUMP_PAGE, DUMP_SOURCE, FactSources, FetchDates,
};
