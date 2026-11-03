//! Tier rules: published thresholds applied to raw signals, with no IO.

mod admit;
mod app;
mod archive;
mod context;
mod dormancy;
mod engine;
mod fact;
mod featured;
mod finished;
#[cfg(test)]
mod fixtures;
mod guard;
mod interval;
mod level;
mod new;
mod notes;
mod rot;
mod signals;
mod verdict;

pub(crate) use app::APP_SIGNALS;
pub use context::Context;
pub use engine::assess;
pub use level::Tier;
pub(crate) use rot::ROT_SIGNALS;
pub use signals::Signals;
pub use verdict::{Reason, Rule, Verdict};
