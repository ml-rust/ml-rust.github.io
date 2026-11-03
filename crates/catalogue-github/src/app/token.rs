//! Installation access tokens and the permissions they ask for.

use std::collections::BTreeMap;

use jiff::Timestamp;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::error::ApiError;
use crate::secret::Secret;

/// Seconds of life a token needs left to be reused. Below this the client mints a new one.
pub const REFRESH_MARGIN: i64 = 5 * 60;

/// The access level of one installation permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// `read`.
    Read,
    /// `write`.
    Write,
}

impl Access {
    /// The API name: `read` or `write`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

/// The permissions an installation token asks for, such as `issues: write`.
///
/// The token gets these and no others, even when the App holds more.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Permissions(BTreeMap<String, Access>);

impl Permissions {
    /// No permission.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `name` at `access`. A later call for the same name replaces it.
    #[must_use]
    pub fn with(mut self, name: &str, access: Access) -> Self {
        self.0.insert(name.to_owned(), access);
        self
    }

    /// The request body that scopes a token to `repository` and these permissions.
    pub(crate) fn token_body(&self, repository: &str) -> String {
        let permissions: Map<String, Value> = self
            .0
            .iter()
            .map(|(name, access)| (name.clone(), Value::from(access.as_str())))
            .collect();
        let mut body = Map::new();
        body.insert(
            "repositories".to_owned(),
            Value::Array(vec![Value::from(repository)]),
        );
        body.insert("permissions".to_owned(), Value::Object(permissions));
        Value::Object(body).to_string()
    }
}

/// An installation access token and when it expires.
///
/// `Debug` shows the expiry and `[redacted]` for the token. The token lives
/// in memory only.
#[derive(Debug)]
pub struct InstallationToken {
    secret: Secret,
    expires_at: Timestamp,
}

impl InstallationToken {
    /// When GitHub stops accepting the token.
    pub fn expires_at(&self) -> Timestamp {
        self.expires_at
    }

    /// Whether the token has at least [`REFRESH_MARGIN`] seconds left at `now`.
    pub fn is_fresh(&self, now: Timestamp) -> bool {
        self.expires_at.as_second() - now.as_second() >= REFRESH_MARGIN
    }

    /// The token, for an `Authorization` header.
    pub(crate) fn secret(&self) -> &Secret {
        &self.secret
    }
}

/// The token response fields the client reads. No `Debug`: it holds the token.
#[derive(Deserialize)]
struct Raw {
    token: Option<String>,
    expires_at: Option<String>,
}

/// Parses the body of a token response from `url`.
///
/// Errors name the field only, never a value, since the body holds the token.
pub(crate) fn parse_token(url: &str, body: &[u8]) -> Result<InstallationToken, ApiError> {
    let bad = |field| ApiError::TokenResponse {
        url: url.to_owned(),
        field,
    };
    let raw: Raw = serde_json::from_slice(body).map_err(|_| bad("token"))?;
    let token = raw
        .token
        .filter(|token| !token.is_empty())
        .ok_or_else(|| bad("token"))?;
    let expires_at = raw
        .expires_at
        .and_then(|text| text.parse::<Timestamp>().ok())
        .ok_or_else(|| bad("expires_at"))?;
    Ok(InstallationToken {
        secret: Secret::new(token),
        expires_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Timestamp {
        text.parse().expect("time")
    }

    #[test]
    fn the_body_scopes_one_repo_and_the_permissions() {
        let body = Permissions::new()
            .with("issues", Access::Write)
            .with("contents", Access::Read)
            .token_body("ml-rust.github.io");
        let value: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(
            value,
            serde_json::json!({
                "repositories": ["ml-rust.github.io"],
                "permissions": {"issues": "write", "contents": "read"},
            })
        );
    }

    #[test]
    fn a_token_response_parses_and_debug_hides_the_token() {
        let body = br#"{"token":"ghs_abc","expires_at":"2026-11-04T01:00:00Z","permissions":{}}"#;
        let token = parse_token("u", body).expect("parses");
        assert_eq!(token.expires_at(), at("2026-11-04T01:00:00Z"));
        assert_eq!(token.secret().expose(), "ghs_abc");
        let debug = format!("{token:?}");
        assert!(
            !debug.contains("ghs_abc") && debug.contains("[redacted]"),
            "{debug}"
        );
    }

    #[test]
    fn a_bad_token_response_names_the_field_and_not_the_value() {
        for (body, field) in [
            (
                br#"{"expires_at":"2026-11-04T01:00:00Z"}"#.as_slice(),
                "token",
            ),
            (br#"{"token":"ghs_abc"}"#.as_slice(), "expires_at"),
            (
                br#"{"token":"ghs_abc","expires_at":"soon"}"#.as_slice(),
                "expires_at",
            ),
            (br#"{"token":7}"#.as_slice(), "token"),
            (b"<html>".as_slice(), "token"),
        ] {
            let error = parse_token("u", body).expect_err("rejected");
            let message = error.to_string();
            assert!(message.contains(&format!("`{field}`")), "{message}");
            assert!(!message.contains("ghs_abc"), "{message}");
        }
    }

    #[test]
    fn a_token_is_fresh_until_five_minutes_before_expiry() {
        let token = parse_token("u", br#"{"token":"t","expires_at":"2026-11-04T01:00:00Z"}"#)
            .expect("parses");
        assert!(token.is_fresh(at("2026-11-04T00:00:00Z")));
        assert!(token.is_fresh(at("2026-11-04T00:55:00Z")));
        assert!(!token.is_fresh(at("2026-11-04T00:55:01Z")));
        assert!(!token.is_fresh(at("2026-11-04T02:00:00Z")));
    }
}
