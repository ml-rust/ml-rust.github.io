//! Page data and the templates that render it.

mod layout;
mod not_found;
mod rules;
mod submit;

pub use layout::SITE_NAME;
pub(crate) use layout::{Layout, NavItem};
pub(crate) use not_found::NotFoundPage;
pub(crate) use rules::RulesPage;
pub(crate) use submit::SubmitPage;
