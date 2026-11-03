//! The README rule.

/// Characters of the README that the scan reads.
pub const README_WINDOW_CHARS: usize = 1000;

/// Phrases checked in the README window.
const README_PHRASES: [&str; 3] = ["no longer maintained", "unmaintained", "deprecated"];

/// Subjects that tie a README line to the crate itself.
const README_SUBJECTS: [&str; 3] = ["this crate", "this project", "this library"];

/// The first heading or line in the README window that marks the crate as deprecated.
pub(super) fn find(readme: &str) -> Option<&str> {
    let window = readme
        .char_indices()
        .nth(README_WINDOW_CHARS)
        .and_then(|(end, _)| readme.get(..end))
        .unwrap_or(readme);
    let lines: Vec<&str> = window.lines().collect();
    let heading = first_heading(&lines);
    if let Some(index) = heading {
        let text = lines.get(index).copied().unwrap_or_default();
        if contains_any(&text.to_lowercase(), &README_PHRASES) {
            return Some(heading_text(text));
        }
    }
    lines.iter().enumerate().find_map(|(index, line)| {
        if Some(index) == heading {
            return None;
        }
        let lower = line.to_lowercase();
        let tied =
            contains_any(&lower, &README_SUBJECTS) || contains_words(&lower, "is deprecated");
        (tied && contains_any(&lower, &README_PHRASES)).then_some(*line)
    })
}

/// Index of the first ATX, setext or `<h1>` heading line.
fn first_heading(lines: &[&str]) -> Option<usize> {
    lines.iter().enumerate().find_map(|(index, line)| {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || contains_h1(trimmed) {
            return Some(index);
        }
        let underline = lines.get(index + 1).map(|next| next.trim())?;
        let equals = underline.len() >= 2 && underline.bytes().all(|b| b == b'=');
        let dashes = underline.len() >= 3 && underline.bytes().all(|b| b == b'-');
        let setext = !trimmed.is_empty() && (equals || dashes);
        setext.then_some(index)
    })
}

/// Whether `line` holds an `<h1` tag opener, ignoring ASCII case.
fn contains_h1(line: &str) -> bool {
    line.as_bytes()
        .windows(3)
        .any(|window| window.eq_ignore_ascii_case(b"<h1"))
}

fn heading_text(line: &str) -> &str {
    line.trim().trim_start_matches('#').trim()
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

/// Whether `needle` occurs in `haystack` with no letter or digit on either side.
fn contains_words(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(start, _)| {
        let before = haystack
            .get(..start)
            .and_then(|head| head.chars().next_back());
        let after = haystack
            .get(start + needle.len()..)
            .and_then(|tail| tail.chars().next());
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

#[cfg(test)]
mod tests {
    use super::README_WINDOW_CHARS;
    use crate::deprecation::{Field, scan};

    fn readme(text: &str) -> Option<String> {
        scan(None, Some(text)).map(|notice| {
            assert_eq!(notice.field, Field::Readme);
            notice.phrase
        })
    }

    #[test]
    fn readme_first_heading_matches() {
        assert_eq!(
            readme("# foo (DEPRECATED)\n\nA parser.").as_deref(),
            Some("foo (DEPRECATED)")
        );
        assert!(readme("Foo is unmaintained\n===\n\ntext").is_some());
        assert!(readme("<h1 align=\"center\">foo: no longer maintained</h1>").is_some());
    }

    #[test]
    fn readme_line_with_subject_matches() {
        assert_eq!(
            readme("# foo\n\n> **Note:** this crate is no longer maintained.\n").as_deref(),
            Some("> **Note:** this crate is no longer maintained.")
        );
        assert!(readme("# foo\n\nThis project is unmaintained.").is_some());
        assert!(readme("# foo\n\nThis library has been deprecated.").is_some());
        assert!(readme("# foo\n\nfoo is deprecated, use bar.").is_some());
    }

    #[test]
    fn readme_line_without_subject_does_not_match() {
        assert_eq!(readme("# foo\n\nSupports deprecated APIs too."), None);
        assert_eq!(
            readme("# foo\n\n## Unmaintained deps\n\nWe list them."),
            None
        );
        assert_eq!(
            readme("# foo\n\nWe keep this deprecated path working."),
            None
        );
    }

    #[test]
    fn readme_reads_only_the_first_1000_characters() {
        let filler = "é".repeat(README_WINDOW_CHARS);
        assert_eq!(
            readme(&format!("# foo\n{filler}\nThis crate is deprecated.")),
            None
        );
        let near = "a".repeat(README_WINDOW_CHARS - 40);
        assert!(readme(&format!("# foo\n{near}\nThis crate is deprecated.")).is_some());
    }
}
