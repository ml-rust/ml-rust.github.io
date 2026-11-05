//! Candidate discovery over a crates.io database dump snapshot.
//!
//! [`candidates`] selects crates by category, keyword, a term in the name or description, or
//! dependency on a core crate, and includes the core crates and the named crates. The selection is
//! a pure function of the snapshot and the [`Rules`].
//!
//! [`group_repos`], [`drop_known`] and [`select_leads`] build the below-floor review list from
//! those candidates.

mod below_floor;
mod candidates;
mod rule_crates;
mod terms;
mod types;

pub use below_floor::{
    DESCRIPTION_CHARS, Group, Grouped, Lead, Selected, TERMS_SHOWN, drop_known, group_repos,
    select_leads,
};
pub use candidates::candidates;
pub(crate) use terms::text_words;
pub use types::{Candidate, Discovered, Reason, Rules};
