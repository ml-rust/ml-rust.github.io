//! The README of one entry, as the check fetched it.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::date::Date;
use crate::date_text::plain;

/// The README text of an entry's repository. A display fact: no tier rule reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadmeFact {
    /// The Markdown text. It is untrusted input.
    pub text: String,
    /// The path of the file in the repository. Relative links resolve against its folder.
    pub path: String,
    /// The default branch the text comes from.
    pub branch: String,
    /// Whether the file is longer than the fetch cap, so the text stops at the cap.
    pub truncated: bool,
    /// The date of the fetch that returned the text.
    #[serde(serialize_with = "plain")]
    pub fetched: Date,
    /// The page of the file on its forge.
    pub source: Url,
}
