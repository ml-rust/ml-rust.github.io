//! The types a GitHub search takes and returns.

use serde::{Deserialize, Deserializer};

/// The rules of one search.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GithubRules {
    /// Topics, each searched together with `language:Rust`.
    pub topics: Vec<String>,
    /// The fewest stars a repo has.
    pub min_stars: u32,
}

/// One repo from the search API. Unknown fields are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Repo {
    /// `owner/name`.
    pub full_name: String,
    /// The repo page.
    pub html_url: String,
    /// The description. It is untrusted text.
    pub description: Option<String>,
    /// The star count.
    pub stargazers_count: u32,
    /// When the last push happened, as an RFC 3339 timestamp. Empty when the repo has no push.
    #[serde(default, deserialize_with = "null_as_default")]
    pub pushed_at: String,
    /// When the repo was created, as an RFC 3339 timestamp.
    #[serde(default, deserialize_with = "null_as_default")]
    pub created_at: String,
    /// The topics the owner set.
    #[serde(default, deserialize_with = "null_as_default")]
    pub topics: Vec<String>,
    /// Whether the repo is archived.
    #[serde(default)]
    pub archived: bool,
    /// Whether the repo is a fork.
    #[serde(default)]
    pub fork: bool,
    /// The home page link.
    pub homepage: Option<String>,
}

/// A repo with the rule topics that found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundRepo {
    /// The repo.
    pub repo: Repo,
    /// The rule topics whose search returned the repo, in rule order.
    pub matched_topics: Vec<String>,
}

/// The result of one search.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GithubFound {
    /// The repos without repeats, sorted by lowercase `full_name`.
    pub repos: Vec<FoundRepo>,
    /// Searches that lost results.
    pub warnings: Vec<String>,
    /// The requests sent, retries included.
    pub requests: u32,
}

/// Reads JSON `null` as the default value.
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_fields_and_nulls_are_tolerated() {
        let repo: Repo = serde_json::from_str(
            r#"{"full_name":"a/b","html_url":"https://github.com/a/b","description":null,
                "stargazers_count":7,"pushed_at":null,"created_at":"2020-01-01T00:00:00Z",
                "topics":null,"homepage":null,"id":1,"owner":{"login":"a"}}"#,
        )
        .expect("parses");
        assert_eq!(repo.stargazers_count, 7);
        assert_eq!(repo.pushed_at, "");
        assert!(repo.topics.is_empty());
        assert!(!repo.archived && !repo.fork);
        assert_eq!(repo.description, None);
    }
}
