//! The bot config file: the repo, the GitHub App and its private key.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

use catalogue_signals::github::{RepoRef, RepoRefError};
use serde::Deserialize;
use thiserror::Error;

/// The environment variable that names the config file.
pub const CONFIG_VAR: &str = "CATALOGUE_BOT_CONFIG";

/// The config file read when neither the flag nor the variable names one.
pub const DEFAULT_CONFIG: &str = "bot.toml";

/// The bot config, with relative paths resolved against the config file's directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotConfig {
    /// The repo the bot works on.
    pub repo: RepoRef,
    /// The GitHub App id.
    pub app_id: NonZeroU64,
    /// The installation of the App on the repo owner.
    pub installation_id: NonZeroU64,
    /// The App private key in PEM form.
    pub private_key_path: PathBuf,
    /// The directory for bot state, when set.
    pub state_dir: Option<PathBuf>,
}

/// A config file that cannot be used.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The file cannot be read.
    #[error(
        "cannot read the bot config {}: {source}. Pass --config, set {CONFIG_VAR}, or copy bot.example.toml to bot.toml",
        .path.display()
    )]
    Read {
        /// The config file.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
    /// The file is not TOML in the config shape.
    #[error("the bot config {} is not valid: {reason}. Compare it with bot.example.toml", .path.display())]
    Parse {
        /// The config file.
        path: PathBuf,
        /// What the parser reports.
        reason: String,
    },
    /// `owner` and `repo` do not name a repo.
    #[error("the bot config {}: {source}", .path.display())]
    Repo {
        /// The config file.
        path: PathBuf,
        /// The rejected `owner/repo`.
        source: RepoRefError,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    owner: String,
    repo: String,
    app_id: NonZeroU64,
    installation_id: NonZeroU64,
    private_key_path: PathBuf,
    #[serde(default)]
    state_dir: Option<PathBuf>,
}

/// The config path: `flag` when given, else a non-empty `env` value, else [`DEFAULT_CONFIG`].
pub fn config_path(flag: Option<&Path>, env: Option<OsString>) -> PathBuf {
    if let Some(path) = flag {
        return path.to_path_buf();
    }
    match env {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => PathBuf::from(DEFAULT_CONFIG),
    }
}

/// Reads and checks the config at `path`.
///
/// The file holds no secret. It names the key file, and the key stays there.
///
/// # Errors
///
/// Returns a [`ConfigError`] when the file cannot be read, is not in the
/// config shape, or names no valid repo.
pub fn load_config(path: &Path) -> Result<BotConfig, ConfigError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse_config(path, &text)
}

/// Parses config `text` read from `path`.
fn parse_config(path: &Path, text: &str) -> Result<BotConfig, ConfigError> {
    let raw: RawConfig = toml::from_str(text).map_err(|error| ConfigError::Parse {
        path: path.to_path_buf(),
        reason: error.message().trim_end().to_owned(),
    })?;
    let repo = RepoRef::parse(&format!("{}/{}", raw.owner, raw.repo)).map_err(|source| {
        ConfigError::Repo {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let base = path.parent().unwrap_or(Path::new(""));
    Ok(BotConfig {
        repo,
        app_id: raw.app_id,
        installation_id: raw.installation_id,
        private_key_path: base.join(raw.private_key_path),
        state_dir: raw.state_dir.map(|dir| base.join(dir)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../../../../bot.example.toml");

    #[test]
    fn the_flag_wins_then_the_variable_then_the_default() {
        assert_eq!(
            config_path(Some(Path::new("a.toml")), Some("b.toml".into())),
            PathBuf::from("a.toml")
        );
        assert_eq!(
            config_path(None, Some("b.toml".into())),
            PathBuf::from("b.toml")
        );
        assert_eq!(
            config_path(None, Some(OsString::new())),
            PathBuf::from("bot.toml")
        );
        assert_eq!(config_path(None, None), PathBuf::from("bot.toml"));
    }

    #[test]
    fn the_example_parses() {
        let config = parse_config(Path::new("bot.example.toml"), EXAMPLE).expect("parses");
        assert_eq!(config.repo.to_string(), "ml-rust/ml-rust.github.io");
        assert!(config.private_key_path.is_absolute());
    }

    #[test]
    fn relative_paths_resolve_against_the_config_directory() {
        let text = "owner = \"o\"\nrepo = \"r\"\napp_id = 1\ninstallation_id = 2\n\
                    private_key_path = \"keys/app.pem\"\nstate_dir = \"state\"\n";
        let config = parse_config(Path::new("/etc/catalogue/bot.toml"), text).expect("parses");
        assert_eq!(
            config.private_key_path,
            PathBuf::from("/etc/catalogue/keys/app.pem")
        );
        assert_eq!(
            config.state_dir,
            Some(PathBuf::from("/etc/catalogue/state"))
        );
        assert_eq!(config.app_id.get(), 1);
        let bare = parse_config(Path::new("bot.toml"), text).expect("parses");
        assert_eq!(bare.private_key_path, PathBuf::from("keys/app.pem"));
    }

    #[test]
    fn bad_configs_name_the_file() {
        let base = "owner = \"o\"\nrepo = \"r\"\ninstallation_id = 2\nprivate_key_path = \"k\"\n";
        for (text, needle) in [
            (base.to_owned(), "app_id"),
            (format!("{base}app_id = 0\n"), "nonzero"),
            (format!("{base}app_id = 1\ntoken = \"x\"\n"), "token"),
        ] {
            let error = parse_config(Path::new("bot.toml"), &text).expect_err("rejected");
            let message = error.to_string();
            assert!(
                message.starts_with("the bot config bot.toml is not valid"),
                "{message}"
            );
            assert!(message.contains(needle), "{message}");
        }
        let error = parse_config(
            Path::new("bot.toml"),
            "owner = \"a b\"\nrepo = \"r\"\napp_id = 1\ninstallation_id = 2\nprivate_key_path = \"k\"\n",
        )
        .expect_err("rejected");
        assert!(matches!(error, ConfigError::Repo { .. }), "{error}");
    }
}
