//! The README value and the size cap on its text.

use catalogue_core::{Date, ReadmeFact};
use serde::{Deserialize, Serialize};
use url::Url;

/// The longest README text kept, in bytes. Longer text stops here and is marked truncated.
pub const MAX_README: usize = 256 << 10;

/// The most bytes a raw file fetch reads: one past [`MAX_README`], so a longer file shows as cut.
pub(super) fn raw_limit() -> u64 {
    u64::try_from(MAX_README).map_or(u64::MAX, |cap| cap.saturating_add(1))
}

/// The README of a repo. The text is untrusted input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoReadme {
    /// The Markdown text.
    pub text: String,
    /// The path of the file in the repo.
    pub path: String,
    /// The default branch.
    pub branch: String,
    /// Whether the file is longer than [`MAX_README`].
    pub truncated: bool,
    /// The page of the file on its forge.
    pub source: Url,
}

impl RepoReadme {
    /// The README built from the file `bytes`, cut at [`MAX_README`].
    pub(super) fn new(bytes: &[u8], path: String, branch: String, source: Url) -> Self {
        let (text, truncated) = capped_text(bytes);
        Self {
            text,
            path,
            branch,
            truncated,
            source,
        }
    }

    /// The report form of this README, with the date of its fetch.
    pub fn to_fact(&self, fetched: Date) -> ReadmeFact {
        ReadmeFact {
            text: self.text.clone(),
            path: self.path.clone(),
            branch: self.branch.clone(),
            truncated: self.truncated,
            fetched,
            source: self.source.clone(),
        }
    }
}

/// The text of `bytes` up to [`MAX_README`], and whether the cut removes anything.
///
/// A cut inside a character drops that character. Invalid UTF-8 elsewhere becomes the replacement
/// character. A leading byte order mark goes.
fn capped_text(bytes: &[u8]) -> (String, bool) {
    let truncated = bytes.len() > MAX_README;
    let kept = bytes.get(..MAX_README).unwrap_or(bytes);
    let end = match std::str::from_utf8(kept) {
        Err(error) if error.error_len().is_none() => error.valid_up_to(),
        _ => kept.len(),
    };
    let text = String::from_utf8_lossy(kept.get(..end).unwrap_or(kept));
    (text.trim_start_matches('\u{feff}').to_owned(), truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_file_stays_whole() {
        assert_eq!(capped_text(b"# A"), ("# A".to_owned(), false));
    }

    #[test]
    fn a_long_file_stops_at_the_cap_and_is_marked() {
        let (text, truncated) = capped_text(&vec![b'a'; MAX_README + 1]);
        assert!(truncated);
        assert_eq!(text.len(), MAX_README);
    }

    #[test]
    fn a_cut_inside_a_character_drops_the_character() {
        let mut bytes = vec![b'a'; MAX_README - 1];
        bytes.extend_from_slice("é".as_bytes());
        let (text, truncated) = capped_text(&bytes);
        assert!(truncated);
        assert_eq!(text.len(), MAX_README - 1);
        assert!(text.bytes().all(|byte| byte == b'a'));
    }

    #[test]
    fn a_byte_order_mark_goes() {
        assert_eq!(capped_text("\u{feff}# A".as_bytes()).0, "# A");
    }

    #[test]
    fn invalid_bytes_become_the_replacement_character() {
        assert_eq!(capped_text(b"a\xffb").0, "a\u{fffd}b");
    }
}
