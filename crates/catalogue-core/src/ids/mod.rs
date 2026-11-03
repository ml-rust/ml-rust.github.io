//! Validated identifier newtypes.

mod crate_name;
mod error;
mod login;
mod slug;

pub use crate_name::{CrateName, canonical_crate_name, push_canonical_crate_name};
pub use error::IdError;
pub use login::GithubLogin;
pub use slug::Slug;
