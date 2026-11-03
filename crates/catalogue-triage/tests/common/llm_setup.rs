//! Providers, chat replies and a backoff that records its waits.

use std::time::Duration;

use catalogue_github::Secret;
use catalogue_github::testing::Reply;
use catalogue_triage::{Backoff, Provider};
use serde_json::{Map, Value, json};
use url::Url;

/// The key every test provider holds. No recorded request may show it.
pub const KEY: &str = "sk-test-key";

/// The chat endpoint of the first test provider.
pub const OLLAMA_CHAT: &str = "https://ollama.com/v1/chat/completions";

/// The chat endpoint of the second test provider.
pub const OPENROUTER_CHAT: &str = "https://openrouter.ai/api/v1/chat/completions";

/// A provider named `name` at `base` with `models`.
pub fn provider(name: &str, base: &str, models: &[&str]) -> Provider {
    Provider::new(
        name,
        &Url::parse(base).expect("base url"),
        models.iter().map(|model| (*model).to_owned()).collect(),
        Secret::new(KEY.to_owned()),
        vec![("X-Title".to_owned(), "ml-rust catalogue".to_owned())],
        Map::new(),
    )
    .expect("provider")
}

/// Ollama Cloud, then OpenRouter, two models each.
pub fn providers() -> Vec<Provider> {
    vec![
        provider("ollama", "https://ollama.com/v1", &["o-1", "o-2"]),
        provider(
            "openrouter",
            "https://openrouter.ai/api/v1",
            &["r-1", "r-2"],
        ),
    ]
}

/// A 200 chat answer with `content`.
pub fn chat(content: &str) -> Reply {
    Reply::new(
        200,
        json!({"choices": [{"message": {"role": "assistant", "content": content}}]}).to_string(),
    )
}

/// A 200 chat answer with the verdict `verdict` and `quote`.
pub fn verdict(verdict: &str, quote: &str) -> Reply {
    chat(&json!({"verdict": verdict, "quote": quote}).to_string())
}

/// The `model` field of a recorded request body.
pub fn model_of(body: &[u8]) -> String {
    let value: Value = serde_json::from_slice(body).expect("json body");
    value["model"].as_str().expect("model").to_owned()
}

/// A [`Backoff`] that records each wait and never sleeps.
#[derive(Debug, Default)]
pub struct RecordedBackoff {
    /// The waits asked for, in order.
    pub waits: Vec<Duration>,
}

impl Backoff for RecordedBackoff {
    fn pause(&mut self, wait: Duration) {
        self.waits.push(wait);
    }
}
