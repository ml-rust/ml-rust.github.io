//! The stylesheet, named by its content hash.

use std::fmt::Write as _;

use aws_lc_rs::digest::{SHA256, digest};

use crate::build::OutputFile;
use crate::href::SafeHref;

/// The hand-written stylesheet of every page.
const SITE_CSS: &[u8] = include_bytes!("../assets/site.css");

/// How many leading bytes of the SHA-256 digest go into the file name.
const HASH_BYTES: usize = 4;

/// The stylesheet with its cache-busting path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stylesheet {
    path: String,
    bytes: &'static [u8],
}

impl Stylesheet {
    /// The site stylesheet.
    pub(crate) fn site() -> Self {
        Self::of(SITE_CSS)
    }

    /// The stylesheet with contents `bytes`, at `assets/site.<hash>.css`.
    fn of(bytes: &'static [u8]) -> Self {
        Self {
            path: hashed_path(bytes),
            bytes,
        }
    }

    /// The link from a page to the stylesheet.
    pub(crate) fn href(&self) -> SafeHref {
        SafeHref::new(&format!("/{}", self.path))
    }

    /// The stylesheet as an output file.
    pub(crate) fn file(&self) -> OutputFile {
        OutputFile {
            path: self.path.clone(),
            bytes: self.bytes.to_vec(),
        }
    }
}

/// `assets/site.<hex>.css`, where `<hex>` is the first 8 hex digits of the SHA-256 of `bytes`.
fn hashed_path(bytes: &[u8]) -> String {
    let found = digest(&SHA256, bytes);
    let mut path = "assets/site.".to_owned();
    for byte in found.as_ref().iter().take(HASH_BYTES) {
        let _ = write!(path, "{byte:02x}");
    }
    path.push_str(".css");
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_holds_the_first_eight_hex_digits_of_the_sha256() {
        // SHA-256 of the empty input starts with e3b0c442.
        assert_eq!(hashed_path(b""), "assets/site.e3b0c442.css");
        // SHA-256 of "abc" starts with ba7816bf.
        assert_eq!(hashed_path(b"abc"), "assets/site.ba7816bf.css");
    }

    #[test]
    fn the_site_stylesheet_links_its_own_file() {
        let sheet = Stylesheet::site();
        let file = sheet.file();
        assert_eq!(file.bytes, SITE_CSS);
        assert_eq!(sheet.href().as_str(), format!("/{}", file.path));
        assert!(!sheet.href().is_refused());
        assert_eq!(file.path, hashed_path(SITE_CSS));
    }

    #[test]
    fn other_contents_give_another_path() {
        assert_ne!(
            Stylesheet::of(b"a").file().path,
            Stylesheet::of(b"b").file().path
        );
    }
}
