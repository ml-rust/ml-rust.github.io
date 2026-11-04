//! The manifest proof: a `Cargo.toml` on the repo's default branch declares the crate.

mod candidates;
mod declares;
mod proof;
mod site;

pub(super) use proof::{declaring_manifest, default_branch, fetch, root_files};
pub(super) use site::RepoSite;
