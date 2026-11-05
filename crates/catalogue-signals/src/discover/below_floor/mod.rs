//! The below-floor review list: repos of crates that match the vocabulary and have too few
//! active users to enter on dependents.
//!
//! [`group_repos`] groups the crates by GitHub repo, [`drop_known`] removes the repos the
//! catalogue already knows, and [`select_leads`] keeps the active Rust repos with enough stars
//! and sorts them. Every function is pure. The list is for a person to review. No tier reads it.

mod group;
mod select;

pub use group::{Group, Grouped, drop_known, group_repos};
pub use select::{DESCRIPTION_CHARS, Lead, Selected, TERMS_SHOWN, select_leads};
