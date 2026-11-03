//! Validated identifier newtypes.

mod crate_name;
mod error;
mod login;
mod slug;

pub use crate_name::CrateName;
pub use error::IdError;
pub use login::GithubLogin;
pub use slug::Slug;
