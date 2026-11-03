//! Issues, issue comments and issue labels.

use catalogue_signals::http::percent_encode;
use jiff::Timestamp;
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::{Map, Value};

use crate::api::{PER_PAGE, Verb, parse_json};
use crate::client::RepoClient;
use crate::error::ApiError;

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

/// An issue, as the API reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// The issue number.
    pub number: u64,
    /// Whether the issue is open.
    pub open: bool,
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

/// The issue fields the client reads.
#[derive(Deserialize)]
struct RawIssue {
    number: u64,
    #[serde(default)]
    state: Option<String>,
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

/// A label name inside an issue or an event.
#[derive(Deserialize)]
struct RawLabel {
    name: String,
}

impl RawIssue {
    /// The issue, open when the API says so or `listed_open` holds.
    fn into_issue(self, listed_open: bool) -> Issue {
        Issue {
            number: self.number,
            open: listed_open || self.state.as_deref() == Some("open"),
            title: self.title,
            body: self.body,
            author: self.user,
            labels: self.labels.into_iter().map(|label| label.name).collect(),
        }
    }
}

/// One issue event, as the API reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueEvent {
    /// The event id.
    pub id: u64,
    /// The event name, such as `labeled`.
    pub event: String,
    /// The account that caused the event. `None` when the API sends none.
    pub actor: Option<Author>,
    /// The label name of a label event.
    pub label: Option<String>,
    /// When the event happened.
    pub created_at: Timestamp,
}

/// The event fields the client reads.
#[derive(Deserialize)]
struct RawEvent {
    id: u64,
    #[serde(default, deserialize_with = "null_as_empty")]
    event: String,
    #[serde(default)]
    actor: Option<Author>,
    #[serde(default)]
    label: Option<RawLabel>,
    created_at: String,
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
            .map(|issue| issue.into_issue(true))
            .collect())
    }

    /// Issue `number` as it is now. A pull request with that number is an error.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when the call fails, the answer is not the
    /// expected JSON, or the number names a pull request.
    pub fn issue(&mut self, number: u64) -> Result<Issue, ApiError> {
        let url = self.repo_url(&format!("/issues/{number}"));
        let answer = self.call(Verb::Get, &url, None, &[200])?;
        let raw: RawIssue = parse_json(&url, &answer.body)?;
        if raw.pull_request.is_some() || raw.number != number {
            return Err(ApiError::Content {
                url,
                reason: format!("the answer is not issue {number}. Report it"),
            });
        }
        Ok(raw.into_issue(false))
    }

    /// Every event of issue `number`, in API order.
    ///
    /// # Errors
    ///
    /// Returns an [`ApiError`] when a call fails, a page is not the expected
    /// JSON, an event time is not a timestamp, or the listing passes the page cap.
    pub fn issue_events(&mut self, number: u64) -> Result<Vec<IssueEvent>, ApiError> {
        let url = self.repo_url(&format!("/issues/{number}/events?per_page={PER_PAGE}"));
        let raw: Vec<RawEvent> = self.get_pages(&url)?;
        raw.into_iter()
            .map(|event| {
                let created_at = event.created_at.parse::<Timestamp>().map_err(|_| {
                    ApiError::Content {
                        url: url.clone(),
                        reason: format!(
                            "event {} has no valid `created_at`. Rerun later. If the error persists, report it",
                            event.id
                        ),
                    }
                })?;
                Ok(IssueEvent {
                    id: event.id,
                    event: event.event,
                    actor: event.actor,
                    label: event.label.map(|label| label.name),
                    created_at,
                })
            })
            .collect()
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
        let closed: RawIssue =
            serde_json::from_str(r#"{"number":5,"state":"closed"}"#).expect("parses");
        assert!(!closed.into_issue(false).open);
    }

    #[test]
    fn an_event_with_a_null_actor_and_no_label_parses() {
        let event: RawEvent = serde_json::from_str(
            r#"{"id":1,"event":"labeled","actor":null,"label":{"name":"approved","color":"x"},"created_at":"2026-11-03T10:00:00Z"}"#,
        )
        .expect("parses");
        assert!(event.actor.is_none());
        assert_eq!(
            event.label.map(|label| label.name).as_deref(),
            Some("approved")
        );
        let closed: RawEvent = serde_json::from_str(
            r#"{"id":2,"event":"closed","actor":{"login":"a","type":"User"},"created_at":"2026-11-03T10:00:00Z"}"#,
        )
        .expect("parses");
        assert!(closed.label.is_none());
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
