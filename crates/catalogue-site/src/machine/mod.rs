//! The files for scripts and AI agents: the Atom feed, the data file, `llms.txt` and the small
//! files it links, `sitemap.xml` and `robots.txt`.
//!
//! They come from the same inputs as the pages and read no clock.

mod agent_files;
mod catalogue_file;
mod compare_files;
mod feed;
mod sitemap;
mod texts;

pub(crate) use agent_files::agent_files;
pub(crate) use catalogue_file::catalogue_file;
pub(crate) use compare_files::compare_files;
pub(crate) use feed::feed_file;
pub(crate) use sitemap::sitemap_file;
pub(crate) use texts::{data_readme_file, llms_file, robots_file};
