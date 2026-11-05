//! The parsed document of a README: Markdown to HTML, then an HTML parser over the result.

use ego_tree::iter::Edge;
use pulldown_cmark::html::push_html;
use pulldown_cmark::{Options, Parser};
use scraper::node::Element;
use scraper::{Html, Node};

/// The elements that drop with everything they hold.
const DROPPED: [&str; 22] = [
    "script", "style", "template", "noscript", "iframe", "object", "embed", "svg", "math",
    "textarea", "select", "title", "head", "link", "meta", "base", "form", "button", "input",
    "audio", "video", "canvas",
];

/// One step of the walk over a document.
pub(super) enum Step<'a> {
    /// An element opens.
    Open(&'a Element),
    /// The element opened last and not yet closed closes.
    Close,
    /// A text run.
    Text(&'a str),
}

/// The document of `markdown`. Unclosed raw tags close the way a browser closes them.
///
/// GFM tables and strikethrough are on. Footnotes, math, task lists and heading attributes are off.
pub(super) fn read_dom(markdown: &str) -> Html {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut html = String::with_capacity(markdown.len());
    push_html(&mut html, Parser::new_ext(markdown, options));
    Html::parse_fragment(&html)
}

/// Whether the element `name` drops with everything it holds.
pub(super) fn is_dropped(name: &str) -> bool {
    DROPPED.contains(&name)
}

/// The steps of `dom`, in document order and without recursion.
///
/// An element `hides` names leaves out with everything it holds. Comments, doctypes and processing
/// instructions give no step.
pub(super) fn visible(dom: &Html, hides: fn(&str) -> bool) -> impl Iterator<Item = Step<'_>> {
    let mut hidden: usize = 0;
    dom.tree
        .root()
        .traverse()
        .filter_map(move |edge| match edge {
            Edge::Open(node) => match node.value() {
                Node::Element(element) if hidden > 0 || hides(element.name()) => {
                    hidden += 1;
                    None
                }
                Node::Element(element) => Some(Step::Open(element)),
                Node::Text(text) if hidden == 0 => Some(Step::Text(text)),
                _ => None,
            },
            Edge::Close(node) if node.value().is_element() => {
                if hidden > 0 {
                    hidden -= 1;
                    None
                } else {
                    Some(Step::Close)
                }
            }
            Edge::Close(_) => None,
        })
}
