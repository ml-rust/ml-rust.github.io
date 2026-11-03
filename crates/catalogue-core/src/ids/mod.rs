//! Validated identifier newtypes.

mod commit;
mod crate_name;
mod error;
mod login;
mod repo_name;
mod repo_url;
mod slug;

pub use commit::CommitSha;
pub use crate_name::{CrateName, canonical_crate_name, push_canonical_crate_name};
pub use error::IdError;
pub use login::GithubLogin;
pub use repo_name::is_github_repo_name;
pub use repo_url::normalize_repo_url;
pub use slug::Slug;
