//! Facts about an issue that come from the GitHub API, never from the issue body.

use std::num::NonZeroU64;

use catalogue_core::{Date, GithubLogin, ISSUE_URL_PREFIX};
use url::Url;

use crate::form::FormError;

/// The directory of entry files, relative to the repository root.
pub const ENTRIES_DIR: &str = "data/entries";
/// The directory of judgment files, relative to the repository root.
pub const JUDGMENTS_DIR: &str = "data/judgments";

/// The issue number and author, as the GitHub API reports them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueMeta {
    /// The issue number.
    pub number: NonZeroU64,
    /// The login of the issue author.
    pub author: GithubLogin,
}

impl IssueMeta {
    /// The issue link: [`ISSUE_URL_PREFIX`] followed by the number.
    pub fn issue_url(&self) -> Result<Url, FormError> {
        let number = self.number.get();
        Url::parse(&format!("{ISSUE_URL_PREFIX}{number}"))
            .map_err(|_| FormError::IssueLink { number })
    }
}

/// The maintainer who approves an issue, and the approval date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approval {
    /// The approving maintainer.
    pub approver: GithubLogin,
    /// The approval date.
    pub date: Date,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_url_appends_the_number() {
        let meta = IssueMeta {
            number: NonZeroU64::new(12).expect("non-zero"),
            author: GithubLogin::new("dave").expect("login"),
        };
        assert_eq!(
            meta.issue_url().expect("link").as_str(),
            "https://github.com/ml-rust/ml-rust.github.io/issues/12"
        );
    }
}
