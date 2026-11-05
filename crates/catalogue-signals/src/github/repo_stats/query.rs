//! The batched stats query, its request body and its parsed answer.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::github::{ApiError, NOT_FOUND, RepoRef};

/// The alias of the repo at `index` in a batch.
fn alias(index: usize) -> String {
    format!("r{index}")
}

/// What a lead review reads about a repo.
///
/// The description is untrusted text.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStats {
    /// The current `owner/name` as GitHub spells it. GitHub follows a rename or a transfer.
    pub name_with_owner: String,
    /// The stars.
    #[serde(default, rename = "stargazerCount")]
    pub stars: u32,
    /// The forks.
    #[serde(default, rename = "forkCount")]
    pub forks: u32,
    /// Whether the repo is a fork.
    #[serde(default)]
    pub is_fork: bool,
    /// Whether the repo is archived.
    #[serde(default)]
    pub is_archived: bool,
    /// The primary language.
    #[serde(default, deserialize_with = "language_name")]
    pub primary_language: Option<String>,
    /// The time of the last push, such as `2026-01-02T03:04:05Z`.
    #[serde(default)]
    pub pushed_at: Option<String>,
    /// The description.
    #[serde(default)]
    pub description: Option<String>,
}

/// Reads the `name` of a `primaryLanguage` object, or `None` for null.
fn language_name<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Named {
        name: String,
    }
    Ok(Option::<Named>::deserialize(deserializer)?.map(|language| language.name))
}

/// The fields read per repo, shared by every alias.
const FRAGMENT: &str = "\
fragment Stats on Repository {
  nameWithOwner
  stargazerCount
  forkCount
  isFork
  isArchived
  primaryLanguage { name }
  pushedAt
  description
}";

/// The JSON body that asks for `repos`, one alias per repo.
///
/// The owners and names travel as variables, never inside the query text.
pub(super) fn request_body(repos: &[RepoRef]) -> Vec<u8> {
    let mut declarations = Vec::with_capacity(repos.len() * 2);
    let mut selections = Vec::with_capacity(repos.len());
    let mut variables = Map::new();
    for (index, repo) in repos.iter().enumerate() {
        declarations.push(format!("$o{index}: String!, $n{index}: String!"));
        selections.push(format!(
            "{}: repository(owner: $o{index}, name: $n{index}) {{ ...Stats }}",
            alias(index)
        ));
        variables.insert(format!("o{index}"), Value::String(repo.owner.clone()));
        variables.insert(format!("n{index}"), Value::String(repo.name.clone()));
    }
    let query = format!(
        "query({}) {{\n  {}\n}}\n{FRAGMENT}",
        declarations.join(", "),
        selections.join("\n  ")
    );
    json!({ "query": query, "variables": variables })
        .to_string()
        .into_bytes()
}

/// The top level of an answer.
#[derive(Debug, Deserialize)]
pub(super) struct Answer {
    data: Option<HashMap<String, Option<RepoStats>>>,
    #[serde(default)]
    errors: Vec<ApiError>,
}

/// What an answer holds for a batch of repos.
#[derive(Debug)]
pub(super) struct Parsed {
    /// The stats of each repo of the batch, `None` for a repo GitHub does not resolve.
    pub(super) repos: Vec<Option<RepoStats>>,
}

impl Answer {
    /// The stats by position in the batch of `size` repos.
    ///
    /// # Errors
    ///
    /// Returns the API messages joined with `; ` when the answer holds an error other than a
    /// missing repo, and a fixed text when it holds neither data nor errors.
    pub(super) fn into_parsed(mut self, size: usize) -> Result<Parsed, String> {
        let other: Vec<&str> = self
            .errors
            .iter()
            .filter(|error| error.kind.as_deref() != Some(NOT_FOUND))
            .map(|error| error.message.as_str())
            .collect();
        if !other.is_empty() {
            return Err(other.join("; "));
        }
        if self.data.is_none() && self.errors.is_empty() {
            return Err("the answer holds no data".to_owned());
        }
        let mut data = self.data.take().unwrap_or_default();
        let repos = (0..size)
            .map(|index| data.remove(&alias(index)).flatten())
            .collect();
        Ok(Parsed { repos })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repos(names: &[&str]) -> Vec<RepoRef> {
        names
            .iter()
            .map(|name| RepoRef::parse(name).expect("repo"))
            .collect()
    }

    #[test]
    fn the_body_carries_each_repo_as_variables_and_one_alias_per_repo() {
        let body = request_body(&repos(&["Some-Owner/some-repo", "b/c"]));
        let value: Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(value["variables"]["o0"], "Some-Owner");
        assert_eq!(value["variables"]["n0"], "some-repo");
        assert_eq!(value["variables"]["o1"], "b");
        assert_eq!(value["variables"]["n1"], "c");
        let query = value["query"].as_str().expect("query text");
        assert!(!query.contains("some-repo") && !query.contains("Some-Owner"));
        assert!(query.contains("r0: repository(owner: $o0, name: $n0) { ...Stats }"));
        assert!(query.contains("r1: repository(owner: $o1, name: $n1) { ...Stats }"));
        assert!(query.contains("$o1: String!, $n1: String!"));
        assert!(!query.contains("r2"));
        for field in [
            "nameWithOwner",
            "stargazerCount",
            "forkCount",
            "isFork",
            "isArchived",
            "primaryLanguage { name }",
            "pushedAt",
            "description",
        ] {
            assert!(query.contains(field), "{field}");
        }
    }

    fn parse(json: &str, size: usize) -> Result<Parsed, String> {
        serde_json::from_str::<Answer>(json)
            .expect("answer")
            .into_parsed(size)
    }

    #[test]
    fn every_field_is_read_and_null_fields_read_as_empty() {
        let parsed = parse(
            r#"{"data":{
              "r0":{"nameWithOwner":"A/b","stargazerCount":150,"forkCount":7,"isFork":false,
                    "isArchived":true,"primaryLanguage":{"name":"Rust"},
                    "pushedAt":"2026-01-02T03:04:05Z","description":"A tool."},
              "r1":{"nameWithOwner":"c/d","primaryLanguage":null,"pushedAt":null,"description":null}
            }}"#,
            2,
        )
        .expect("parses");
        let first = parsed.repos[0].as_ref().expect("found");
        assert_eq!(first.name_with_owner, "A/b");
        assert_eq!((first.stars, first.forks), (150, 7));
        assert!(!first.is_fork && first.is_archived);
        assert_eq!(first.primary_language.as_deref(), Some("Rust"));
        assert_eq!(first.pushed_at.as_deref(), Some("2026-01-02T03:04:05Z"));
        assert_eq!(first.description.as_deref(), Some("A tool."));
        let second = parsed.repos[1].as_ref().expect("found");
        assert_eq!((second.stars, second.forks), (0, 0));
        assert_eq!(second.primary_language, None);
        assert_eq!(
            (second.pushed_at.as_deref(), second.description.as_deref()),
            (None, None)
        );
    }

    #[test]
    fn a_null_alias_and_a_missing_alias_are_unresolved() {
        let parsed = parse(
            r#"{"data":{"r0":null,"r2":{"nameWithOwner":"a/b"}},
                "errors":[{"type":"NOT_FOUND","path":["r0"],"message":"gone"}]}"#,
            3,
        )
        .expect("parses");
        assert!(parsed.repos[0].is_none());
        assert!(parsed.repos[1].is_none());
        assert!(parsed.repos[2].is_some());
    }

    #[test]
    fn other_errors_fail_the_batch_with_their_messages() {
        let error = parse(
            r#"{"data":{"r0":null},"errors":[{"type":"NOT_FOUND","path":["r0"],"message":"gone"},
                {"type":"RATE_LIMITED","message":"slow down"},{"message":"two"}]}"#,
            1,
        )
        .expect_err("error");
        assert_eq!(error, "slow down; two");
    }

    #[test]
    fn an_answer_with_no_data_and_no_errors_fails_the_batch() {
        let error = parse("{}", 1).expect_err("error");
        assert!(error.contains("holds no data"), "{error}");
    }
}
