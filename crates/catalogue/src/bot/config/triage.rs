//! The `[triage]` section of the bot config: the advisory LLM link check.
//!
//! The section names key variables, never keys. The bot reads each key from
//! the environment at the start of a run.

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::time::Duration;

use catalogue_triage::{Provider, ProviderError, Retry, WEB_FETCH_URL, check_endpoint};
use serde::Deserialize;
use serde_json::{Map, Number, Value};
use thiserror::Error;
use url::Url;

/// The links checked per issue when `max_links` is unset.
pub const DEFAULT_MAX_LINKS: usize = 5;
/// The seconds one model or fetch request waits when `request_timeout_secs` is unset.
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 90;
/// The attempts per model when `max_attempts_per_model` is unset.
pub const DEFAULT_MAX_ATTEMPTS_PER_MODEL: u32 = 2;
/// The seconds before a failed check runs again when `retry_after_secs` is unset.
pub const DEFAULT_RETRY_AFTER_SECS: u64 = 3600;

/// The link check config, with defaults filled in.
#[derive(Debug, Clone, PartialEq)]
pub struct TriageConfig {
    /// Whether the run checks evidence links.
    pub enabled: bool,
    /// The links checked per issue. Later links show as not checked.
    pub max_links: usize,
    /// The wait for one model or fetch response.
    pub request_timeout: Duration,
    /// How often one model is tried.
    pub retry: Retry,
    /// The seconds before a check that no model answered runs again.
    pub retry_after_secs: u64,
    /// The model providers, in the order they are tried.
    pub providers: Vec<ProviderConfig>,
    /// The `web_fetch` service. `None` reads GitHub pages only.
    pub fetch: Option<FetchConfig>,
}

impl Default for TriageConfig {
    /// The check is off. Every other value is its default.
    fn default() -> Self {
        Self {
            enabled: false,
            max_links: DEFAULT_MAX_LINKS,
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            retry: Retry::default(),
            retry_after_secs: DEFAULT_RETRY_AFTER_SECS,
            providers: Vec::new(),
            fetch: None,
        }
    }
}

/// One `[[triage.providers]]` table.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderConfig {
    /// The provider name in logs and errors.
    pub name: String,
    /// The OpenAI-compatible base URL. Requests go to `{base_url}/chat/completions`.
    pub base_url: Url,
    /// The environment variable that holds the API key.
    pub api_key_env: String,
    /// The model ids, in the order they are tried.
    pub models: Vec<String>,
    /// Extra request headers.
    pub headers: Vec<(String, String)>,
    /// Extra request body fields.
    pub request: Map<String, Value>,
}

/// The `[triage.fetch]` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchConfig {
    /// The Ollama `web_fetch` endpoint.
    pub web_fetch_url: Url,
    /// The environment variable that holds the `web_fetch` API key.
    pub api_key_env: String,
}

/// A `[triage]` value that cannot be used. The message names the field and the fix.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("`{field}`: {reason}")]
pub struct TriageConfigError {
    /// The dotted field path, such as `triage.providers.base_url`.
    pub field: String,
    /// What is wrong and how to fix it.
    pub reason: String,
}

impl TriageConfigError {
    fn new(field: &str, reason: impl Into<String>) -> Self {
        Self {
            field: field.to_owned(),
            reason: reason.into(),
        }
    }
}

/// The `[triage]` section as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawTriage {
    #[serde(default = "enabled_when_present")]
    enabled: bool,
    #[serde(default = "default_max_links")]
    max_links: usize,
    #[serde(default = "default_request_timeout")]
    request_timeout_secs: u64,
    #[serde(default = "default_max_attempts")]
    max_attempts_per_model: u32,
    #[serde(default = "default_retry_after")]
    retry_after_secs: u64,
    #[serde(default)]
    providers: Vec<RawProvider>,
    #[serde(default)]
    fetch: Option<RawFetch>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawProvider {
    name: String,
    base_url: Url,
    api_key_env: String,
    models: Vec<String>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    request: toml::Table,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFetch {
    #[serde(default)]
    web_fetch_url: Option<Url>,
    api_key_env: String,
}

/// A written `[triage]` section turns the check on unless `enabled` says otherwise.
fn enabled_when_present() -> bool {
    true
}

fn default_max_links() -> usize {
    DEFAULT_MAX_LINKS
}

fn default_request_timeout() -> u64 {
    DEFAULT_REQUEST_TIMEOUT_SECS
}

fn default_max_attempts() -> u32 {
    DEFAULT_MAX_ATTEMPTS_PER_MODEL
}

fn default_retry_after() -> u64 {
    DEFAULT_RETRY_AFTER_SECS
}

impl RawTriage {
    /// Checks every value and fills in the defaults.
    ///
    /// The provider checks are the ones [`Provider::new`] runs, without the key.
    pub(super) fn check(self) -> Result<TriageConfig, TriageConfigError> {
        if self.max_links == 0 {
            return Err(TriageConfigError::new(
                "triage.max_links",
                "is 0. Set it to 1 or more",
            ));
        }
        if self.request_timeout_secs == 0 {
            return Err(TriageConfigError::new(
                "triage.request_timeout_secs",
                "is 0. Set it to 1 or more",
            ));
        }
        let Some(max_attempts_per_model) = NonZeroU32::new(self.max_attempts_per_model) else {
            return Err(TriageConfigError::new(
                "triage.max_attempts_per_model",
                "is 0. Set it to 1 or more",
            ));
        };
        if self.enabled && self.providers.is_empty() {
            return Err(TriageConfigError::new(
                "triage.providers",
                "lists no provider while the check is enabled. Add a [[triage.providers]] table, or set enabled = false",
            ));
        }
        let providers = self
            .providers
            .into_iter()
            .map(|provider| provider.check("triage.providers"))
            .collect::<Result<Vec<_>, _>>()?;
        let fetch = self.fetch.map(RawFetch::check).transpose()?;
        Ok(TriageConfig {
            enabled: self.enabled,
            max_links: self.max_links,
            request_timeout: Duration::from_secs(self.request_timeout_secs),
            retry: Retry {
                max_attempts_per_model,
            },
            retry_after_secs: self.retry_after_secs,
            providers,
            fetch,
        })
    }
}

impl RawProvider {
    /// Checks one provider table. `section` names it in errors, such as `triage.providers`.
    pub(crate) fn check(self, section: &str) -> Result<ProviderConfig, TriageConfigError> {
        check_env_name(&format!("{section}.api_key_env"), &self.api_key_env)?;
        let request = json_table(self.request).ok_or_else(|| {
            TriageConfigError::new(
                &format!("{section}.request"),
                format!(
                    "provider `{}` sets a date, NaN or infinity, which JSON cannot carry. Use a string or a finite number",
                    self.name
                ),
            )
        })?;
        let headers: Vec<(String, String)> = self.headers.into_iter().collect();
        Provider::check(&self.name, &self.base_url, &self.models, &headers, &request).map_err(
            |error| {
                let field = format!("{section}.{}", provider_field(&error));
                TriageConfigError::new(&field, error.to_string())
            },
        )?;
        Ok(ProviderConfig {
            name: self.name,
            base_url: self.base_url,
            api_key_env: self.api_key_env,
            models: self.models,
            headers,
            request,
        })
    }
}

impl RawFetch {
    fn check(self) -> Result<FetchConfig, TriageConfigError> {
        let field = "triage.fetch.web_fetch_url";
        let web_fetch_url = match self.web_fetch_url {
            Some(url) => url,
            None => Url::parse(WEB_FETCH_URL)
                .map_err(|error| TriageConfigError::new(field, error.to_string()))?,
        };
        check_endpoint(&web_fetch_url)
            .map_err(|error| TriageConfigError::new(field, error.to_string()))?;
        check_env_name("triage.fetch.api_key_env", &self.api_key_env)?;
        Ok(FetchConfig {
            web_fetch_url,
            api_key_env: self.api_key_env,
        })
    }
}

/// The config key of the value `error` rejects.
fn provider_field(error: &ProviderError) -> &'static str {
    match error {
        ProviderError::Name { .. } => "name",
        ProviderError::BaseUrl { .. } => "base_url",
        ProviderError::EmptyKey { .. } => "api_key_env",
        ProviderError::Models { .. } => "models",
        ProviderError::HeaderName { .. }
        | ProviderError::ReservedHeader { .. }
        | ProviderError::HeaderValue { .. } => "headers",
        ProviderError::ReservedField { .. } => "request",
    }
}

/// Checks that `name` is an environment variable name: ASCII letters, digits
/// and `_`, not starting with a digit.
fn check_env_name(field: &str, name: &str) -> Result<(), TriageConfigError> {
    let valid = name
        .bytes()
        .next()
        .is_some_and(|first| !first.is_ascii_digit())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if valid {
        Ok(())
    } else {
        Err(TriageConfigError::new(
            field,
            format!(
                "`{name}` is not an environment variable name. Use ASCII letters, digits and `_`, such as OLLAMA_API_KEY"
            ),
        ))
    }
}

/// `table` as a JSON object, or `None` when it holds a date or a float JSON cannot carry.
fn json_table(table: toml::Table) -> Option<Map<String, Value>> {
    table
        .into_iter()
        .map(|(key, value)| json_value(value).map(|value| (key, value)))
        .collect()
}

/// `value` as JSON, or `None` when it holds a date or a float JSON cannot carry.
fn json_value(value: toml::Value) -> Option<Value> {
    Some(match value {
        toml::Value::String(text) => Value::String(text),
        toml::Value::Integer(number) => Value::Number(number.into()),
        toml::Value::Float(number) => Value::Number(Number::from_f64(number)?),
        toml::Value::Boolean(flag) => Value::Bool(flag),
        toml::Value::Datetime(_) => return None,
        toml::Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(json_value)
                .collect::<Option<Vec<_>>>()?,
        ),
        toml::Value::Table(table) => Value::Object(json_table(table)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<TriageConfig, TriageConfigError> {
        let raw: RawTriage = toml::from_str(text).expect("toml");
        raw.check()
    }

    const PROVIDER: &str = "[[providers]]\nname = \"ollama\"\nbase_url = \"https://ollama.com/v1\"\n\
                            api_key_env = \"OLLAMA_API_KEY\"\nmodels = [\"m\"]\n";

    #[test]
    fn defaults_fill_every_unset_value() {
        let config = parse(PROVIDER).expect("valid");
        assert!(config.enabled);
        assert_eq!(config.max_links, DEFAULT_MAX_LINKS);
        assert_eq!(config.request_timeout, Duration::from_secs(90));
        assert_eq!(config.retry, Retry::default());
        assert_eq!(config.retry_after_secs, DEFAULT_RETRY_AFTER_SECS);
        assert_eq!(config.fetch, None);
        assert_eq!(config.providers[0].models, ["m"]);
        assert!(!TriageConfig::default().enabled);
    }

    #[test]
    fn request_fields_become_json() {
        let text = format!(
            "{PROVIDER}[providers.request]\nreasoning = {{ effort = \"low\" }}\ntop_p = 0.5\nstop = [\"a\"]\n\
             [providers.headers]\nX-Title = \"ml-rust catalogue\"\n"
        );
        let config = parse(&text).expect("valid");
        let provider = &config.providers[0];
        assert_eq!(
            Value::Object(provider.request.clone()),
            serde_json::json!({"reasoning": {"effort": "low"}, "top_p": 0.5, "stop": ["a"]})
        );
        assert_eq!(
            provider.headers,
            [("X-Title".to_owned(), "ml-rust catalogue".to_owned())]
        );
    }

    #[test]
    fn the_fetch_url_defaults_to_ollama() {
        let config = parse(&format!(
            "[fetch]\napi_key_env = \"OLLAMA_API_KEY\"\n{PROVIDER}"
        ))
        .expect("valid");
        let fetch = config.fetch.expect("fetch");
        assert_eq!(fetch.web_fetch_url.as_str(), WEB_FETCH_URL);
    }

    #[test]
    fn bad_values_name_their_field() {
        for (text, field) in [
            (format!("max_links = 0\n{PROVIDER}"), "triage.max_links"),
            (
                format!("request_timeout_secs = 0\n{PROVIDER}"),
                "triage.request_timeout_secs",
            ),
            (
                format!("max_attempts_per_model = 0\n{PROVIDER}"),
                "triage.max_attempts_per_model",
            ),
            (String::new(), "triage.providers"),
            (
                PROVIDER.replace("https://ollama.com", "http://ollama.com"),
                "triage.providers.base_url",
            ),
            (PROVIDER.replace("[\"m\"]", "[]"), "triage.providers.models"),
            (
                PROVIDER.replace("OLLAMA_API_KEY", "OLLAMA KEY"),
                "triage.providers.api_key_env",
            ),
            (
                format!("{PROVIDER}[providers.request]\nmodel = \"x\"\n"),
                "triage.providers.request",
            ),
            (
                format!("{PROVIDER}[providers.request]\nwhen = 2026-11-04\n"),
                "triage.providers.request",
            ),
            (
                format!("{PROVIDER}[providers.headers]\nAuthorization = \"x\"\n"),
                "triage.providers.headers",
            ),
            (
                format!(
                    "[fetch]\nweb_fetch_url = \"http://ollama.com/api/web_fetch\"\napi_key_env = \"K\"\n{PROVIDER}"
                ),
                "triage.fetch.web_fetch_url",
            ),
        ] {
            let error = parse(&text).expect_err(&text);
            assert_eq!(error.field, field, "{error}");
            assert!(
                error.to_string().starts_with(&format!("`{field}`: ")),
                "{error}"
            );
        }
        assert!(parse("enabled = false\n").is_ok());
    }
}
