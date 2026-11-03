//! Conversion of parsed form answers into catalogue records and their TOML text.

mod entry;
mod judgment;
mod meta;
mod toml;

pub use self::toml::{draft_toml, entry_toml, judgment_toml};
pub use entry::{Affiliation, PrNotes, SlugSource, Submission, submission};
pub use judgment::{JudgmentDraft, judgment, judgment_draft};
pub use meta::{Approval, ENTRIES_DIR, IssueMeta, JUDGMENTS_DIR};
