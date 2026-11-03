//! The bot check comment: its marker line, its text, and the writes an issue needs.

mod body;
mod digest;
mod marker;
mod names;
mod plan;

pub use body::{MAX_COMMENT_BYTES, comment_body};
pub use digest::Sha256;
pub use marker::Marker;
pub use names::{APPROVED, BOT_CHECKED, NEEDS_FIX};
pub use plan::{Actions, CommentAction, Desired, Previous, plan};
