//! Static site renderer for the ml-rust catalogue.
//!
//! The crate reads no files, opens no connections, and reads no clock or
//! environment. [`build_site`] takes a checked [`catalogue_core::Catalogue`],
//! an optional [`catalogue_core::TierReport`] and the tier changes in
//! [`SiteInputs`]. It returns every page and asset as an [`OutputFile`].
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

pub use build::{OutputFile, build_site};
pub use error::SiteError;
pub use forms::{ISSUE_FORMS, IssueForm};
pub use href::SafeHref;
pub use inputs::{SiteInputs, TierChangeRow};
pub use pages::SITE_NAME;
