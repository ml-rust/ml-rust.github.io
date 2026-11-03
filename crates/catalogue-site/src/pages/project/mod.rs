//! `/projects/<slug>/`: one entry with its links, tier reasons, signals and judgments.

mod header;
mod judgments;
mod links;
mod page;
mod tier;

pub(crate) use page::ProjectPage;
