//! The tree of allowed elements a README becomes.

use crate::href::SafeHref;

/// An element the renderer can write. No other element reaches a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Element {
    H3,
    H4,
    H5,
    P,
    Ul,
    Ol,
    Li,
    Blockquote,
    Pre,
    Code,
    Em,
    Strong,
    Del,
    Table,
    Thead,
    Tbody,
    Tr,
    Th,
    Td,
}

impl Element {
    /// The tag name.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::H3 => "h3",
            Self::H4 => "h4",
            Self::H5 => "h5",
            Self::P => "p",
            Self::Ul => "ul",
            Self::Ol => "ol",
            Self::Li => "li",
            Self::Blockquote => "blockquote",
            Self::Pre => "pre",
            Self::Code => "code",
            Self::Em => "em",
            Self::Strong => "strong",
            Self::Del => "del",
            Self::Table => "table",
            Self::Thead => "thead",
            Self::Tbody => "tbody",
            Self::Tr => "tr",
            Self::Th => "th",
            Self::Td => "td",
        }
    }

    /// Whether the element stands as a block, so it never sits inside a paragraph.
    pub(super) fn is_block(self) -> bool {
        !matches!(self, Self::Code | Self::Em | Self::Strong | Self::Del)
    }

    /// Whether an element with no content stays, because a table needs its shape.
    fn stays_empty(self) -> bool {
        matches!(
            self,
            Self::Thead | Self::Tbody | Self::Tr | Self::Th | Self::Td
        )
    }
}

/// One piece of a README.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Node {
    /// Text, not yet escaped.
    Text(String),
    /// A hard line break.
    Break,
    /// A horizontal rule.
    Rule,
    /// An allowed element.
    Element {
        element: Element,
        children: Vec<Node>,
    },
    /// A link whose target passed [`super::resolve::resolve`].
    Link {
        href: SafeHref,
        title: String,
        children: Vec<Node>,
    },
    /// An image whose source passed [`super::resolve::resolve`].
    Image {
        src: SafeHref,
        alt: String,
        title: String,
        /// The `width` attribute: a positive integer of at most 4 digits.
        width: Option<u16>,
        /// The `height` attribute: a positive integer of at most 4 digits.
        height: Option<u16>,
    },
}

impl Node {
    /// Whether the node is a block or holds one, so it never sits inside a paragraph.
    pub(super) fn holds_block(&self) -> bool {
        match self {
            Self::Rule => true,
            Self::Element { element, children } => {
                element.is_block() || children.iter().any(Self::holds_block)
            }
            Self::Link { children, .. } => children.iter().any(Self::holds_block),
            Self::Text(_) | Self::Break | Self::Image { .. } => false,
        }
    }

    /// Whether the node shows anything: text beyond whitespace, an image or a rule.
    pub(super) fn holds_content(&self) -> bool {
        match self {
            Self::Text(text) => !text.trim().is_empty(),
            Self::Break => false,
            Self::Rule | Self::Image { .. } => true,
            Self::Element { children, .. } | Self::Link { children, .. } => {
                children.iter().any(Self::holds_content)
            }
        }
    }

    /// Adds the text this node shows to `out`. An image shows none.
    pub(super) fn push_text(&self, out: &mut String) {
        match self {
            Self::Text(text) => out.push_str(text),
            Self::Break => out.push(' '),
            Self::Rule | Self::Image { .. } => {}
            Self::Element { children, .. } | Self::Link { children, .. } => {
                for child in children {
                    child.push_text(out);
                }
            }
        }
    }

    /// The text this node shows.
    pub(super) fn text(&self) -> String {
        let mut out = String::new();
        self.push_text(&mut out);
        out
    }

    /// Whether the node is a heading.
    pub(super) fn is_heading(&self) -> bool {
        matches!(
            self,
            Self::Element {
                element: Element::H3 | Element::H4 | Element::H5,
                ..
            }
        )
    }
}

/// `nodes` without the links, paragraphs, list items and lists the sanitizing left empty.
pub(super) fn prune(nodes: Vec<Node>) -> Vec<Node> {
    nodes
        .into_iter()
        .filter_map(|node| match node {
            Node::Element { element, children } => {
                let mut children = prune(children);
                if !element.stays_empty() && !children.iter().any(Node::holds_content) {
                    return None;
                }
                if element == Element::P {
                    if only_separators(&children) {
                        return None;
                    }
                    trim_start(&mut children);
                    trim_end(&mut children);
                }
                Some(Node::Element { element, children })
            }
            Node::Link {
                href,
                title,
                children,
            } => {
                let children = prune(children);
                children
                    .iter()
                    .any(Node::holds_content)
                    .then_some(Node::Link {
                        href,
                        title,
                        children,
                    })
            }
            other => Some(other),
        })
        .collect()
}

/// Removes the blank text and the breaks that a removed image left at the start of a paragraph.
fn trim_start(children: &mut Vec<Node>) {
    let blank = children
        .iter()
        .take_while(|node| !node.holds_content() && matches!(node, Node::Text(_) | Node::Break))
        .count();
    children.drain(..blank);
    if let Some(Node::Text(text)) = children.first_mut() {
        text.drain(..text.len() - text.trim_start().len());
    }
}

/// Removes the blank text and the breaks at the end of a paragraph.
fn trim_end(children: &mut Vec<Node>) {
    while children
        .last()
        .is_some_and(|node| !node.holds_content() && matches!(node, Node::Text(_) | Node::Break))
    {
        children.pop();
    }
    if let Some(Node::Text(text)) = children.last_mut() {
        text.truncate(text.trim_end().len());
    }
}

/// Whether `children` are only breaks and text with no letter or digit: the separators that stood
/// between removed badges.
fn only_separators(children: &[Node]) -> bool {
    children.iter().all(|node| match node {
        Node::Text(text) => !text.chars().any(char::is_alphanumeric),
        Node::Break => true,
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(text: &str) -> Node {
        Node::Text(text.to_owned())
    }

    fn element(element: Element, children: Vec<Node>) -> Node {
        Node::Element { element, children }
    }

    fn link(children: Vec<Node>) -> Node {
        Node::Link {
            href: SafeHref::new("https://example.org/"),
            title: String::new(),
            children,
        }
    }

    #[test]
    fn a_link_paragraph_and_list_with_nothing_left_go() {
        let nodes = vec![
            element(Element::P, vec![link(Vec::new()), text(" "), link(vec![])]),
            element(
                Element::Ul,
                vec![element(Element::Li, vec![link(Vec::new())])],
            ),
        ];
        assert_eq!(prune(nodes), Vec::new());
    }

    #[test]
    fn a_paragraph_loses_the_blank_start_a_removed_badge_left() {
        let nodes = vec![element(
            Element::P,
            vec![text(" "), Node::Break, text(" kept")],
        )];
        assert_eq!(prune(nodes), [element(Element::P, vec![text("kept")])]);
    }

    #[test]
    fn a_table_keeps_its_empty_cells_and_an_empty_table_goes() {
        let row = |cell: &str| {
            element(
                Element::Tr,
                vec![
                    element(Element::Td, vec![text(cell)]),
                    element(Element::Td, Vec::new()),
                ],
            )
        };
        let full = element(Element::Table, vec![row("a")]);
        assert_eq!(prune(vec![full.clone()]), [full]);
        assert_eq!(
            prune(vec![element(Element::Table, vec![row(" ")])]),
            Vec::new()
        );
    }

    #[test]
    fn a_rule_and_an_image_stay() {
        let image = Node::Image {
            src: SafeHref::new("https://example.org/a.png"),
            alt: String::new(),
            title: String::new(),
            width: None,
            height: None,
        };
        let nodes = vec![Node::Rule, element(Element::P, vec![image])];
        assert_eq!(prune(nodes.clone()), nodes);
    }

    #[test]
    fn the_text_of_a_node_leaves_images_out() {
        let node = element(
            Element::P,
            vec![
                text("a "),
                Node::Image {
                    src: SafeHref::new("https://example.org/a.png"),
                    alt: "hidden".to_owned(),
                    title: String::new(),
                    width: None,
                    height: None,
                },
                link(vec![text("b")]),
            ],
        );
        assert_eq!(node.text(), "a b");
    }
}
