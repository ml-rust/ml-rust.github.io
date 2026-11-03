//! Issues, issue comments and issue labels.

use catalogue_signals::http::percent_encode;
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::{Map, Value};

use crate::api::{Verb, parse_json};
use crate::client::RepoClient;
use crate::error::ApiError;

/// Items per page of a listing, the most GitHub serves.
const PER_PAGE: u32 = 100;

/// The account that wrote an issue or a comment, as the API reports it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Author {
    /// The account login.
    pub login: String,
    /// The account type: `User`, `Bot` or `Organization`.
    #[serde(rename = "type")]
    pub kind: String,
}

impl Author {
    /// Whether this is the bot account of the App with `slug`: login
    /// `<slug>[bot]` and type `Bot`.
    pub fn is_app_bot(&self, slug: &str) -> bool {
        self.kind == "Bot"
            && self
                .login
                .strip_suffix("[bot]")
                .is_some_and(|name| name == slug)
    }
}

/// An open issue, as the listing reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// The issue number.
    pub number: u64,
    /// The issue title.
    pub title: String,
    /// The body exactly as the API returns it. A `null` body reads as empty.
    pub body: String,
    /// The author. `None` when the API sends no account.
    pub author: Option<Author>,
    /// The label names.
    pub labels: Vec<String>,
}

/// An issue comment.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Comment {
    /// The comment id.
    pub id: u64,
    /// The comment text. A `null` body reads as empty.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub body: String,
    /// The author. `None` when the API sends no account.
    #[serde(default, rename = "user")]
    pub author: Option<Author>,
    /// The API URL of the issue the comment sits on.
    #[serde(default)]
    pub issue_url: String,
}

impl Comment {
    /// Whether the comment sits on issue `number`, by the last part of its issue URL.
    pub fn is_on_issue(&self, number: u64) -> bool {
        self.issue_url
            .rsplit('/')
            .next()
            .is_some_and(|last| last == number.to_string())
    }
}

/// The result of a comment update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentUpdate {
    /// The comment holds the new text.
    Updated,
    /// The comment no longer exists. Post a new one.
    Missing,
}

/// The issue fields the listing reads.
#[derive(Deserialize)]
struct RawIssue {
    number: u64,
    #[serde(default, deserialize_with = "null_as_empty")]
    title: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    body: String,
    #[serde(default)]
    user: Option<Author>,
    #[serde(default)]
    labels: Vec<RawLabel>,
    /// Present on a pull request, which the issue listing also returns.
    #[serde(default)]
    pull_request: Option<IgnoredAny>,
}

/// A label name inside an issue.
#[derive(Deserialize)]
struct RawLabel {
    name: String,
}

/// The id of a created comment.
#[derive(Deserialize)]
struct Created {
    id: u64,
}

/// A string that the API can send as `null`.
fn null_as_empty<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Option::<String>::deserialize(deserializer).map(Option::unwrap_or_default)
}

/// The JSON body `{"<key>": value}`.
fn object(key: &str, value: Value) -> String {
    let mut body = Map::new();
    body.insert(key.to_owned(), value);
    Value::Object(body).to_string()
}

impl RepoClient<'_> {
    /// Every open issue, least recently updated first. Pull requests are left out.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when a call fails, a page is not the expected
    /// JSON, or the listing passes the page cap.
    pub fn open_issues(&mut self) -> Result<Vec<Issue>, ApiError> {
        let url = self.repo_url(&format!(
            "/issues?state=open&sort=updated&direction=asc&per_page={PER_PAGE}"
        ));
        let raw: Vec<RawIssue> = self.get_pages(&url)?;
        Ok(raw
            .into_iter()
            .filter(|issue| issue.pull_request.is_none())
            .map(|issue| Issue {
                number: issue.number,
                title: issue.title,
                body: issue.body,
                author: issue.user,
                labels: issue.labels.into_iter().map(|label| label.name).collect(),
            })
            .collect())
    }

    /// Every comment on issue `number`, oldest first.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when a call fails, a page is not the expected
    /// JSON, or the listing passes the page cap.
    pub fn issue_comments(&mut self, number: u64) -> Result<Vec<Comment>, ApiError> {
        let url = self.repo_url(&format!("/issues/{number}/comments?per_page={PER_PAGE}"));
        self.get_pages(&url)
    }

    /// The comment with `id`, or `None` when it no longer exists.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or the answer is not the
    /// expected JSON.
    pub fn comment(&mut self, id: u64) -> Result<Option<Comment>, ApiError> {
        let url = self.repo_url(&format!("/issues/comments/{id}"));
        let answer = self.call(Verb::Get, &url, None, &[200, 404])?;
        if answer.status == 404 {
            return Ok(None);
        }
        parse_json(&url, &answer.body).map(Some)
    }

    /// Posts `body` as a new comment on issue `number` and returns its id.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or GitHub answers with a
    /// status other than 201.
    pub fn create_comment(&mut self, number: u64, body: &str) -> Result<u64, ApiError> {
        let url = self.repo_url(&format!("/issues/{number}/comments"));
        let payload = object("body", Value::from(body));
        let answer = self.call(Verb::Post, &url, Some(&payload), &[201])?;
        let created: Created = parse_json(&url, &answer.body)?;
        Ok(created.id)
    }

    /// Writes `body` over comment `id`.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or GitHub answers with a
    /// status other than 200 or 404.
    pub fn update_comment(&mut self, id: u64, body: &str) -> Result<CommentUpdate, ApiError> {
        let url = self.repo_url(&format!("/issues/comments/{id}"));
        let payload = object("body", Value::from(body));
        let answer = self.call(Verb::Patch, &url, Some(&payload), &[200, 404])?;
        Ok(if answer.status == 404 {
            CommentUpdate::Missing
        } else {
            CommentUpdate::Updated
        })
    }

    /// Adds `labels` to issue `number`. Labels already on the issue stay.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or GitHub answers with a
    /// status other than 200.
    pub fn add_labels(&mut self, number: u64, labels: &[String]) -> Result<(), ApiError> {
        let url = self.repo_url(&format!("/issues/{number}/labels"));
        let names = labels
            .iter()
            .map(|name| Value::from(name.as_str()))
            .collect();
        let payload = object("labels", Value::Array(names));
        self.call(Verb::Post, &url, Some(&payload), &[200])?;
        Ok(())
    }

    /// Removes label `name` from issue `number`. A label the issue lacks is no error.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails or GitHub answers with a
    /// status other than 200 or 404.
    pub fn remove_label(&mut self, number: u64, name: &str) -> Result<(), ApiError> {
        let url = self.repo_url(&format!("/issues/{number}/labels/{}", percent_encode(name)));
        self.call(Verb::Delete, &url, None, &[200, 404])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn author(login: &str, kind: &str) -> Author {
        Author {
            login: login.to_owned(),
            kind: kind.to_owned(),
        }
    }

    #[test]
    fn only_the_app_bot_account_matches() {
        assert!(author("catalogue[bot]", "Bot").is_app_bot("catalogue"));
        assert!(!author("catalogue[bot]", "User").is_app_bot("catalogue"));
        assert!(!author("catalogue", "Bot").is_app_bot("catalogue"));
        assert!(!author("evil-catalogue[bot]", "Bot").is_app_bot("catalogue"));
        assert!(!author("catalogue[bot]", "Bot").is_app_bot("other"));
    }

    #[test]
    fn an_issue_with_null_fields_parses() {
        let raw: RawIssue = serde_json::from_str(
            r#"{"number":3,"title":"t","body":null,"user":null,"labels":[{"name":"submit","color":"x"}]}"#,
        )
        .expect("parses");
        assert_eq!(raw.body, "");
        assert!(raw.user.is_none() && raw.pull_request.is_none());
        assert_eq!(raw.labels.len(), 1);
        let pr: RawIssue =
            serde_json::from_str(r#"{"number":4,"pull_request":{"url":"u"}}"#).expect("parses");
        assert!(pr.pull_request.is_some());
    }

    #[test]
    fn a_comment_with_a_null_body_parses() {
        let comment: Comment = serde_json::from_str(
            r#"{"id":9,"body":null,"user":{"login":"a[bot]","type":"Bot","id":1}}"#,
        )
        .expect("parses");
        assert_eq!(comment.body, "");
        assert_eq!(comment.author, Some(author("a[bot]", "Bot")));
        assert!(!comment.is_on_issue(3));
        let on_three: Comment = serde_json::from_str(
            r#"{"id":9,"body":"x","issue_url":"https://api.github.com/repos/o/r/issues/3"}"#,
        )
        .expect("parses");
        assert!(on_three.is_on_issue(3) && !on_three.is_on_issue(33));
    }
}
