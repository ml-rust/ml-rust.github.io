//! Registry files: maintainers, topics, synonyms, discovery rules and tier thresholds.

mod discovery;
mod maintainers;
mod synonyms;
mod thresholds;
mod topics;

pub use discovery::Discovery;
pub use maintainers::{Maintainer, Maintainers};
pub use synonyms::{SynonymGroup, Synonyms};
pub use thresholds::{
    AppFeaturedThresholds, DormancyThresholds, FinishedThresholds, IntervalThresholds,
    LibraryFeaturedThresholds, NewThresholds, RotThresholds, Thresholds,
};
pub use topics::{Topic, TopicSummary, Topics};
