//! The files for scripts and AI agents: the Atom feed, the data file, `llms.txt`,
//! `sitemap.xml` and `robots.txt`.
//!
//! They come from the same inputs as the pages and read no clock.

mod catalogue_file;
mod feed;
mod sitemap;
mod texts;

pub(crate) use catalogue_file::catalogue_file;
pub(crate) use feed::feed_file;
pub(crate) use sitemap::sitemap_file;
pub(crate) use texts::{data_readme_file, llms_file, robots_file};
