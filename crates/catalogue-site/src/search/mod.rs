//! The search index of the site: one record per entry, encoded as index files.

mod aliases;
mod boost;
mod files;
mod records;

pub use boost::{NO_TIER_BOOST, TIER_BOOSTS};
pub use files::SEARCH_DIR;
pub(crate) use files::search_files;
pub use records::search_records;
pub(crate) use records::{KIND_TAG, ORG_AFFILIATED, ORG_COMMUNITY, ORG_TAG, TIER_TAG, TOPIC_TAG};
