//! `catalogue bot run`: one poll pass that checks every open issue with a form label.

mod command;
mod error;
mod issue;
mod lock;
mod pass;
mod state;

pub use command::RunArgs;
pub(crate) use command::run_run;
pub use error::{IssueError, RunError};
pub use lock::{LOCK_FILE, StateLock, take_lock};
pub use pass::{MAIN_BRANCH, RunReport, SIGNALS_BRANCH, run_pass};
pub use state::{
    BotState, IssueState, STATE_DIRECTORY_VAR, STATE_FILE, STATE_VERSION, StateError, state_dir,
};
