//! The facts of one GitHub repo, read with a single GraphQL request.
//!
//! [`repo_facts`] sends the request and derives [`RepoFacts`] from the answer.
//! The derivation holds pure functions over the parsed response: bot commit
//! filtering in `commits`, maintainer response times in `threads`, and the
//! assembly in `derive`.

mod bot;
mod commits;
mod derive;
mod fetch;
mod query;
mod repo_ref;
mod response;
mod threads;
mod types;

pub use fetch::repo_facts;
pub use query::LATEST_SAMPLE;
pub use repo_ref::{RepoRef, RepoRefError};
pub use types::{CiState, Facts, RepoFacts};
