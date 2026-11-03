//! Repository discovery through the GitHub search API.
//!
//! [`search_repos`] runs one search per topic. It splits the `created` date
//! range until each query returns at most 1000 repos, which is the most the
//! search API serves. A [`Pacer`] keeps the requests inside the rate limit. The
//! HTTP layer sits behind [`Transport`](crate::http::Transport), so the search
//! runs without a network.

mod error;
mod pacer;
mod query;
mod repo;
mod search;

pub use error::GithubError;
pub use pacer::{Pacer, SleepPacer};
pub use repo::{FoundRepo, GithubFound, GithubRules, Repo};
pub use search::{search_repos, search_repos_on};
