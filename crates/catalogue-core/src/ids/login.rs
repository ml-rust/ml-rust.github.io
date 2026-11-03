use std::fmt;

use serde::{Deserialize, Serialize};

use super::error::IdError;

const MAX_LEN: usize = 39;

/// A GitHub login: 1 to 39 characters of ASCII letters, digits and `-`.
///
/// The login has no leading or trailing `-` and no `--`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GithubLogin(String);

impl GithubLogin {
    /// Checks `value` and wraps it.
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        check(&value)?;
        Ok(Self(value))
    }

    /// The login text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The lowercase form, which GitHub treats as the account identity.
    pub fn normalized(&self) -> String {
        self.0.to_ascii_lowercase()
    }

    /// Whether both logins name the same GitHub account.
    pub fn same_account(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

fn check(value: &str) -> Result<(), IdError> {
    let err = |rule: &'static str| IdError::new("GitHub login", value, rule);
    if value.is_empty() || value.len() > MAX_LEN {
        return Err(err("length must be 1 to 39 characters"));
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(err("only ASCII letters, digits and `-` are allowed"));
    }
    if value.starts_with('-') || value.ends_with('-') || value.contains("--") {
        return Err(err("`-` must sit between two characters and never repeat"));
    }
    Ok(())
}

impl TryFrom<String> for GithubLogin {
    type Error = IdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<GithubLogin> for String {
    fn from(login: GithubLogin) -> Self {
        login.0
    }
}

impl fmt::Display for GithubLogin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_logins() {
        let long = "a".repeat(39);
        for ok in ["a", "Octo-Cat", "user123", "a-b-c", long.as_str()] {
            assert!(GithubLogin::new(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn rejects_invalid_logins() {
        let too_long = "a".repeat(40);
        for bad in ["", "-a", "a-", "a--b", "a_b", "a b", "é", too_long.as_str()] {
            assert!(GithubLogin::new(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn same_account_ignores_case() {
        let a = GithubLogin::new("Bob").expect("valid");
        let b = GithubLogin::new("bob").expect("valid");
        let c = GithubLogin::new("carol").expect("valid");
        assert!(a.same_account(&b));
        assert!(!a.same_account(&c));
        assert_eq!(a.normalized(), "bob");
    }
}
