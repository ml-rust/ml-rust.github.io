//! GitHub signals: repository discovery and per-repo facts.
//!
//! [`search_repos`] runs one search per topic. It splits the `created` date
//! range until each query returns at most 1000 repos, which is the most the
//! search API serves. A [`Pacer`] keeps the requests inside the rate limit. The
//! HTTP layer sits behind [`Transport`](crate::http::Transport), so the search
//! runs without a network.
//!
//! [`repo_facts`] reads the facts of one repo with a single GraphQL request:
//! its state, its human commits and how fast its maintainers answer issues and
//! pull requests.

mod error;
mod facts;
mod headers;
mod json;
mod pacer;
mod query;
mod repo;
mod search;

pub use crate::http::Pacer;
pub use error::GithubError;
pub use facts::{CiState, Facts, LATEST_SAMPLE, RepoFacts, RepoRef, RepoRefError, repo_facts};
pub(crate) use headers::rest_headers;
pub(crate) use json::parse_json;
pub use pacer::GITHUB_INTERVAL;
pub use repo::{FoundRepo, GithubFound, GithubRules, Repo};
pub use search::{search_repos, search_repos_on};
