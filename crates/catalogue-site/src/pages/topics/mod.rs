//! `/topics/`: the topic grid, and one page per topic with its entries.

mod index;
mod topic;

pub(crate) use index::TopicsPage;
pub(crate) use topic::TopicPage;
