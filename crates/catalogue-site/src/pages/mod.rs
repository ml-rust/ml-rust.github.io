//! Page data and the templates that render it.

mod changes;
mod home;
mod layout;
mod not_found;
mod project;
mod rules;
mod search;
mod shared;
mod submit;
mod topics;

pub(crate) use changes::ChangesPage;
pub(crate) use home::HomePage;
pub use layout::SITE_NAME;
pub(crate) use layout::{Layout, NavItem};
pub(crate) use not_found::NotFoundPage;
pub(crate) use project::ProjectPage;
pub(crate) use rules::RulesPage;
pub(crate) use search::SearchPage;
pub(crate) use shared::{project_file, project_href, topic_file, topic_link};
pub(crate) use submit::SubmitPage;
pub(crate) use topics::{TopicPage, TopicsPage};
