//! The config of `catalogue state`. It names key variables, never keys.

use std::fs;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::time::Duration;

use catalogue_triage::Retry;
use serde::Deserialize;
use thiserror::Error;

use crate::bot::{ProviderConfig, RawProvider};

/// The config, with defaults filled in.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct StateConfig {
    /// The wait for one model response.
    pub request_timeout: Duration,
    /// How often one model is tried.
    pub retry: Retry,
    /// The model providers, in the order they are tried.
    pub providers: Vec<ProviderConfig>,
}

/// A config file that cannot be used.
#[derive(Debug, Error)]
#[error("the state config {}: {reason}. Compare it with state.example.toml", .path.display())]
pub(super) struct StateConfigError {
    path: PathBuf,
    reason: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    request_timeout_secs: u64,
    max_attempts_per_model: u32,
    providers: Vec<RawProvider>,
}

/// Reads and checks the config at `path`.
pub(super) fn load_state_config(path: &Path) -> Result<StateConfig, StateConfigError> {
    let fail = |reason: String| StateConfigError {
        path: path.to_path_buf(),
        reason,
    };
    let text = fs::read_to_string(path).map_err(|error| fail(error.to_string()))?;
    let raw: RawConfig =
        toml::from_str(&text).map_err(|error| fail(error.message().trim_end().to_owned()))?;
    if raw.request_timeout_secs == 0 {
        return Err(fail(
            "`request_timeout_secs` is 0. Set it to 1 or more".to_owned(),
        ));
    }
    let max_attempts_per_model = NonZeroU32::new(raw.max_attempts_per_model)
        .ok_or_else(|| fail("`max_attempts_per_model` is 0. Set it to 1 or more".to_owned()))?;
    if raw.providers.is_empty() {
        return Err(fail(
            "`providers` is empty. Add a [[providers]] table".to_owned(),
        ));
    }
    let providers = raw
        .providers
        .into_iter()
        .map(|provider| provider.check("providers"))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| fail(error.to_string()))?;
    Ok(StateConfig {
        request_timeout: Duration::from_secs(raw.request_timeout_secs),
        retry: Retry {
            max_attempts_per_model,
        },
        providers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_config_loads_with_ollama_first() {
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../state.example.toml");
        let config = load_state_config(&example).expect("example loads");
        let names: Vec<&str> = config.providers.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["ollama", "openrouter"]);
        assert_eq!(config.providers[0].api_key_env, "OLLAMA_API_KEY");
    }

    #[test]
    fn a_missing_file_names_the_path() {
        let error = load_state_config(Path::new("/nonexistent/state.toml")).expect_err("missing");
        assert!(
            error.to_string().contains("/nonexistent/state.toml"),
            "{error}"
        );
    }
}
