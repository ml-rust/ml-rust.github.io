//! The README of an entry on its page. README text is untrusted input.
//!
//! [`dom`] reads the Markdown and its raw HTML into one parsed document. [`parse`] walks the
//! document into the tree of allowed elements: [`map`] decides what each element becomes,
//! [`builder`] holds the tree while it grows, and [`resolve`] decides every URL. [`html`] holds the
//! one type the templates print without escaping. [`view`] splits the tree into what the page shows
//! first and the rest. [`search`] builds the search text from the same document.

mod builder;
mod dom;
mod html;
mod map;
mod parse;
mod resolve;
mod search;
mod tree;
mod view;

pub(crate) use search::readme_search_text;
pub(crate) use view::ReadmeView;
