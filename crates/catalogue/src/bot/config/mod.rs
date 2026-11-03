//! The bot config file: the repo, the GitHub App, its private key and the link check.

mod load;
mod triage;

pub use self::load::{
    BotConfig, CONFIG_VAR, ConfigError, DEFAULT_CONFIG, config_path, load_config,
};
pub use self::triage::{
    DEFAULT_MAX_ATTEMPTS_PER_MODEL, DEFAULT_MAX_LINKS, DEFAULT_REQUEST_TIMEOUT_SECS,
    DEFAULT_RETRY_AFTER_SECS, FetchConfig, ProviderConfig, TriageConfig, TriageConfigError,
};
