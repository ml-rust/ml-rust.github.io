//! The sandbox build check: `cargo check` on the latest release of dormant crates.
//!
//! The check runs third-party build scripts, so it runs only in a CI job with no
//! secrets. [`CheckItem`] is one crate the check builds. [`classify()`] turns the
//! JSON messages of one `cargo check` into a [`CheckResult`] with a fixed detail.
//! [`CheckRecord`] is one result line. [`BuildChecks`] holds the results that a
//! tier run reads for rot signal R6.

mod checks;
mod classify;
mod error;
mod lines;
mod message;
mod record;

pub use checks::BuildChecks;
pub use classify::{Classified, Exit, classify};
pub use error::SandboxError;
pub use lines::read_lines;
pub use record::{CheckItem, CheckRecord, CheckResult};
