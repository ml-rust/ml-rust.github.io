//! Holds the tree of a README while it grows, one open tag at a time.

use super::tree::{Element, Node, prune};
use crate::href::SafeHref;

/// The deepest nesting kept. Deeper tags drop and keep their content, so hostile nesting cannot
/// overflow the stack of the code that walks the tree.
pub(super) const MAX_DEPTH: usize = 32;

/// What an open tag becomes when it closes.
#[derive(Debug)]
pub(super) enum Kind {
    /// Its children join the parent: a tag the renderer does not write, or a refused link.
    Plain,
    /// A block the renderer does not write. Each run of its inline children becomes a paragraph, and
    /// its block children join the parent as they are.
    Block,
    /// An allowed element.
    Element(Element),
    /// A link.
    Link { href: SafeHref, title: String },
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
#[derive(Debug, Default)]
pub(super) struct Builder {
    stack: Vec<Frame>,
    root: Vec<Node>,
    /// How many `pre` tags stand open.
    pres: usize,
}

impl Builder {
    pub(super) fn open(&mut self, kind: Kind) {
        if matches!(kind, Kind::Element(Element::Pre)) {
            self.pres += 1;
        }
        let flat = self.stack.len() >= MAX_DEPTH;
        self.stack.push(Frame {
            kind,
            flat,
            children: Vec::new(),
        });
    }

    pub(super) fn push(&mut self, node: Node) {
        match self.stack.last_mut() {
            Some(frame) => frame.children.push(node),
            None => self.root.push(node),
        }
    }

    /// Adds a text run. A line break after a `<br>` goes, because the `<br>` already shows it.
    pub(super) fn text(&mut self, text: &str) {
        let last = match self.stack.last() {
            Some(frame) => frame.children.last(),
            None => self.root.last(),
        };
        let text = if matches!(last, Some(Node::Break)) && !self.in_pre() {
            text.trim_start_matches('\n')
        } else {
            text
        };
        if !text.is_empty() {
            self.push(Node::Text(text.to_owned()));
        }
    }

    pub(super) fn close(&mut self) {
        let Some(Frame {
            kind,
            flat,
            children,
        }) = self.stack.pop()
        else {
            return;
        };
        if matches!(kind, Kind::Element(Element::Pre)) {
            self.pres -= 1;
        }
        match kind {
            Kind::Block => self.adopt(paragraphs(children)),
            Kind::Element(element) if !flat => {
                let children = if element.is_block() && element != Element::Pre {
                    tidy(children)
                } else {
                    children
                };
                self.push(Node::Element { element, children });
            }
            Kind::Link { href, title } if !flat => self.push(Node::Link {
                href,
                title,
                children,
            }),
            Kind::Plain | Kind::Element(_) | Kind::Link { .. } => self.adopt(children),
        }
    }

    /// The finished top-level blocks, without the blocks the sanitizing left empty.
    pub(super) fn finish(self) -> Vec<Node> {
        prune(paragraphs(self.root))
    }

    /// Moves `nodes` to the end of the innermost open tag.
    fn adopt(&mut self, mut nodes: Vec<Node>) {
        let target = match self.stack.last_mut() {
            Some(frame) => &mut frame.children,
            None => &mut self.root,
        };
        if target.is_empty() {
            *target = nodes;
        } else {
            target.append(&mut nodes);
        }
    }

    /// Whether a `pre` stands open, so its whitespace stays exact.
    fn in_pre(&self) -> bool {
        self.pres > 0
    }
}

/// `children` with each run of inline nodes in a paragraph. A run that shows nothing goes.
fn paragraphs(children: Vec<Node>) -> Vec<Node> {
    let mut out = Vec::new();
    let mut run = Vec::new();
    for child in children {
        if child.holds_block() {
            flush_run(&mut run, &mut out);
            out.push(child);
        } else {
            run.push(child);
        }
    }
    flush_run(&mut run, &mut out);
    out
}

/// Moves `run` to `out` as one paragraph.
fn flush_run(run: &mut Vec<Node>, out: &mut Vec<Node>) {
    let children = std::mem::take(run);
    if children.iter().any(Node::holds_content) {
        out.push(Node::Element {
            element: Element::P,
            children: tidy(children),
        });
    }
}

/// `children` without the whitespace at either end or beside a block, which no browser shows.
fn tidy(mut children: Vec<Node>) -> Vec<Node> {
    let blocks: Vec<bool> = children.iter().map(Node::holds_block).collect();
    for (at, child) in children.iter_mut().enumerate() {
        let Node::Text(text) = child else {
            continue;
        };
        let before = at.checked_sub(1).and_then(|before| blocks.get(before));
        if before.is_none_or(|block| *block) {
            text.drain(..text.len() - text.trim_start().len());
        }
        if blocks.get(at + 1).is_none_or(|block| *block) {
            text.truncate(text.trim_end().len());
        }
    }
    children.retain(|child| !matches!(child, Node::Text(text) if text.is_empty()));
    children
}
