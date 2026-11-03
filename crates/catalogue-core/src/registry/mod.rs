//! Registry files: maintainers, topics and synonyms.

mod maintainers;
mod synonyms;
mod topics;

pub use maintainers::{Maintainer, Maintainers};
pub use synonyms::{SynonymGroup, Synonyms};
pub use topics::{Topic, TopicSummary, Topics};
