//! What the About section of an entry page reads.

use catalogue_core::ReadmeFact;

use super::html::{ReadmeHtml, render};
use super::parse::parse;
use super::resolve::ReadmeBase;
use super::tree::Node;
use crate::href::SafeHref;
use crate::pages::shared::date_text;

/// The characters of text the first part of a README holds, about.
const HEAD_TEXT: usize = 1200;

/// The data of the About section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadmeView {
    /// The blocks the page shows first.
    pub head: ReadmeHtml,
    /// The blocks behind the disclosure. `None` when the head holds every block.
    pub rest: Option<ReadmeHtml>,
    /// The page of the file on its forge.
    pub source: SafeHref,
    /// The fetch date as an ISO date.
    pub fetched: String,
    /// The fetch date as a person reads it.
    pub fetched_text: String,
    /// Whether the file is longer than the fetch cap.
    pub truncated: bool,
}

impl ReadmeView {
    /// The section of `readme` on the page of the entry named `name`.
    ///
    /// Returns `None` when the source is on no forge the renderer knows, or nothing survives the
    /// sanitizing, so the page leaves the section out.
    pub(crate) fn of(readme: &ReadmeFact, name: &str) -> Option<Self> {
        let base = ReadmeBase::of(&readme.source, &readme.path, &readme.branch)?;
        let mut blocks = parse(&readme.text, &base);
        drop_title(&mut blocks, name);
        if blocks.is_empty() {
            return None;
        }
        let (head, rest) = blocks.split_at(head_len(&blocks));
        let fetched = readme.fetched.to_string();
        Some(Self {
            head: render(head),
            rest: (!rest.is_empty()).then(|| render(rest)),
            source: SafeHref::from(&readme.source),
            fetched_text: date_text(&fetched).unwrap_or_else(|| fetched.clone()),
            fetched,
            truncated: readme.truncated,
        })
    }
}

/// Removes the first heading when it only repeats the project name.
fn drop_title(blocks: &mut Vec<Node>, name: &str) {
    let Some(at) = blocks.iter().position(Node::is_heading) else {
        return;
    };
    let wanted = letters_and_digits(name);
    if !wanted.is_empty()
        && blocks
            .get(at)
            .is_some_and(|heading| letters_and_digits(&heading.text()) == wanted)
    {
        blocks.remove(at);
    }
}

/// The lowercase letters and digits of `text`.
fn letters_and_digits(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// How many of `blocks` the head holds: at least one, and no more than about [`HEAD_TEXT`]
/// characters of text.
///
/// A heading never ends the head when another block can follow it, because it belongs to the block
/// under it.
fn head_len(blocks: &[Node]) -> usize {
    let mut text = 0;
    let mut len = 0;
    for block in blocks {
        let chars = block.text().chars().count();
        if len > 0 && text + chars > HEAD_TEXT {
            break;
        }
        text += chars;
        len += 1;
    }
    let ends_on_heading = |len: usize| {
        len < blocks.len() && len > 0 && blocks.get(len - 1).is_some_and(Node::is_heading)
    };
    if len == 1 && ends_on_heading(len) {
        return 2;
    }
    while len > 1 && ends_on_heading(len) {
        len -= 1;
    }
    len
}

#[cfg(test)]
mod tests {
    use catalogue_core::Date;
    use url::Url;

    use super::*;

    fn fact(text: &str, truncated: bool) -> ReadmeFact {
        ReadmeFact {
            text: text.to_owned(),
            path: "README.md".to_owned(),
            branch: "main".to_owned(),
            truncated,
            fetched: Date::new(2026, 11, 3).expect("date"),
            source: Url::parse("https://github.com/o/r/blob/main/README.md").expect("url"),
        }
    }

    fn view(text: &str) -> Option<ReadmeView> {
        ReadmeView::of(&fact(text, false), "My-Project")
    }

    fn paragraph(chars: usize) -> String {
        "a".repeat(chars)
    }

    #[test]
    fn a_short_readme_has_no_rest() {
        let found = view("One.\n\nTwo.").expect("view");
        assert_eq!(found.head.to_string(), "<p>One.</p><p>Two.</p>");
        assert_eq!(found.rest, None);
        assert_eq!(found.fetched_text, "3 Nov 2026");
    }

    #[test]
    fn a_first_heading_that_repeats_the_name_goes() {
        for title in ["# My Project", "# my-project!", "## MY_PROJECT"] {
            let found = view(&format!("{title}\n\nBody.")).expect("view");
            assert_eq!(found.head.to_string(), "<p>Body.</p>", "{title}");
        }
    }

    #[test]
    fn another_first_heading_stays_and_only_the_first_heading_counts() {
        let found = view("# Install\n\n# My Project\n\nBody.").expect("view");
        assert_eq!(
            found.head.to_string(),
            "<h3>Install</h3><h3>My Project</h3><p>Body.</p>"
        );
    }

    #[test]
    fn a_readme_with_nothing_left_has_no_view() {
        assert_eq!(view("# My Project"), None);
        assert_eq!(view("<div align=\"center\"><img src=\"x\"></div>"), None);
        assert_eq!(
            view("[![a](https://x.example/a.svg)](https://x.example)"),
            None
        );
    }

    #[test]
    fn the_head_stops_at_a_block_boundary_and_always_holds_one_block() {
        let long = paragraph(5000);
        let found = view(&format!("{long}\n\nTwo.")).expect("view");
        assert_eq!(found.head.to_string(), format!("<p>{long}</p>"));
        assert_eq!(found.rest.expect("rest").to_string(), "<p>Two.</p>");

        let first = paragraph(700);
        let second = paragraph(400);
        let third = paragraph(400);
        let found = view(&format!("{first}\n\n{second}\n\n{third}")).expect("view");
        assert_eq!(
            found.head.to_string(),
            format!("<p>{first}</p><p>{second}</p>")
        );
        assert_eq!(
            found.rest.expect("rest").to_string(),
            format!("<p>{third}</p>")
        );
    }

    #[test]
    fn a_heading_moves_to_the_rest_with_the_block_under_it() {
        let first = paragraph(1100);
        let second = paragraph(400);
        let found = view(&format!("{first}\n\n# Next\n\n{second}")).expect("view");
        assert_eq!(found.head.to_string(), format!("<p>{first}</p>"));
        assert_eq!(
            found.rest.expect("rest").to_string(),
            format!("<h3>Next</h3><p>{second}</p>")
        );
    }

    #[test]
    fn a_lone_heading_keeps_the_block_under_it() {
        let body = paragraph(5000);
        let found = view(&format!("# Start\n\n{body}\n\nEnd.")).expect("view");
        assert_eq!(
            found.head.to_string(),
            format!("<h3>Start</h3><p>{body}</p>")
        );
    }

    #[test]
    fn a_source_on_another_host_has_no_view() {
        let mut readme = fact("Body.", false);
        readme.source = Url::parse("https://example.org/o/r/README.md").expect("url");
        assert_eq!(ReadmeView::of(&readme, "x"), None);
    }
}
