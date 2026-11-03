//! The body of an OpenAI-compatible `chat/completions` request and the content of its answer.

use serde::Deserialize;
use serde_json::{Map, Number, Value};

use super::prompt::Prompt;

/// One `chat/completions` request.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatRequest<'a> {
    /// The model id, as the provider names it.
    pub model: &'a str,
    /// The system and user messages.
    pub prompt: &'a Prompt,
    /// The default sampling temperature. `extra` can replace it.
    pub temperature: Option<f64>,
    /// The default cap on answer tokens. `extra` can replace it.
    pub max_tokens: Option<u32>,
    /// Provider fields merged at the top level, such as `reasoning_effort`.
    pub extra: &'a Map<String, Value>,
}

impl ChatRequest<'_> {
    /// The JSON body.
    ///
    /// `temperature` and `max_tokens` go in first, then `extra`, then the
    /// fields the bot owns: `model`, `messages`, `response_format` with type
    /// `json_object`, and `stream` set to false. An `extra` key never replaces an owned field.
    pub fn body(&self) -> Vec<u8> {
        let mut body = Map::new();
        if let Some(number) = self.temperature.and_then(Number::from_f64) {
            body.insert("temperature".to_owned(), Value::Number(number));
        }
        if let Some(tokens) = self.max_tokens {
            body.insert("max_tokens".to_owned(), Value::from(tokens));
        }
        for (key, value) in self.extra {
            body.insert(key.clone(), value.clone());
        }
        body.insert("model".to_owned(), Value::from(self.model));
        body.insert(
            "messages".to_owned(),
            Value::Array(vec![
                message("system", self.prompt.system),
                message("user", &self.prompt.user),
            ]),
        );
        let mut format = Map::new();
        format.insert("type".to_owned(), Value::from("json_object"));
        body.insert("response_format".to_owned(), Value::Object(format));
        body.insert("stream".to_owned(), Value::Bool(false));
        Value::Object(body).to_string().into_bytes()
    }
}

fn message(role: &str, content: &str) -> Value {
    let mut out = Map::new();
    out.insert("role".to_owned(), Value::from(role));
    out.insert("content".to_owned(), Value::from(content));
    Value::Object(out)
}

/// The answer fields the bot reads. Other fields are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChatResponse {
    /// The choices. The bot reads the first.
    #[serde(default)]
    pub choices: Vec<ChatChoice>,
}

/// One choice of an answer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChatChoice {
    /// The answer message.
    pub message: ChatMessage,
}

/// The message of one choice.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChatMessage {
    /// The text. `None` when the provider sends `null` or leaves it out.
    #[serde(default)]
    pub content: Option<String>,
}

impl ChatResponse {
    /// Parses an answer body.
    ///
    /// # Errors
    ///
    /// Returns the JSON error when `bytes` is not an answer object.
    pub fn parse(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// The content of the first choice, or `None` when it is absent or blank.
    pub fn content(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|choice| choice.message.content.as_deref())
            .filter(|content| !content.trim().is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt() -> Prompt {
        Prompt {
            system: "sys",
            user: "usr".to_owned(),
        }
    }

    #[test]
    fn the_body_holds_owned_fields_over_extra_ones() {
        let prompt = prompt();
        let mut extra = Map::new();
        extra.insert("reasoning_effort".to_owned(), Value::from("low"));
        extra.insert("temperature".to_owned(), Value::from(0.7));
        extra.insert("model".to_owned(), Value::from("evil"));
        extra.insert("stream".to_owned(), Value::Bool(true));
        let request = ChatRequest {
            model: "m1",
            prompt: &prompt,
            temperature: Some(0.0),
            max_tokens: Some(512),
            extra: &extra,
        };
        let body: Value = serde_json::from_slice(&request.body()).expect("json");
        assert_eq!(body["model"], "m1");
        assert_eq!(body["temperature"], 0.7);
        assert_eq!(body["max_tokens"], 512);
        assert_eq!(body["reasoning_effort"], "low");
        assert_eq!(body["stream"], false);
        assert_eq!(body["response_format"]["type"], "json_object");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "sys");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "usr");
    }

    #[test]
    fn the_content_comes_from_the_first_choice() {
        let answer = ChatResponse::parse(
            br#"{"id":"x","choices":[{"index":0,"message":{"role":"assistant","content":" {} "}},{"message":{"content":"no"}}],"usage":{}}"#,
        )
        .expect("parses");
        assert_eq!(answer.content(), Some(" {} "));
        for body in [
            &br#"{"choices":[]}"#[..],
            br#"{"choices":[{"message":{"content":null}}]}"#,
            br#"{"choices":[{"message":{"content":"  "}}]}"#,
            br#"{}"#,
        ] {
            assert_eq!(ChatResponse::parse(body).expect("parses").content(), None);
        }
        assert!(ChatResponse::parse(b"<html>").is_err());
    }
}
