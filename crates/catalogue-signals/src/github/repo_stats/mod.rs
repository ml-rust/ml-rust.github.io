//! Stars, forks and state of many GitHub repos, read in batched GraphQL requests.
//!
//! [`fetch_repo_stats`] sends up to [`BATCH_SIZE`] repos per request. A repo that GitHub does not
//! resolve is returned as unresolved. A request that fails leaves its repos unfetched and the run
//! goes on with the next batch.

mod fetch;
mod query;

pub use fetch::{BATCH_SIZE, FailedBatch, StatsFetch, fetch_repo_stats};
pub use query::RepoStats;
