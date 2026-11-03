//! Pure helpers that turn raw repository and description text into entry fields.

mod description;
mod name;
mod repo;

pub use description::description_text;
pub use name::entry_name;
pub use repo::repo_link;
