//! The advisory LLM link check of the ml-rust catalogue bot.
//!
//! [`triage`] reads each evidence link of an issue and asks a model whether
//! the page backs the claim. [`github_page`] reads GitHub pages through the
//! GitHub API. [`WebFetch`] reads every other page through Ollama
//! `web_fetch`, so the bot host never sends a request to an evidence host.
//! [`Llm`] asks the [`Provider`]s in order, each model in order, and retries
//! by the [`Retry`] rule. Every request goes through a
//! [`Transport`](catalogue_signals::http::Transport), so tests run without a network.
//!
//! The result becomes comment text only. No label, tier or approval reads it.
//! Keys never appear in `Debug` output or errors. Errors never hold a
//! response body.

mod client;
mod error;
mod fetch;
mod headers;
mod provider;
mod run;

pub use client::{
    Asked, AttemptFailure, BACKOFF_STEP, Backoff, Failure, Llm, MAX_ANSWER_TOKENS, MODEL_INTERVAL,
    SleepBackoff, TEMPERATURE,
};
pub use error::TriageError;
pub use fetch::{
    PageError, WEB_FETCH_INTERVAL, WEB_FETCH_URL, WebFetch, WebFetchError, check_endpoint,
    github_page,
};
pub use provider::{Provider, ProviderError, Retry};
pub use run::{Checkers, TriageResult, triage};
