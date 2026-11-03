//! `data/deprecated.toml`: requirements on crates their own authors deprecated.

mod list;
mod requirement;

pub use list::{Deprecated, DeprecatedRequirement, RequirementCheck};
