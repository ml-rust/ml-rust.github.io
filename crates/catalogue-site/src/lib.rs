//! Static site renderer for the ml-rust catalogue.
//!
//! The crate reads no files, opens no connections, and reads no clock or
//! environment. [`build_site`] takes a checked [`catalogue_core::Catalogue`],
//! an optional [`catalogue_core::TierReport`] and the tier changes in
//! [`SiteInputs`]. It returns every page and asset as an [`OutputFile`]:
//! the home page, the topic list, one page per topic, category and entry, the tier
//! changes, the rules, the submit page, the not-found page, the search page,
//! its script and the search index under [`SEARCH_DIR`]. The deploy adds the
//! wasm module of the search engine to [`SEARCH_DIR`]. [`search_records`]
//! gives one search record per entry.
//! Without a report, no page shows a tier.
//!
//! Templates escape every value. [`SafeHref`] carries every link built from data.

mod asset;
mod build;
mod error;
mod forms;
mod href;
mod inputs;
mod pages;
mod refusal;
mod search;

pub use build::{OutputFile, build_site};
pub use error::SiteError;
pub use forms::{ISSUE_FORMS, IssueForm};
pub use href::SafeHref;
pub use inputs::{SiteInputs, TierChangeRow};
pub use pages::SITE_NAME;
pub use search::{NO_TIER_BOOST, SEARCH_DIR, TIER_BOOSTS, search_records};
