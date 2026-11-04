//! The stylesheet, the favicon and the scripts, each named by its content hash.

use std::fmt::Write as _;

use aws_lc_rs::digest::{SHA256, digest};

use crate::build::OutputFile;
use crate::href::SafeHref;

/// The hand-written stylesheet of every page.
const SITE_CSS: &[u8] = include_bytes!("../assets/site.css");

/// The script of the search page.
const SEARCH_JS: &[u8] = include_bytes!("../assets/search.js");

/// The script of the list pages: sort, filter, copy and the compare selection.
const LIST_JS: &[u8] = include_bytes!("../assets/list.js");

/// The script of the compare page.
const COMPARE_JS: &[u8] = include_bytes!("../assets/compare.js");

/// The favicon of every page. It carries its own light and dark colours.
const FAVICON: &[u8] = include_bytes!("../assets/favicon.svg");

/// How many leading bytes of the SHA-256 digest go into the file name.
const HASH_BYTES: usize = 4;

/// A static file with its cache-busting path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HashedAsset {
    path: String,
    bytes: &'static [u8],
}

impl HashedAsset {
    /// The site stylesheet, at `assets/site.<hash>.css`.
    pub(crate) fn stylesheet() -> Self {
        Self::of("assets/site", "css", SITE_CSS)
    }

    /// The search page script, at `search/search.<hash>.js`.
    pub(crate) fn search_script() -> Self {
        Self::of("search/search", "js", SEARCH_JS)
    }

    /// The list script, at `assets/list.<hash>.js`.
    pub(crate) fn list_script() -> Self {
        Self::of("assets/list", "js", LIST_JS)
    }

    /// The compare page script, at `assets/compare.<hash>.js`.
    pub(crate) fn compare_script() -> Self {
        Self::of("assets/compare", "js", COMPARE_JS)
    }

    /// The favicon, at `assets/favicon.<hash>.svg`.
    pub(crate) fn favicon() -> Self {
        Self::of("assets/favicon", "svg", FAVICON)
    }

    /// The file with contents `bytes`, at `<stem>.<hash>.<ext>`.
    fn of(stem: &str, ext: &str, bytes: &'static [u8]) -> Self {
        Self {
            path: hashed_path(stem, ext, bytes),
            bytes,
        }
    }

    /// The link from a page to the file.
    pub(crate) fn href(&self) -> SafeHref {
        SafeHref::new(&format!("/{}", self.path))
    }

    /// The file as an output file.
    pub(crate) fn file(&self) -> OutputFile {
        OutputFile {
            path: self.path.clone(),
            bytes: self.bytes.to_vec(),
        }
    }
}

/// `<stem>.<hex>.<ext>`, where `<hex>` is the first 8 hex digits of the SHA-256 of `bytes`.
fn hashed_path(stem: &str, ext: &str, bytes: &[u8]) -> String {
    let found = digest(&SHA256, bytes);
    let mut path = format!("{stem}.");
    for byte in found.as_ref().iter().take(HASH_BYTES) {
        let _ = write!(path, "{byte:02x}");
    }
    path.push('.');
    path.push_str(ext);
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_holds_the_first_eight_hex_digits_of_the_sha256() {
        // SHA-256 of the empty input starts with e3b0c442.
        assert_eq!(
            hashed_path("assets/site", "css", b""),
            "assets/site.e3b0c442.css"
        );
        // SHA-256 of "abc" starts with ba7816bf.
        assert_eq!(
            hashed_path("search/search", "js", b"abc"),
            "search/search.ba7816bf.js"
        );
    }

    #[test]
    fn each_asset_links_its_own_file() {
        for (asset, bytes, stem, ext) in [
            (HashedAsset::stylesheet(), SITE_CSS, "assets/site", "css"),
            (HashedAsset::favicon(), FAVICON, "assets/favicon", "svg"),
            (
                HashedAsset::search_script(),
                SEARCH_JS,
                "search/search",
                "js",
            ),
            (HashedAsset::list_script(), LIST_JS, "assets/list", "js"),
            (
                HashedAsset::compare_script(),
                COMPARE_JS,
                "assets/compare",
                "js",
            ),
        ] {
            let file = asset.file();
            assert_eq!(file.bytes, bytes);
            assert_eq!(asset.href().as_str(), format!("/{}", file.path));
            assert!(!asset.href().is_refused());
            assert_eq!(file.path, hashed_path(stem, ext, bytes));
        }
    }

    #[test]
    fn other_contents_give_another_path() {
        assert_ne!(
            HashedAsset::of("assets/site", "css", b"a").file().path,
            HashedAsset::of("assets/site", "css", b"b").file().path
        );
    }
}
