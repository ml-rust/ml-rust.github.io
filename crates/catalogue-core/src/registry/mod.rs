//! Registry files: maintainers, topics, categories, synonyms, discovery rules,
//! tier thresholds, deprecated requirements, the backend table and the showcase.

mod backends;
mod below_floor;
mod categories;
mod deprecated;
mod discovery;
mod maintainers;
mod mappings;
mod showcase;
mod synonyms;
mod thresholds;
mod topics;

pub use backends::{Backend, Backends, NO_GPU_FEATURE_ID, NOT_MEASURED_ID};
pub use below_floor::BelowFloor;
pub use categories::{Categories, Category};
pub use deprecated::{Deprecated, DeprecatedRequirement, RequirementCheck};
pub use discovery::Discovery;
pub use maintainers::{Maintainer, Maintainers};
pub use showcase::Showcase;
pub use synonyms::{SynonymGroup, Synonyms};
pub use thresholds::{
    AppFeaturedThresholds, DormancyThresholds, FallbackThresholds, FinishedThresholds,
    IntervalThresholds, LibraryFeaturedThresholds, MovementThresholds, NewThresholds,
    RepoRouteThresholds, RotThresholds, Thresholds,
};
pub use topics::{Topic, TopicSummary, Topics};
