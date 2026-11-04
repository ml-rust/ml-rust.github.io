//! Registry files: maintainers, topics, categories, synonyms, discovery rules,
//! tier thresholds and deprecated requirements.

mod categories;
mod deprecated;
mod discovery;
mod maintainers;
mod mappings;
mod synonyms;
mod thresholds;
mod topics;

pub use categories::{Categories, Category};
pub use deprecated::{Deprecated, DeprecatedRequirement, RequirementCheck};
pub use discovery::Discovery;
pub use maintainers::{Maintainer, Maintainers};
pub use synonyms::{SynonymGroup, Synonyms};
pub use thresholds::{
    AppFeaturedThresholds, DormancyThresholds, FallbackThresholds, FinishedThresholds,
    IntervalThresholds, LibraryFeaturedThresholds, NewThresholds, RotThresholds, Thresholds,
};
pub use topics::{Topic, TopicSummary, Topics};
