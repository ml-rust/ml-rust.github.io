//! `catalogue bot run`: one poll pass that checks every open issue with a form
//! label, turns approved issues into data PRs, and runs the advisory LLM link check.

mod command;
mod error;
mod issue;
mod lock;
mod pass;
mod pr;
mod state;
mod triage;

pub use command::RunArgs;
pub(crate) use command::run_run;
pub use error::{IssueError, RunError};
pub use lock::{LOCK_FILE, StateLock, take_lock};
pub use pass::{MAIN_BRANCH, PassDeps, RunReport, SIGNALS_BRANCH, run_pass};
pub use state::{
    BotState, IssueState, STATE_DIRECTORY_VAR, STATE_FILE, STATE_VERSION, StateError, state_dir,
};
pub use triage::{EnvLookup, TriageDeps};
