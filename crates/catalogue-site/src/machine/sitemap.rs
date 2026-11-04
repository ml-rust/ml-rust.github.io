//! `sitemap.xml`: every page of the site. It carries no date, so it needs no clock.

use askama::Template;

use crate::build::{OutputFile, render};
use crate::error::SiteError;
use crate::pages::SITE_URL;

/// The path of the sitemap below the site root.
const SITEMAP_FILE: &str = "sitemap.xml";

/// The file name of a page below its directory.
const PAGE_FILE: &str = "index.html";

/// The data `sitemap.xml` reads.
#[derive(Debug, Template)]
#[template(path = "sitemap.xml")]
struct Sitemap {
    urls: Vec<String>,
}

/// The sitemap of `files`, in path order. Only `index.html` files are pages.
///
/// # Errors
///
/// Returns [`SiteError::Render`] when the template fails.
pub(crate) fn sitemap_file(files: &[OutputFile]) -> Result<OutputFile, SiteError> {
    let mut urls: Vec<String> = files
        .iter()
        .filter_map(|file| file.path.strip_suffix(PAGE_FILE))
        .filter(|dir| dir.is_empty() || dir.ends_with('/'))
        .map(|dir| format!("{SITE_URL}/{dir}"))
        .collect();
    urls.sort();
    render(SITEMAP_FILE.to_owned(), &Sitemap { urls })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> OutputFile {
        OutputFile {
            path: path.to_owned(),
            bytes: Vec::new(),
        }
    }

    #[test]
    fn the_sitemap_lists_each_page_once_in_order_and_nothing_else() {
        let files = [
            file("topics/index.html"),
            file("index.html"),
            file("404.html"),
            file("assets/site.css"),
            file("data/catalogue.json"),
            file("notindex.html"),
        ];
        let built = sitemap_file(&files).expect("renders");
        assert_eq!(built.path, "sitemap.xml");
        let text = String::from_utf8(built.bytes).expect("UTF-8");
        assert_eq!(
            text,
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
             <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n\
             <url><loc>https://ml-rust.github.io/</loc></url>\n\
             <url><loc>https://ml-rust.github.io/topics/</loc></url>\n\
             </urlset>"
        );
    }
}
