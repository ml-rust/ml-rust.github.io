//! Turns the Markdown events of a README into the tree of allowed elements.

use pulldown_cmark::{CowStr, Event, HeadingLevel, Tag, TagEnd};

use super::events::visible_events;
use super::resolve::{ReadmeBase, resolve};
use super::tree::{Element, Node, prune};
use crate::href::SafeHref;

/// The deepest nesting kept. Deeper tags drop and keep their content, so hostile nesting cannot
/// overflow the stack of the code that walks the tree.
const MAX_DEPTH: usize = 32;

/// What an open tag becomes when it closes.
#[derive(Debug)]
enum Kind {
    /// Its children join the parent: a tag the renderer does not write, or a refused link.
    Plain,
    /// An allowed element.
    Element(Element),
    /// A link.
    Link { href: SafeHref, title: String },
    /// An image. The children are its alt text. A refused source drops it whole.
    Image {
        src: Option<SafeHref>,
        title: String,
    },
}

/// One open tag.
#[derive(Debug)]
struct Frame {
    kind: Kind,
    /// Whether the tag sits past [`MAX_DEPTH`], so only its children stay.
    flat: bool,
    children: Vec<Node>,
}

/// The tree under construction.
struct Builder<'a> {
    base: &'a ReadmeBase,
    stack: Vec<Frame>,
    root: Vec<Node>,
}

impl Builder<'_> {
    fn open(&mut self, kind: Kind) {
        let flat = self.stack.len() >= MAX_DEPTH && !matches!(kind, Kind::Image { .. });
        self.stack.push(Frame {
            kind,
            flat,
            children: Vec::new(),
        });
    }

    fn open_element(&mut self, element: Element) {
        self.open(Kind::Element(element));
    }

    fn push(&mut self, node: Node) {
        match self.stack.last_mut() {
            Some(frame) => frame.children.push(node),
            None => self.root.push(node),
        }
    }

    fn close(&mut self) {
        let Some(frame) = self.stack.pop() else {
            return;
        };
        let Frame {
            kind,
            flat,
            children,
        } = frame;
        let node = match kind {
            Kind::Image { src, title } => {
                let Some(src) = src else {
                    return;
                };
                Node::Image {
                    src,
                    alt: text_of(&children),
                    title,
                }
            }
            Kind::Element(element) if !flat => Node::Element { element, children },
            Kind::Link { href, title } if !flat => Node::Link {
                href,
                title,
                children,
            },
            Kind::Plain | Kind::Element(_) | Kind::Link { .. } => {
                for child in children {
                    self.push(child);
                }
                return;
            }
        };
        self.push(node);
    }

    fn top_is(&self, element: Element) -> bool {
        matches!(
            self.stack.last(),
            Some(Frame { kind: Kind::Element(open), .. }) if *open == element
        )
    }

    /// Whether a table head stands open, so a cell is `th`.
    fn in_head(&self) -> bool {
        self.stack
            .iter()
            .any(|frame| matches!(frame.kind, Kind::Element(Element::Thead)))
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.open_element(Element::P),
            Tag::Heading { level, .. } => self.open_element(heading(level)),
            Tag::BlockQuote(_) => self.open_element(Element::Blockquote),
            Tag::CodeBlock(_) => {
                self.open_element(Element::Pre);
                self.open_element(Element::Code);
            }
            Tag::List(Some(_)) => self.open_element(Element::Ol),
            Tag::List(None) => self.open_element(Element::Ul),
            Tag::Item => self.open_element(Element::Li),
            Tag::Table(_) => self.open_element(Element::Table),
            Tag::TableHead => {
                self.open_element(Element::Thead);
                self.open_element(Element::Tr);
            }
            Tag::TableRow => {
                if !self.top_is(Element::Tbody) {
                    self.open_element(Element::Tbody);
                }
                self.open_element(Element::Tr);
            }
            Tag::TableCell => {
                let cell = if self.in_head() {
                    Element::Th
                } else {
                    Element::Td
                };
                self.open_element(cell);
            }
            Tag::Emphasis => self.open_element(Element::Em),
            Tag::Strong => self.open_element(Element::Strong),
            Tag::Strikethrough => self.open_element(Element::Del),
            Tag::Link {
                dest_url, title, ..
            } => match resolve(&dest_url, self.base) {
                Some(found) => self.open(Kind::Link {
                    href: found.link,
                    title: title.into_string(),
                }),
                None => self.open(Kind::Plain),
            },
            Tag::Image {
                dest_url, title, ..
            } => self.open(Kind::Image {
                src: resolve(&dest_url, self.base).and_then(|found| found.image),
                title: title.into_string(),
            }),
            _ => self.open(Kind::Plain),
        }
    }

    fn end(&mut self, end: TagEnd) {
        match end {
            TagEnd::CodeBlock | TagEnd::TableHead => {
                self.close();
                self.close();
            }
            TagEnd::Table => {
                if self.top_is(Element::Tbody) {
                    self.close();
                }
                self.close();
            }
            _ => self.close(),
        }
    }

    fn text(&mut self, text: CowStr<'_>) {
        self.push(Node::Text(text.into_string()));
    }
}

/// The element of a heading: one level under the page's own About heading, and never deeper than
/// `h5`.
fn heading(level: HeadingLevel) -> Element {
    match level {
        HeadingLevel::H1 => Element::H3,
        HeadingLevel::H2 => Element::H4,
        _ => Element::H5,
    }
}

/// The text of `nodes`, for an alt attribute.
fn text_of(nodes: &[Node]) -> String {
    let mut out = String::new();
    for node in nodes {
        node.push_text(&mut out);
    }
    out
}

/// The top-level blocks of `markdown`, with every URL resolved against `base`.
///
/// Raw HTML, refused URLs and the empty blocks they leave are gone. Nothing from the input reaches
/// the tree except text and the URLs [`resolve`] passes.
pub(super) fn parse(markdown: &str, base: &ReadmeBase) -> Vec<Node> {
    let mut builder = Builder {
        base,
        stack: Vec::new(),
        root: Vec::new(),
    };
    for event in visible_events(markdown) {
        match event {
            Event::Start(tag) => builder.start(tag),
            Event::End(end) => builder.end(end),
            Event::Text(text) => builder.text(text),
            Event::Code(text) => {
                builder.open_element(Element::Code);
                builder.text(text);
                builder.close();
            }
            Event::SoftBreak => builder.push(Node::Text("\n".to_owned())),
            Event::HardBreak => builder.push(Node::Break),
            Event::Rule => builder.push(Node::Rule),
            _ => {}
        }
    }
    while !builder.stack.is_empty() {
        builder.close();
    }
    prune(builder.root)
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::*;

    fn base() -> ReadmeBase {
        let source = Url::parse("https://github.com/o/r/blob/main/README.md").expect("url");
        ReadmeBase::of(&source, "README.md", "main").expect("base")
    }

    fn element_names(nodes: &[Node], out: &mut Vec<&'static str>) {
        for node in nodes {
            match node {
                Node::Element { element, children } => {
                    out.push(element.name());
                    element_names(children, out);
                }
                Node::Link { children, .. } => element_names(children, out),
                _ => {}
            }
        }
    }

    fn names(markdown: &str) -> Vec<&'static str> {
        let mut out = Vec::new();
        element_names(&parse(markdown, &base()), &mut out);
        out
    }

    #[test]
    fn headings_shift_under_the_about_heading() {
        assert_eq!(
            names("# a\n\n## b\n\n### c\n\n###### d"),
            ["h3", "h4", "h5", "h5"]
        );
    }

    #[test]
    fn a_table_gets_a_head_and_a_body() {
        assert_eq!(
            names("| a |\n| - |\n| b |\n| c |"),
            [
                "table", "thead", "tr", "th", "tbody", "tr", "td", "tr", "td"
            ]
        );
    }

    #[test]
    fn a_code_block_is_code_in_pre_and_keeps_no_language() {
        assert_eq!(names("```rust\nfn a() {}\n```"), ["pre", "code"]);
    }

    #[test]
    fn lists_quotes_and_marks_map_to_their_elements() {
        assert_eq!(
            names("- a\n\n1. *b* **c** ~~d~~ `e`\n\n> f"),
            [
                "ul",
                "li",
                "ol",
                "li",
                "em",
                "strong",
                "del",
                "code",
                "blockquote",
                "p"
            ]
        );
    }

    #[test]
    fn a_refused_link_keeps_its_text_and_a_refused_image_drops_its_alt() {
        let nodes = parse("[kept](javascript:x) ![hidden](data:x) end", &base());
        let [block] = nodes.as_slice() else {
            panic!("{nodes:?}");
        };
        assert_eq!(block.text().trim(), "kept  end");
    }

    #[test]
    fn a_badge_in_a_link_takes_the_link_and_the_paragraph_with_it() {
        let nodes = parse(
            "[![build](https://ci.example.org/b.svg)](https://ci.example.org)\n\nkept",
            &base(),
        );
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].text(), "kept");
    }

    #[test]
    fn hostile_nesting_stays_under_the_depth_limit() {
        let markdown = format!("{}a", "> ".repeat(5_000));
        let nodes = parse(&markdown, &base());
        fn depth(nodes: &[Node]) -> usize {
            nodes
                .iter()
                .map(|node| match node {
                    Node::Element { children, .. } | Node::Link { children, .. } => {
                        1 + depth(children)
                    }
                    _ => 1,
                })
                .max()
                .unwrap_or(0)
        }
        assert!(depth(&nodes) <= MAX_DEPTH + 1, "{}", depth(&nodes));
    }
}
