//! The advisory LLM link check: pure parts.
//!
//! [`classify`] sorts an evidence link into a GitHub page, another page or a
//! refused link. [`Claim`] holds what the issue claims. [`prompt()`] frames the
//! claim and one page as data for a model. [`ChatRequest`] and
//! [`ChatResponse`] are the `chat/completions` body and answer.
//! [`parse_answer`] reads the model reply strictly, and [`anchor`] drops a
//! verdict whose quote is not in the page. [`triage_section`] renders the
//! results as a collapsed, advisory comment section. [`section_within`] cuts
//! it to fit a comment, and [`section_of_comment`] reads it back from the last
//! bot comment. [`triage_digest`] hashes the claim, the links and the page
//! sources, and [`evidence_links`] lists the links of a form.
//!
//! Model output only ever becomes comment text. No label, tier or approval reads it.

mod chat;
mod claim;
mod digest;
mod link;
mod prompt;
mod render;
mod verdict;

pub use chat::{ChatChoice, ChatMessage, ChatRequest, ChatResponse};
pub use claim::{Claim, ClaimField, evidence_links};
pub use digest::triage_digest;
pub use link::{GithubRepo, GithubTarget, LinkClass, classify};
pub use prompt::{MAX_PAGE_BYTES, MAX_QUOTE_CHARS, Prompt, SYSTEM_PROMPT, page_excerpt, prompt};
pub(crate) use render::section_floor;
pub use render::{LinkResult, SECTION_SUMMARY, section_of_comment, section_within, triage_section};
pub use verdict::{AnswerError, Checked, ModelAnswer, Outcome, Verdict, anchor, parse_answer};
