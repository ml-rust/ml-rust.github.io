//! Asks the providers in order until one model gives a valid answer.

use std::fmt;
use std::thread;
use std::time::Duration;

use catalogue_bot::triage::{AnswerError, ChatRequest, ChatResponse, Prompt};
use catalogue_signals::http::{Paced, PacedError, Pacer, Transport};

use crate::error::TriageError;
use crate::headers::json_bearer;
use crate::provider::{Provider, Retry};

/// The wait after a failed attempt, times the attempt number.
pub const BACKOFF_STEP: Duration = Duration::from_millis(500);

/// The sampling temperature each request asks for, unless the provider fields replace it.
pub const TEMPERATURE: f64 = 0.0;

/// The answer token cap each request asks for, unless the provider fields replace it.
pub const MAX_ANSWER_TOKENS: u32 = 1024;

/// Waits between two attempts at one model.
pub trait Backoff {
    /// Blocks for `wait`.
    fn pause(&mut self, wait: Duration);
}

/// A [`Backoff`] that sleeps.
#[derive(Debug, Clone, Copy, Default)]
pub struct SleepBackoff;

impl Backoff for SleepBackoff {
    fn pause(&mut self, wait: Duration) {
        thread::sleep(wait);
    }
}

/// Why one attempt at one model failed. No variant holds a key or a response body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// No response arrived: a connection error or a timeout.
    Transport,
    /// The provider answered with a status other than 200.
    Status(u16),
    /// The provider kept answering with a rate limit.
    RateLimited,
    /// The response body broke off.
    Body,
    /// The response is not a `chat/completions` answer.
    NotChat,
    /// The answer holds no content.
    NoContent,
    /// The content breaks the reply format.
    BadAnswer(AnswerError),
}

impl Failure {
    /// Whether the same model gets another attempt: timeouts, 408, 429, 5xx
    /// and bad answers. Any other status moves on to the next model.
    pub fn retryable(self) -> bool {
        match self {
            Self::Status(status) => status == 408 || status == 429 || status >= 500,
            Self::Transport
            | Self::RateLimited
            | Self::Body
            | Self::NotChat
            | Self::NoContent
            | Self::BadAnswer(_) => true,
        }
    }

    fn from_paced(error: &PacedError) -> Self {
        match error {
            PacedError::Http(_) => Self::Transport,
            PacedError::Status { status, .. } => Self::Status(*status),
            PacedError::Body { .. } => Self::Body,
            PacedError::RetriesExhausted { .. } => Self::RateLimited,
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport => f.write_str("no response"),
            Self::Status(status) => write!(f, "status {status}"),
            Self::RateLimited => f.write_str("rate limited"),
            Self::Body => f.write_str("the body broke off"),
            Self::NotChat => f.write_str("not a chat/completions answer"),
            Self::NoContent => f.write_str("no content"),
            Self::BadAnswer(error) => write!(f, "{error}"),
        }
    }
}

/// One failed attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptFailure {
    /// The provider name.
    pub provider: String,
    /// The model id.
    pub model: String,
    /// The attempt number at this model, from 1.
    pub attempt: u32,
    /// Why it failed.
    pub failure: Failure,
}

impl fmt::Display for AttemptFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} attempt {}: {}",
            self.provider, self.model, self.attempt, self.failure
        )
    }
}

/// A valid answer and the model that gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked<T> {
    /// The provider name.
    pub provider: String,
    /// The model id.
    pub model: String,
    /// The answer the validator returned.
    pub answer: T,
}

/// Sends `chat/completions` requests to the providers in order.
pub struct Llm<'a> {
    transport: &'a dyn Transport,
    paced: Paced<'a>,
    backoff: &'a mut dyn Backoff,
    providers: &'a [Provider],
    retry: Retry,
}

impl<'a> Llm<'a> {
    /// A client over `transport`. The `pacer` spaces every request. The
    /// `backoff` waits between attempts at one model.
    pub fn new(
        transport: &'a dyn Transport,
        pacer: &'a mut dyn Pacer,
        backoff: &'a mut dyn Backoff,
        providers: &'a [Provider],
        retry: Retry,
    ) -> Self {
        Self {
            transport,
            paced: Paced::new(pacer),
            backoff,
            providers,
            retry,
        }
    }

    /// The providers, in the order they are tried.
    pub fn providers(&self) -> &[Provider] {
        self.providers
    }

    /// Sends `prompt` until a model gives content that `validate` accepts.
    ///
    /// Providers go in order, and models in order within each. A model gets up
    /// to `max_attempts_per_model` attempts while its failures are
    /// [retryable](Failure::retryable). Before attempt `n + 1` the client waits
    /// [`BACKOFF_STEP`] times `n`.
    ///
    /// # Errors
    ///
    /// Returns [`TriageError::NoProvider`] when no provider is set, and
    /// [`TriageError::AllFailed`] with every attempt when no model gives a valid answer.
    pub fn ask<T>(
        &mut self,
        prompt: &Prompt,
        validate: impl Fn(&str) -> Result<T, AnswerError>,
    ) -> Result<Asked<T>, TriageError> {
        if self.providers.is_empty() {
            return Err(TriageError::NoProvider);
        }
        let providers = self.providers;
        let max = self.retry.max_attempts_per_model.get();
        let mut failures = Vec::new();
        for provider in providers {
            for model in provider.models() {
                for attempt in 1..=max {
                    let failure = match self.once(provider, model, prompt) {
                        Ok(content) => match validate(&content) {
                            Ok(answer) => {
                                return Ok(Asked {
                                    provider: provider.name().to_owned(),
                                    model: model.clone(),
                                    answer,
                                });
                            }
                            Err(error) => Failure::BadAnswer(error),
                        },
                        Err(failure) => failure,
                    };
                    failures.push(AttemptFailure {
                        provider: provider.name().to_owned(),
                        model: model.clone(),
                        attempt,
                        failure,
                    });
                    if !failure.retryable() {
                        break;
                    }
                    if attempt < max {
                        self.backoff.pause(BACKOFF_STEP.saturating_mul(attempt));
                    }
                }
            }
        }
        Err(TriageError::AllFailed { failures })
    }

    /// One request to `model` at `provider`. Returns the answer content.
    fn once(
        &mut self,
        provider: &Provider,
        model: &str,
        prompt: &Prompt,
    ) -> Result<String, Failure> {
        let mut headers: Vec<(&str, String)> = json_bearer(provider.api_key());
        headers.extend(
            provider
                .headers()
                .iter()
                .map(|(name, value)| (name.as_str(), value.clone())),
        );
        let body = ChatRequest {
            model,
            prompt,
            temperature: Some(TEMPERATURE),
            max_tokens: Some(MAX_ANSWER_TOKENS),
            extra: provider.extra(),
        }
        .body();
        let url = provider.endpoint();
        let transport = self.transport;
        let answer = self
            .paced
            .send(url, || transport.post(url, &headers, &body), &[200])
            .map_err(|error| Failure::from_paced(&error))?;
        let response = ChatResponse::parse(&answer.body).map_err(|_| Failure::NotChat)?;
        response
            .content()
            .map(str::to_owned)
            .ok_or(Failure::NoContent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_timeouts_rate_limits_server_errors_and_bad_answers_retry() {
        for status in [408, 429, 500, 503] {
            assert!(Failure::Status(status).retryable(), "{status}");
        }
        for status in [400, 401, 403, 404, 422] {
            assert!(!Failure::Status(status).retryable(), "{status}");
        }
        assert!(Failure::Transport.retryable());
        assert!(Failure::BadAnswer(AnswerError::Shape).retryable());
    }

    #[test]
    fn a_failure_names_the_provider_model_and_attempt_only() {
        let failure = AttemptFailure {
            provider: "ollama".to_owned(),
            model: "m".to_owned(),
            attempt: 2,
            failure: Failure::Status(503),
        };
        assert_eq!(failure.to_string(), "ollama m attempt 2: status 503");
    }
}
