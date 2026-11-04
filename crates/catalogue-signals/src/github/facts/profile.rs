//! The profile of one GitHub repo: what a lead needs, read with a single GraphQL request.

use serde::Deserialize;
use serde_json::json;

use super::fetch::{post_query, repository_of};
use super::repo_ref::RepoRef;
use super::response::Connection;
use crate::github::error::GithubError;
use crate::http::{Pacer, Transport};

/// The profile query. GitHub follows a rename or a transfer, so `nameWithOwner` is canonical.
const QUERY: &str = "\
query($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    nameWithOwner
    isArchived
    isFork
    description
    homepageUrl
    repositoryTopics(first: 30) { nodes { topic { name } } }
    primaryLanguage { name }
    licenseInfo { spdxId }
  }
}";

/// The `repository` object of the answer.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Node {
    name_with_owner: String,
    #[serde(default)]
    is_archived: bool,
    #[serde(default)]
    is_fork: bool,
    description: Option<String>,
    homepage_url: Option<String>,
    repository_topics: Option<Connection<TopicNode>>,
    primary_language: Option<Named>,
    license_info: Option<License>,
}

/// One node of `repositoryTopics`.
#[derive(Debug, Deserialize)]
struct TopicNode {
    topic: Option<Named>,
}

/// An object the query reads the name of.
#[derive(Debug, Deserialize)]
struct Named {
    name: String,
}

/// The `licenseInfo` object.
#[derive(Debug, Deserialize)]
struct License {
    #[serde(rename = "spdxId")]
    spdx_id: Option<String>,
}

/// What a lead needs to know about an existing repo.
///
/// The description, the homepage and the topics are untrusted text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoProfile {
    /// The current `owner/name` as GitHub spells it.
    pub name_with_owner: String,
    /// Whether the repo is archived.
    pub archived: bool,
    /// Whether the repo is a fork.
    pub fork: bool,
    /// The repo description.
    pub description: Option<String>,
    /// The home page link.
    pub homepage: Option<String>,
    /// The repository topics the owner set.
    pub topics: Vec<String>,
    /// The primary language.
    pub language: Option<String>,
    /// The SPDX id of the license.
    pub license: Option<String>,
}

impl From<Node> for RepoProfile {
    fn from(node: Node) -> Self {
        let topics = node
            .repository_topics
            .iter()
            .flat_map(|topics| topics.items())
            .filter_map(|item| item.topic.as_ref())
            .map(|topic| topic.name.clone())
            .collect();
        Self {
            name_with_owner: node.name_with_owner,
            archived: node.is_archived,
            fork: node.is_fork,
            description: node.description,
            homepage: node.homepage_url,
            topics,
            language: node.primary_language.map(|language| language.name),
            license: node.license_info.and_then(|license| license.spdx_id),
        }
    }
}

/// Reads the profile of `repo` with one GraphQL request.
///
/// Returns `None` when GitHub holds no repo of that name. `token` is sent as a
/// bearer token. `pacer` holds the request back until the API allows it.
///
/// # Errors
///
/// Returns a [`GithubError`] when the request fails, the server answers with
/// an error status, the body is not the expected JSON, the rate limit does not
/// lift after the retries, or the API reports a GraphQL error other than a
/// missing repo.
pub fn repo_profile(
    transport: &dyn Transport,
    token: &str,
    repo: &RepoRef,
    pacer: &mut dyn Pacer,
) -> Result<Option<RepoProfile>, GithubError> {
    let envelope = post_query::<Node>(transport, token, &request_body(repo), pacer)?;
    Ok(repository_of(envelope, repo)?.map(RepoProfile::from))
}

/// The JSON body of the request. The repo travels as variables, never inside the query text.
fn request_body(repo: &RepoRef) -> Vec<u8> {
    json!({
        "query": QUERY,
        "variables": { "owner": repo.owner, "name": repo.name },
    })
    .to_string()
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::super::response::Envelope;
    use super::*;

    fn run(json: &str) -> Result<Option<RepoProfile>, GithubError> {
        let envelope: Envelope<Node> = serde_json::from_str(json).expect("envelope");
        let repo = RepoRef::parse("a/b").expect("parses");
        Ok(repository_of(envelope, &repo)?.map(RepoProfile::from))
    }

    #[test]
    fn the_body_carries_the_repo_as_variables() {
        let repo = RepoRef::parse("some-owner/some-repo").expect("parses");
        let value: Value = serde_json::from_slice(&request_body(&repo)).expect("json");
        assert_eq!(value["variables"]["owner"], "some-owner");
        assert_eq!(value["variables"]["name"], "some-repo");
        let query = value["query"].as_str().expect("query text");
        assert!(!query.contains("some-repo"));
    }

    #[test]
    fn every_field_is_read() {
        let found = run(r#"{"data":{"repository":{
                "nameWithOwner":"New/Name","isArchived":true,"isFork":true,
                "description":"A tool.","homepageUrl":"https://x.example",
                "repositoryTopics":{"nodes":[{"topic":{"name":"llm"}},null,{"topic":null}]},
                "primaryLanguage":{"name":"Rust"},"licenseInfo":{"spdxId":"MIT"}}}}"#)
        .expect("reads")
        .expect("found");
        assert_eq!(found.name_with_owner, "New/Name");
        assert!(found.archived && found.fork);
        assert_eq!(found.description.as_deref(), Some("A tool."));
        assert_eq!(found.homepage.as_deref(), Some("https://x.example"));
        assert_eq!(found.topics, ["llm"]);
        assert_eq!(found.language.as_deref(), Some("Rust"));
        assert_eq!(found.license.as_deref(), Some("MIT"));
    }

    #[test]
    fn null_fields_read_as_empty() {
        let found = run(
            r#"{"data":{"repository":{"nameWithOwner":"a/b","description":null,
                "homepageUrl":null,"repositoryTopics":null,"primaryLanguage":null,
                "licenseInfo":null}}}"#,
        )
        .expect("reads")
        .expect("found");
        assert!(!found.archived && !found.fork);
        assert_eq!(found.description, None);
        assert!(found.topics.is_empty());
        assert_eq!((found.language, found.license), (None, None));
    }

    #[test]
    fn a_missing_repository_is_none_and_other_errors_name_the_repo() {
        let gone = run(
            r#"{"data":{"repository":null},"errors":[{"type":"NOT_FOUND","path":["repository"],"message":"x"}]}"#,
        )
        .expect("reads");
        assert_eq!(gone, None);
        let error = run(r#"{"data":null,"errors":[{"message":"nope"}]}"#).expect_err("graphql");
        let text = error.to_string();
        assert!(text.contains("a/b") && text.contains("nope"), "{text}");
    }
}
