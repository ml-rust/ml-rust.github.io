//! The collapsed comment section that shows the link check results.

use url::Url;

use super::verdict::{Checked, Outcome, Verdict};
use crate::check::fenced;

/// The summary line of the collapsed section.
pub const SECTION_SUMMARY: &str = "Automated link check (LLM, advisory)";

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
    let mut parts = vec![
        format!("<details>\n<summary>{SECTION_SUMMARY}</summary>"),
        NOTICE.to_owned(),
    ];
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
    parts.push("</details>".to_owned());
    let mut out = parts.join("\n\n");
    out.push('\n');
    out
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
