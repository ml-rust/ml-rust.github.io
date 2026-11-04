//! Page data and the templates that render it.

mod category;
mod changes;
mod compare;
mod featured;
mod home;
mod layout;
mod not_found;
mod project;
mod rules;
mod search;
mod shared;
mod submit;
mod topics;

pub(crate) use category::CategoryPage;
pub(crate) use changes::ChangesPage;
pub(crate) use compare::ComparePage;
pub(crate) use featured::FeaturedPage;
pub(crate) use home::HomePage;
pub(crate) use layout::{Layout, NavItem};
pub use layout::{SITE_NAME, SITE_URL};
pub(crate) use not_found::NotFoundPage;
pub(crate) use project::{ProjectPage, WhyText};
pub(crate) use rules::RulesPage;
pub(crate) use search::SearchPage;
pub(crate) use shared::{
    ChoiceText, category_file, category_href, category_link, docs_href, first_check_line,
    install_line, is_first_check, native_code_fact, project_file, project_href, row_statuses,
    tier_name, topic_file, topic_href, topic_link,
};
pub(crate) use submit::SubmitPage;
pub(crate) use topics::{TopicPage, TopicsPage};
