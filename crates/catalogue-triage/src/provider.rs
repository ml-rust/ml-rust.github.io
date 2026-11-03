//! One OpenAI-compatible model provider and the retry rule.

use std::num::NonZeroU32;

use catalogue_github::Secret;
use serde_json::{Map, Value};
use thiserror::Error;
use url::Url;

/// Headers the bot sets on every model request. A provider never sets them.
const RESERVED_HEADERS: [&str; 8] = [
    "accept",
    "authorization",
    "connection",
    "content-length",
    "content-type",
    "host",
    "transfer-encoding",
    "user-agent",
];

/// Request fields the bot owns. A provider never sets them.
const RESERVED_FIELDS: [&str; 4] = ["messages", "model", "response_format", "stream"];

/// A provider that cannot be used as given.
///
/// No variant holds the key or a header value.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderError {
    /// The name is empty or holds characters outside ASCII letters, digits, `-` and `_`.
    #[error(
        "provider name `{name}` is empty or holds characters other than ASCII letters, digits, `-` and `_`. Rename the provider"
    )]
    Name {
        /// The rejected name.
        name: String,
    },
    /// The base URL is not plain https.
    #[error(
        "provider `{name}` has base URL `{url}`, which is not https or holds a user part, query or fragment. Set an https base URL such as https://ollama.com/v1"
    )]
    BaseUrl {
        /// The provider name.
        name: String,
        /// The rejected URL.
        url: String,
    },
    /// The key is empty.
    #[error("provider `{name}` has an empty API key. Set the key, or remove the provider")]
    EmptyKey {
        /// The provider name.
        name: String,
    },
    /// The model list is empty or holds an empty id.
    #[error("provider `{name}` lists no model, or an empty model id. List at least one model id")]
    Models {
        /// The provider name.
        name: String,
    },
    /// A header name is not an HTTP token.
    #[error(
        "provider `{name}` sets header `{header}`, which is not a valid HTTP header name. Fix the name"
    )]
    HeaderName {
        /// The provider name.
        name: String,
        /// The rejected header name.
        header: String,
    },
    /// A header the bot sets itself.
    #[error("provider `{name}` sets header `{header}`, which the bot sets itself. Remove it")]
    ReservedHeader {
        /// The provider name.
        name: String,
        /// The header name.
        header: String,
    },
    /// A header value holds a character HTTP does not allow. The value can be a secret.
    #[error(
        "provider `{name}` sets header `{header}` to a value with a character HTTP does not allow. Fix the value"
    )]
    HeaderValue {
        /// The provider name.
        name: String,
        /// The header name.
        header: String,
    },
    /// A request field the bot owns.
    #[error("provider `{name}` sets request field `{field}`, which the bot sets itself. Remove it")]
    ReservedField {
        /// The provider name.
        name: String,
        /// The field name.
        field: String,
    },
}

/// One OpenAI-compatible model provider, built from values the caller already resolved.
///
/// `Debug` shows the key as `[redacted]`.
#[derive(Debug, Clone)]
pub struct Provider {
    name: String,
    endpoint: String,
    models: Vec<String>,
    api_key: Secret,
    headers: Vec<(String, String)>,
    extra: Map<String, Value>,
}

impl Provider {
    /// Checks every value and builds the provider.
    ///
    /// Requests go to `{base_url}/chat/completions`. `models` are tried in
    /// order. `headers` go out on each request. `extra` fields join each request body.
    ///
    /// # Errors
    ///
    /// Returns a [`ProviderError`] for a bad name, a base URL that is not plain
    /// https, an empty key, an empty model list or id, a bad or reserved
    /// header, or a reserved request field.
    pub fn new(
        name: &str,
        base_url: &Url,
        models: Vec<String>,
        api_key: Secret,
        headers: Vec<(String, String)>,
        extra: Map<String, Value>,
    ) -> Result<Self, ProviderError> {
        let named = || name.to_owned();
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Err(ProviderError::Name { name: named() });
        }
        if base_url.scheme() != "https"
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.host_str().is_none()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
        {
            return Err(ProviderError::BaseUrl {
                name: named(),
                url: base_url.to_string(),
            });
        }
        if api_key.expose().trim().is_empty() {
            return Err(ProviderError::EmptyKey { name: named() });
        }
        if models.is_empty() || models.iter().any(|model| model.trim().is_empty()) {
            return Err(ProviderError::Models { name: named() });
        }
        for (header, value) in &headers {
            check_header(name, header, value)?;
        }
        if let Some(field) = extra
            .keys()
            .find(|key| RESERVED_FIELDS.contains(&key.as_str()))
        {
            return Err(ProviderError::ReservedField {
                name: named(),
                field: field.clone(),
            });
        }
        let endpoint = format!(
            "{}/chat/completions",
            base_url.as_str().trim_end_matches('/')
        );
        Ok(Self {
            name: named(),
            endpoint,
            models,
            api_key,
            headers,
            extra,
        })
    }

    /// The provider name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The `chat/completions` URL.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// The model ids, in the order they are tried.
    pub fn models(&self) -> &[String] {
        &self.models
    }

    /// The API key.
    pub(crate) fn api_key(&self) -> &Secret {
        &self.api_key
    }

    /// The extra request headers.
    pub(crate) fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// The extra request body fields.
    pub(crate) fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// Checks one extra header of provider `name`.
fn check_header(name: &str, header: &str, value: &str) -> Result<(), ProviderError> {
    let is_token = !header.is_empty()
        && header
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b));
    if !is_token {
        return Err(ProviderError::HeaderName {
            name: name.to_owned(),
            header: header.to_owned(),
        });
    }
    if RESERVED_HEADERS.contains(&header.to_ascii_lowercase().as_str()) {
        return Err(ProviderError::ReservedHeader {
            name: name.to_owned(),
            header: header.to_owned(),
        });
    }
    if !value
        .bytes()
        .all(|b| b == b'\t' || (b' '..=b'~').contains(&b))
    {
        return Err(ProviderError::HeaderValue {
            name: name.to_owned(),
            header: header.to_owned(),
        });
    }
    Ok(())
}

/// How often one model is tried before the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retry {
    /// The most attempts per model, retries included.
    pub max_attempts_per_model: NonZeroU32,
}

impl Default for Retry {
    /// Two attempts per model.
    fn default() -> Self {
        Self {
            max_attempts_per_model: NonZeroU32::MIN.saturating_add(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).expect(text)
    }

    fn build(
        name: &str,
        base: &str,
        models: &[&str],
        headers: &[(&str, &str)],
        extra: &[&str],
    ) -> Result<Provider, ProviderError> {
        Provider::new(
            name,
            &url(base),
            models.iter().map(|model| (*model).to_owned()).collect(),
            Secret::new("sk-secret".to_owned()),
            headers
                .iter()
                .map(|(header, value)| ((*header).to_owned(), (*value).to_owned()))
                .collect(),
            extra
                .iter()
                .map(|key| ((*key).to_owned(), Value::Bool(true)))
                .collect(),
        )
    }

    #[test]
    fn a_valid_provider_builds_its_endpoint_and_hides_its_key() {
        let provider = build(
            "openrouter",
            "https://openrouter.ai/api/v1/",
            &["a/b", "c"],
            &[("X-Title", "ml-rust catalogue")],
            &["reasoning"],
        )
        .expect("valid");
        assert_eq!(
            provider.endpoint(),
            "https://openrouter.ai/api/v1/chat/completions"
        );
        assert_eq!(provider.models(), ["a/b", "c"]);
        let debug = format!("{provider:?}");
        assert!(
            debug.contains("[redacted]") && !debug.contains("sk-secret"),
            "{debug}"
        );
    }

    #[test]
    fn bad_values_are_refused() {
        let ok = "https://ollama.com/v1";
        assert!(matches!(
            build("", ok, &["m"], &[], &[]),
            Err(ProviderError::Name { .. })
        ));
        assert!(matches!(
            build("a b", ok, &["m"], &[], &[]),
            Err(ProviderError::Name { .. })
        ));
        for base in [
            "http://ollama.com/v1",
            "https://u:p@ollama.com/v1",
            "https://ollama.com/v1?x=1",
            "https://ollama.com/v1#f",
        ] {
            assert!(
                matches!(
                    build("p", base, &["m"], &[], &[]),
                    Err(ProviderError::BaseUrl { .. })
                ),
                "{base}"
            );
        }
        assert!(matches!(
            build("p", ok, &[], &[], &[]),
            Err(ProviderError::Models { .. })
        ));
        assert!(matches!(
            build("p", ok, &[" "], &[], &[]),
            Err(ProviderError::Models { .. })
        ));
        assert!(matches!(
            build("p", ok, &["m"], &[("Bad Name", "v")], &[]),
            Err(ProviderError::HeaderName { .. })
        ));
        assert!(matches!(
            build("p", ok, &["m"], &[("authorization", "v")], &[]),
            Err(ProviderError::ReservedHeader { .. })
        ));
        let value =
            build("p", ok, &["m"], &[("X-Key", "a\r\nInjected: b")], &[]).expect_err("refused");
        assert!(matches!(value, ProviderError::HeaderValue { .. }));
        assert!(!value.to_string().contains("Injected"));
        assert!(matches!(
            build("p", ok, &["m"], &[], &["messages"]),
            Err(ProviderError::ReservedField { .. })
        ));
        let empty_key = Provider::new(
            "p",
            &url(ok),
            vec!["m".to_owned()],
            Secret::new(" ".to_owned()),
            Vec::new(),
            Map::new(),
        );
        assert!(matches!(empty_key, Err(ProviderError::EmptyKey { .. })));
    }

    #[test]
    fn the_default_retry_tries_each_model_twice() {
        assert_eq!(Retry::default().max_attempts_per_model.get(), 2);
    }
}
