//! The one type templates print without escaping, and the only code that builds it.
//!
//! README text is untrusted input. [`render`] writes every tag and attribute name itself from the
//! tree, and escapes every text run and attribute value.

use std::fmt;

use super::tree::Node;

/// HTML the renderer wrote from a README tree. [`render`] is the only constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadmeHtml(String);

impl fmt::Display for ReadmeHtml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl askama::filters::HtmlSafe for ReadmeHtml {}

/// The HTML of `nodes`.
pub(super) fn render(nodes: &[Node]) -> ReadmeHtml {
    let mut out = String::new();
    write_nodes(nodes, &mut out);
    ReadmeHtml(out)
}

fn write_nodes(nodes: &[Node], out: &mut String) {
    for node in nodes {
        write_node(node, out);
    }
}

fn write_node(node: &Node, out: &mut String) {
    match node {
        Node::Text(text) => escape_into(text, out),
        Node::Break => out.push_str("<br>"),
        Node::Rule => out.push_str("<hr>"),
        Node::Element { element, children } => {
            let name = element.name();
            let table = name == "table";
            if table {
                out.push_str("<div class=\"table-scroll\">");
            }
            out.push('<');
            out.push_str(name);
            out.push('>');
            write_nodes(children, out);
            out.push_str("</");
            out.push_str(name);
            out.push('>');
            if table {
                out.push_str("</div>");
            }
        }
        Node::Link {
            href,
            title,
            children,
        } => {
            out.push_str("<a");
            write_attribute("href", href.as_str(), out);
            write_title(title, out);
            out.push_str(" rel=\"nofollow noopener\">");
            write_nodes(children, out);
            out.push_str("</a>");
        }
        Node::Image { src, alt, title } => {
            out.push_str("<img");
            write_attribute("src", src.as_str(), out);
            write_attribute("alt", alt, out);
            write_title(title, out);
            out.push_str(" loading=\"lazy\">");
        }
    }
}

fn write_title(title: &str, out: &mut String) {
    if !title.is_empty() {
        write_attribute("title", title, out);
    }
}

/// Writes one attribute. Every `name` is a literal of this file.
fn write_attribute(name: &str, value: &str, out: &mut String) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    escape_into(value, out);
    out.push('"');
}

/// Writes `text` with `&`, `<`, `>`, `"` and `'` escaped.
fn escape_into(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use askama::Template;

    use super::*;
    use crate::href::SafeHref;

    #[derive(Template)]
    #[template(source = "<div>{{ html }}</div>{{ text }}", ext = "html")]
    struct Page<'a> {
        html: ReadmeHtml,
        text: &'a str,
    }

    #[test]
    fn a_template_prints_the_html_as_it_is_and_escapes_other_values() {
        let page = Page {
            html: render(&[Node::Rule]),
            text: "<b>",
        };
        assert_eq!(
            page.render().expect("renders"),
            "<div><hr></div>&#60;b&#62;"
        );
    }

    #[test]
    fn text_and_attributes_are_escaped() {
        let html = render(&[
            Node::Text("<script>&\"'".to_owned()),
            Node::Image {
                src: SafeHref::new("https://example.org/a.png?x=1&y=2"),
                alt: "a\" onerror=\"x".to_owned(),
                title: "t'<".to_owned(),
            },
        ]);
        assert_eq!(
            html.to_string(),
            "&lt;script&gt;&amp;&quot;&#39;\
             <img src=\"https://example.org/a.png?x=1&amp;y=2\" alt=\"a&quot; onerror=&quot;x\" \
             title=\"t&#39;&lt;\" loading=\"lazy\">"
        );
    }

    #[test]
    fn a_link_carries_the_renderer_rel_and_an_empty_title_prints_nothing() {
        let html = render(&[Node::Link {
            href: SafeHref::new("https://example.org/"),
            title: String::new(),
            children: vec![Node::Text("a".to_owned())],
        }]);
        assert_eq!(
            html.to_string(),
            "<a href=\"https://example.org/\" rel=\"nofollow noopener\">a</a>"
        );
    }
}
