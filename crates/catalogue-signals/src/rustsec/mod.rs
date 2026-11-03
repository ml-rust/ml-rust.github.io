//! The RustSec advisory database.
//!
//! [`read`] parses the `advisory-db` archive into [`Advisories`], keyed by
//! canonical crate name. Advisories for the Rust toolchain are ignored.

mod advisories;
mod advisory;
mod block;
mod error;
mod read;

pub use advisories::{Advisories, UNPATCHED_GRACE_DAYS};
pub use advisory::{Advisory, AdvisoryKind};
pub use error::RustsecError;
pub use read::read;
