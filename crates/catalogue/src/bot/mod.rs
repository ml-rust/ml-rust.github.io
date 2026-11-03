//! The `catalogue bot` command: the issue intake steps of the catalogue bot.
//!
//! `check` reads an issue form offline. `labels` syncs the repo labels as the
//! GitHub App named in the bot config. `run` checks every open issue with a
//! form label and writes the bot comment and labels.

mod command;
mod config;
mod labels;
pub mod run;

pub use command::{BotCommand, BotError, CheckArgs, bot_check, run_bot};
pub use config::{BotConfig, CONFIG_VAR, ConfigError, DEFAULT_CONFIG, config_path, load_config};
pub use labels::{LabelsArgs, LabelsCommandError, LabelsReport, sync_labels};
