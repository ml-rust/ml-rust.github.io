//! The GraphQL query and its request body.

use jiff::Span;
use jiff::civil::Date;
use serde_json::json;

use super::repo_ref::RepoRef;

/// The GraphQL endpoint.
pub(crate) const ENDPOINT: &str = "https://api.github.com/graphql";

/// Commits read from the default branch history.
pub(super) const COMMIT_PAGE: u64 = 100;

/// Commits read from the head of the default branch with no date bound.
///
/// The sample finds the newest human commit when it is older than the window.
pub const LATEST_SAMPLE: u64 = 30;

/// The days of history that the commit window and the response window cover.
pub(super) const WINDOW_DAYS: i32 = 365;

/// The first day of the 365-day window that ends on `today`.
pub(super) fn window_start(today: Date) -> Date {
    today.saturating_sub(Span::new().days(WINDOW_DAYS))
}

/// The repo query. `$since` bounds the `recent` commit history. The `latest`
/// history has no bound and reads [`LATEST_SAMPLE`] commits. Its `totalCount` is the total
/// commits on the default branch, whatever the page size, so the total costs no second history.
const QUERY: &str = "\
query($owner: String!, $name: String!, $since: GitTimestamp!) {
  repository(owner: $owner, name: $name) {
    nameWithOwner
    isArchived
    isFork
    pushedAt
    stargazerCount
    forkCount
    licenseInfo { spdxId }
    repositoryTopics(first: 20) { nodes { topic { name } } }
    defaultBranchRef {
      name
      target {
        ... on Commit {
          statusCheckRollup { state }
          recent: history(first: 100, since: $since) {
            totalCount
            nodes { committedDate author { user { login } name } }
          }
          latest: history(first: 30) {
            totalCount
            nodes { committedDate author { user { login } name } }
          }
        }
      }
    }
    issues(first: 50, orderBy: {field: CREATED_AT, direction: DESC}) {
      nodes {
        createdAt state authorAssociation author { login }
        comments(first: 30) { nodes { createdAt authorAssociation author { login } } }
      }
    }
    pullRequests(first: 50, orderBy: {field: CREATED_AT, direction: DESC}) {
      nodes {
        createdAt state authorAssociation author { login }
        comments(first: 30) { nodes { createdAt authorAssociation author { login } } }
      }
    }
  }
  rateLimit { cost remaining resetAt }
}";

/// The JSON body of the request for `repo`, with commits since `since`.
///
/// The repo and the date travel as variables, never inside the query text.
pub(super) fn request_body(repo: &RepoRef, since: Date) -> Vec<u8> {
    json!({
        "query": QUERY,
        "variables": {
            "owner": repo.owner,
            "name": repo.name,
            "since": format!("{since}T00:00:00Z"),
        },
    })
    .to_string()
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use serde_json::Value;

    use super::*;

    #[test]
    fn the_body_carries_the_repo_and_the_date_as_variables() {
        let repo = RepoRef::parse("rust-lang/rustlings").expect("parses");
        let body = request_body(&repo, date(2025, 11, 3));
        let value: Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(value["variables"]["owner"], "rust-lang");
        assert_eq!(value["variables"]["name"], "rustlings");
        assert_eq!(value["variables"]["since"], "2025-11-03T00:00:00Z");
        let query = value["query"].as_str().expect("query text");
        assert!(!query.contains("rustlings"));
        assert!(query.contains("recent: history(first: 100, since: $since)"));
        assert!(query.contains(&format!("latest: history(first: {LATEST_SAMPLE})")));
        assert!(query.contains("forkCount"));
        assert!(query.contains("repositoryTopics(first: 20)"));
        let latest = query.split("latest: history").nth(1).expect("latest");
        assert!(latest.contains("totalCount"), "{latest}");
    }
}
