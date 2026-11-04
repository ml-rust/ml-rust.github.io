//! `/projects/<slug>/`: one entry with its links, tier reasons, signals and judgments.

mod action;
mod also_in;
mod compat;
mod header;
mod judgments;
mod links;
mod measurements;
mod page;
mod reason_text;
mod sources;
mod stats;
mod tier;
mod why;

pub(crate) use page::ProjectPage;
pub(crate) use why::WhyText;
