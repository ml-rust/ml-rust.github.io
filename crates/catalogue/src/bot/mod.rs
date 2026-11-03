//! The `catalogue bot` command: the issue intake steps of the catalogue bot.

mod command;

pub use command::{BotCommand, BotError, CheckArgs, bot_check, run_bot};
