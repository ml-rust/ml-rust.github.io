//! Maps one element of a parsed README to what the tree holds for it.

use scraper::node::Element as DomElement;

use super::builder::{Builder, Kind};
use super::resolve::{ReadmeBase, resolve};
use super::tree::{Element, Node};

/// The tree element of the DOM element `name`, when the renderer writes one.
///
/// Headings sit one level under the page's own About heading, and never deeper than `h5`.
fn element_of(name: &str) -> Option<Element> {
    Some(match name {
        "p" => Element::P,
        "h1" => Element::H3,
        "h2" => Element::H4,
        "h3" | "h4" | "h5" | "h6" => Element::H5,
        "blockquote" => Element::Blockquote,
        "ul" => Element::Ul,
        "ol" => Element::Ol,
        "li" => Element::Li,
        "table" => Element::Table,
        "thead" => Element::Thead,
        "tbody" => Element::Tbody,
        "tr" => Element::Tr,
        "th" => Element::Th,
        "td" => Element::Td,
        "em" | "i" => Element::Em,
        "strong" | "b" => Element::Strong,
        "del" | "s" | "strike" => Element::Del,
        "code" | "tt" => Element::Code,
        "pre" => Element::Pre,
        _ => return None,
    })
}

/// Whether `name` is a block the renderer does not write. `html` is the root of the document.
fn is_plain_block(name: &str) -> bool {
    matches!(
        name,
        "html"
            | "div"
            | "center"
            | "section"
            | "article"
            | "main"
            | "header"
            | "footer"
            | "aside"
            | "nav"
            | "figure"
            | "figcaption"
            | "details"
            | "summary"
            | "dl"
            | "dt"
            | "dd"
            | "address"
    )
}

/// Opens the tag of `element`. Attributes come only from the names read here.
pub(super) fn open(builder: &mut Builder, element: &DomElement, base: &ReadmeBase) {
    let name = element.name();
    let kind = match name {
        "hr" => {
            builder.push(Node::Rule);
            Kind::Plain
        }
        "br" => {
            builder.push(Node::Break);
            Kind::Plain
        }
        "img" => {
            if let Some(image) = image_of(element, base) {
                builder.push(image);
            }
            Kind::Plain
        }
        "a" => link_of(element, base),
        _ => match element_of(name) {
            Some(allowed) => Kind::Element(allowed),
            None if is_plain_block(name) => Kind::Block,
            None => Kind::Plain,
        },
    };
    builder.open(kind);
}

/// The link of an `a`. A refused or missing `href` keeps only the text.
fn link_of(element: &DomElement, base: &ReadmeBase) -> Kind {
    match element.attr("href").and_then(|href| resolve(href, base)) {
        Some(found) => Kind::Link {
            href: found.link,
            title: attribute(element, "title"),
        },
        None => Kind::Plain,
    }
}

/// The image of an `img`. `None` when its source is missing or refused.
fn image_of(element: &DomElement, base: &ReadmeBase) -> Option<Node> {
    Some(Node::Image {
        src: resolve(element.attr("src")?, base)?.image?,
        alt: attribute(element, "alt"),
        title: attribute(element, "title"),
        width: dimension(element.attr("width")),
        height: dimension(element.attr("height")),
    })
}

/// The value of the attribute `name`, or nothing.
fn attribute(element: &DomElement, name: &str) -> String {
    element.attr(name).unwrap_or_default().to_owned()
}

/// A plain positive integer of at most 4 digits. Every other value, such as `100%`, goes.
fn dimension(value: Option<&str>) -> Option<u16> {
    let value = value?;
    if value.len() > 4 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok().filter(|number| *number > 0)
}
