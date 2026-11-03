//! The GitHub token that the commands read from the environment.

use thiserror::Error;

/// The environment variable that holds the GitHub token.
pub(crate) const TOKEN_VAR: &str = "GITHUB_TOKEN";

/// `GITHUB_TOKEN` is unset or empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error(
    "set GITHUB_TOKEN; prefix the command with `GITHUB_TOKEN=$(gh auth token)` to use the gh login locally"
)]
pub struct NoToken;

/// The token from the value of `GITHUB_TOKEN`, trimmed.
pub(crate) fn token_from(value: Option<String>) -> Result<String, NoToken> {
    value
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
        .ok_or(NoToken)
}

/// The token from the environment.
pub(crate) fn token_from_env() -> Result<String, NoToken> {
    token_from(std::env::var(TOKEN_VAR).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_token_is_missing() {
        for value in [None, Some(String::new()), Some("  \n".to_owned())] {
            assert_eq!(token_from(value), Err(NoToken));
        }
        assert_eq!(token_from(Some(" t\n".to_owned())), Ok("t".to_owned()));
    }

    #[test]
    fn the_message_names_the_variable_and_the_gh_login() {
        let message = NoToken.to_string();
        assert!(message.contains("set GITHUB_TOKEN"), "{message}");
        assert!(message.contains("gh auth token"), "{message}");
    }
}
