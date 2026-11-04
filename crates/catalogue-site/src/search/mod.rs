//! The search index of the site: one record per entry, encoded as index files.

mod aliases;
mod boost;
mod choice_tags;
mod files;
mod records;
mod row_tags;

pub use boost::{NO_TIER_BOOST, TIER_BOOSTS};
pub(crate) use choice_tags::{LICENSE_TAG, RUNS_ON_TAG, is_stated_license, license_value};
pub use files::SEARCH_DIR;
pub(crate) use files::search_files;
pub use records::search_records;
pub(crate) use records::{
    CATEGORY_TAG, KIND_TAG, ORG_AFFILIATED, ORG_COMMUNITY, ORG_TAG, TIER_TAG, TOPIC_TAG,
};
