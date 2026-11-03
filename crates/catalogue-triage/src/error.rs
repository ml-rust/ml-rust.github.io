//! Errors of the link check that stop it as a whole.

use thiserror::Error;

use crate::client::AttemptFailure;

/// A model request that ends without a valid answer.
///
/// No variant holds a key, a prompt or a response body.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TriageError {
    /// The provider list is empty.
    #[error(
        "no model provider is set. Add a provider with an API key to the bot config, then rerun"
    )]
    NoProvider,
    /// Every attempt at every model failed.
    #[error(
        "every model provider failed: {}. Check the provider keys and model ids in the bot config, then rerun later",
        attempts(.failures)
    )]
    AllFailed {
        /// Every failed attempt, in order.
        failures: Vec<AttemptFailure>,
    },
}

/// The failures joined with `; `.
fn attempts(failures: &[AttemptFailure]) -> String {
    let parts: Vec<String> = failures.iter().map(ToString::to_string).collect();
    parts.join("; ")
}
