//! The README of an entry on its page. README text is untrusted input.
//!
//! [`events`] reads the Markdown and drops raw HTML with its content. [`parse`] turns the events
//! into a tree of allowed elements, and [`resolve`] decides every URL. [`html`] holds the one type
//! the templates print without escaping. [`view`] splits the tree into what the page shows first
//! and the rest. [`search`] builds the search text from the same events.

mod events;
mod html;
mod parse;
mod raw;
mod resolve;
mod search;
mod tree;
mod view;

pub(crate) use search::readme_search_text;
pub(crate) use view::ReadmeView;
