//! The facts a person reads to choose a project: license, backends, native code, Python
//! bindings and dependents.
//!
//! Every value comes from measured data. No tier rule reads any of them. A negative value states
//! only what its check found, and each check module lists what it cannot see.

mod dependents;
mod family;
mod license;
mod measure;
mod native;
mod native_walk;
mod reach;
mod runs_on;
mod search_terms;

pub use dependents::DependentIndex;
pub use license::{LicenseOrigin, Licensed};
pub use measure::{ChoiceFacts, ChoiceInputs, measure};
pub use runs_on::{BackendHits, Hit, RunsOn, Shows};
