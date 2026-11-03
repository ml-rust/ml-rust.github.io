//! The collapsed comment section that shows the link check results.

use url::Url;

use super::verdict::{Checked, Outcome, Verdict};
use crate::check::fenced;

/// The summary line of the collapsed section.
pub const SECTION_SUMMARY: &str = "Automated link check (LLM, advisory)";

/// The line that closes the section.
const CLOSE: &str = "</details>";

/// The gap between two parts of the section.
const SEPARATOR: &str = "\n\n";

/// The last part of a section cut to fit a comment.
const SECTION_CUT: &str = "**Link check cut.** The other results do not fit in a GitHub comment.";

/// The fixed notice under the summary.
const NOTICE: &str = "This check is advisory. A language model reads each page, and it can be wrong. \
No label, tier or approval reads this section.";

/// The result for one evidence link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkResult {
    /// The evidence link.
    pub url: Url,
    /// What the check found.
    pub outcome: Outcome,
}

/// The section as GitHub markdown, in a collapsed `<details>` block.
///
/// Each link renders as inline code, so it never becomes a link. Each label
/// is fixed bot text. A quote renders in a fence longer than any backtick run
/// inside it, with control characters removed. No model text appears outside a fence.
pub fn triage_section(results: &[LinkResult]) -> String {
    let mut parts = vec![open(), NOTICE.to_owned()];
    if results.is_empty() {
        parts.push("The issue lists no evidence link.".to_owned());
    }
    for (index, result) in results.iter().enumerate() {
        let number = index.saturating_add(1);
        let link = inline_code(result.url.as_str());
        let (label, quote) = describe(&result.outcome);
        parts.push(format!("**Link {number}:** {link}: {label}"));
        if let Some(quote) = quote {
            parts.push(fenced("text", &strip_controls(quote)));
        }
    }
    parts.push(CLOSE.to_owned());
    let mut out = parts.join(SEPARATOR);
    out.push('\n');
    out
}

/// The opening part of the section: the `<details>` tag and the summary line.
fn open() -> String {
    format!("<details>\n<summary>{SECTION_SUMMARY}</summary>")
}

/// The text after the last kept part of a cut section.
fn cut_tail() -> String {
    format!("{SEPARATOR}{SECTION_CUT}{SEPARATOR}{CLOSE}\n")
}

/// The length of the shortest cut section: the opening part, the cut notice and the close.
pub(crate) fn section_floor() -> usize {
    open().len().saturating_add(cut_tail().len())
}

/// `section`, a text [`triage_section`] gave, in at most `max` bytes.
///
/// A section that fits comes out unchanged. A longer one keeps its parts in
/// order while they fit, then ends with a fixed notice and the close. A part
/// is never cut, so no fence opens without its close. A `max` below the
/// shortest cut section gives the empty string.
pub fn section_within(section: &str, max: usize) -> String {
    if section.len() <= max {
        return section.to_owned();
    }
    let open = open();
    let tail = cut_tail();
    if max < open.len().saturating_add(tail.len()) {
        return String::new();
    }
    let parts = section
        .strip_prefix(open.as_str())
        .and_then(|rest| rest.strip_prefix(SEPARATOR))
        .and_then(|rest| rest.strip_suffix(&format!("{SEPARATOR}{CLOSE}\n")))
        .unwrap_or_default();
    let mut out = open;
    for part in parts.split(SEPARATOR).filter(|part| !part.is_empty()) {
        let need = out
            .len()
            .saturating_add(SEPARATOR.len())
            .saturating_add(part.len())
            .saturating_add(tail.len());
        if need > max {
            break;
        }
        out.push_str(SEPARATOR);
        out.push_str(part);
    }
    out.push_str(&tail);
    out
}

/// The section at the end of bot comment `text`, or `None` when it ends without one.
///
/// The section is always the last part of a bot comment. It starts at the
/// last opening part and runs to the end of the text. No issue text or model
/// text can hold the opening part, since a quote or a link in the section
/// holds no line break. A `\r\n` line end reads as `\n`.
pub fn section_of_comment(text: &str) -> Option<String> {
    let text = text.replace("\r\n", "\n");
    let start = text.rfind(&open())?;
    let before = text.get(..start)?;
    let section = text.get(start..)?;
    let close = format!("{SEPARATOR}{CLOSE}\n");
    ((before.is_empty() || before.ends_with(SEPARATOR)) && section.ends_with(&close))
        .then(|| section.to_owned())
}

/// The fixed label of `outcome` and its quote, when it has one.
fn describe(outcome: &Outcome) -> (String, Option<&str>) {
    match outcome {
        Outcome::Checked(Checked { verdict, quote }) => {
            let label = match verdict {
                Verdict::Supports => "supports the claim.",
                Verdict::DoesNotSupport => "does not support the claim.",
                Verdict::Unclear => "unclear.",
            };
            let label = if quote.is_some() {
                format!("{label} Quote from the page:")
            } else {
                label.to_owned()
            };
            (label, quote.as_deref())
        }
        Outcome::Unreachable(reason) => (
            format!(
                "not checked. The page could not be read: {}.",
                inline_code(reason)
            ),
            None,
        ),
        Outcome::NotChecked => (
            "not checked. It comes after the link limit.".to_owned(),
            None,
        ),
        Outcome::NoAnswer => (
            "not checked. No model answered. The bot retries later.".to_owned(),
            None,
        ),
    }
}

/// `text` as an inline code span whose backtick run is longer than any inside.
///
/// Control characters become spaces. A space pads the content when it starts
/// or ends with a backtick or a space, so the span shows it unchanged.
fn inline_code(text: &str) -> String {
    let text: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let longest = text
        .split(|c| c != '`')
        .map(str::len)
        .max()
        .unwrap_or_default();
    let fence = "`".repeat(longest.saturating_add(1));
    let pad = if text.starts_with(['`', ' ']) || text.ends_with(['`', ' ']) {
        " "
    } else {
        ""
    };
    format!("{fence}{pad}{text}{pad}{fence}")
}

/// `text` without control characters, newlines included.
fn strip_controls(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).expect(text)
    }

    fn checked(verdict: Verdict, quote: Option<&str>) -> Outcome {
        Outcome::Checked(Checked {
            verdict,
            quote: quote.map(str::to_owned),
        })
    }

    #[test]
    fn the_section_is_collapsed_and_lists_each_link() {
        let text = triage_section(&[
            LinkResult {
                url: url("https://example.com/a"),
                outcome: checked(Verdict::Supports, Some("used by Acme")),
            },
            LinkResult {
                url: url("https://example.com/b"),
                outcome: Outcome::NotChecked,
            },
        ]);
        assert_eq!(
            text,
            format!(
                "<details>\n<summary>Automated link check (LLM, advisory)</summary>\n\n{NOTICE}\n\n\
                 **Link 1:** `https://example.com/a`: supports the claim. Quote from the page:\n\n\
                 ```text\nused by Acme\n```\n\n\
                 **Link 2:** `https://example.com/b`: not checked. It comes after the link limit.\n\n\
                 </details>\n"
            )
        );
    }

    #[test]
    fn every_outcome_has_a_fixed_label() {
        let text = triage_section(&[
            LinkResult {
                url: url("https://example.com/a"),
                outcome: checked(Verdict::Unclear, None),
            },
            LinkResult {
                url: url("https://example.com/b"),
                outcome: checked(Verdict::DoesNotSupport, Some("q")),
            },
            LinkResult {
                url: url("https://example.com/c"),
                outcome: Outcome::Unreachable("the fetch service answered 429".to_owned()),
            },
            LinkResult {
                url: url("https://example.com/d"),
                outcome: Outcome::NoAnswer,
            },
        ]);
        assert!(text.contains("`https://example.com/a`: unclear.\n"));
        assert!(text.contains("`https://example.com/b`: does not support the claim. Quote"));
        assert!(text.contains(
            "`https://example.com/c`: not checked. The page could not be read: `the fetch service answered 429`."
        ));
        assert!(text.contains("`https://example.com/d`: not checked. No model answered."));
        assert!(triage_section(&[]).contains("The issue lists no evidence link."));
    }

    #[test]
    fn a_fence_breaking_quote_stays_inside_its_fence() {
        let quote = "x ```` </details> @maintainer <!-- [a](https://evil.example)\n# Approved";
        let text = triage_section(&[LinkResult {
            url: url("https://example.com/a"),
            outcome: checked(Verdict::Supports, Some(quote)),
        }]);
        let fence = "`````text\n";
        let start = text.find(fence).expect("fence") + fence.len();
        let end = text[start..].find("\n`````").expect("closing fence") + start;
        let inside = &text[start..end];
        assert_eq!(
            inside,
            "x ```` </details> @maintainer <!-- [a](https://evil.example)# Approved"
        );
        let outside = format!("{}{}", &text[..start], &text[end..]);
        for needle in ["@maintainer", "<!--", "evil.example", "Approved"] {
            assert!(!outside.contains(needle), "{needle} outside the fence");
        }
        assert_eq!(text.matches("</details>").count(), 2);
        assert!(text.ends_with("`````\n\n</details>\n"));
    }

    fn long_section() -> String {
        let results: Vec<LinkResult> = (0..40)
            .map(|index| LinkResult {
                url: url(&format!("https://example.com/{index}")),
                outcome: checked(Verdict::Supports, Some(&"quote ```` x ".repeat(20))),
            })
            .collect();
        triage_section(&results)
    }

    #[test]
    fn a_section_that_fits_is_unchanged() {
        let text = long_section();
        assert_eq!(section_within(&text, text.len()), text);
    }

    #[test]
    fn a_long_section_is_cut_between_parts() {
        let text = long_section();
        let max = text.len() / 3;
        let cut = section_within(&text, max);
        assert!(cut.len() <= max, "{} > {max}", cut.len());
        assert!(cut.starts_with(&open()));
        assert!(
            cut.ends_with(&format!("{SECTION_CUT}\n\n</details>\n")),
            "{cut}"
        );
        assert!(cut.contains("**Link 1:**"));
        assert!(!cut.contains("**Link 40:**"));
        for part in cut.split(SEPARATOR) {
            assert!(
                text.contains(part) || part.starts_with(SECTION_CUT),
                "{part}"
            );
        }
        let opens = cut.lines().filter(|line| *line == "`````text").count();
        let closes = cut.lines().filter(|line| *line == "`````").count();
        assert!(opens > 0);
        assert_eq!(opens, closes);
        assert_eq!(
            section_within(&text, section_floor()).len(),
            section_floor()
        );
        assert_eq!(section_within(&text, section_floor() - 1), "");
        assert_eq!(section_of_comment(&format!("x\n\n{cut}")), Some(cut));
    }

    #[test]
    fn the_section_is_found_only_at_the_end_of_a_comment() {
        let section = triage_section(&[LinkResult {
            url: url("https://example.com/a"),
            outcome: checked(Verdict::Supports, Some("q")),
        }]);
        let comment = format!("<!-- m -->\n\n## Catalogue check\n\n{section}");
        assert_eq!(section_of_comment(&comment), Some(section.clone()));
        assert_eq!(
            section_of_comment(&comment.replace('\n', "\r\n")),
            Some(section.clone())
        );
        assert_eq!(section_of_comment("## Catalogue check\n"), None);
        assert_eq!(section_of_comment(&format!("{comment}more\n")), None);
        let forged = format!(
            "## Catalogue check\n\n```text\n{}\n```\n",
            section.trim_end()
        );
        assert_eq!(section_of_comment(&forged), None);
        let glued = format!("x{section}");
        assert_eq!(section_of_comment(&glued), None);
    }

    #[test]
    fn links_with_backticks_stay_in_one_code_span() {
        let link = url("https://example.com/?q=a``b");
        assert_eq!(link.as_str(), "https://example.com/?q=a``b");
        assert_eq!(
            inline_code(link.as_str()),
            "```https://example.com/?q=a``b```"
        );
        assert_eq!(inline_code("`x"), "`` `x ``");
        assert_eq!(inline_code("a\nb"), "`a b`");
    }
}
