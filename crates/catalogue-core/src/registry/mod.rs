//! Registry files: maintainers, topics, synonyms and discovery rules.

mod discovery;
mod maintainers;
mod synonyms;
mod topics;

pub use discovery::Discovery;
pub use maintainers::{Maintainer, Maintainers};
pub use synonyms::{SynonymGroup, Synonyms};
pub use topics::{Topic, TopicSummary, Topics};
