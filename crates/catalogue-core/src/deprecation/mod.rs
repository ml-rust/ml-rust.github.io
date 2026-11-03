//! Deprecation notices found in a crate's description or README.
//!
//! [`scan`] applies two rules.
//!
//! - Description: the text splits into clauses at `. , ; : ! ? ( ) [ ] |`,
//!   line breaks, `–`, `—`, and ` - ` (a hyphen with a space on each side). Each
//!   clause loses its leading and trailing non-alphanumeric characters and
//!   any leading self-reference (`this crate is`, `this project is`,
//!   `this library is`, `this package is`, `this repository is`,
//!   `this repo is`, `this is`, `it is`, `now`, `currently`, `officially`,
//!   `a`, `an`). The clause matches when the rest equals a description phrase
//!   (`no longer maintained`, `temporary fork`, `unmaintained`, `deprecated`,
//!   `unfinished`, `do not use`), or starts with one followed by a continuation word
//!   (`in favor of`, `in favour of`, `use`, `please`, `see`, `of`, `since`,
//!   `as of`, `for now`, `fork`, `this`, `it`, `yet`). The phrase then refers
//!   to the crate itself. `replaces the deprecated foo crate` and
//!   `deprecated APIs are supported` do not match.
//! - README: only the first 1000 characters count. The README phrases are
//!   `no longer maintained`, `unmaintained` and `deprecated`. The first ATX,
//!   setext or `<h1>` heading matches when it contains one. Any other line
//!   matches when it contains one and also `this crate`, `this project`, `this library`,
//!   or the words `is deprecated`.
//!
//! Matching ignores case.

mod description;
mod notice;
mod readme;

pub use notice::{DeprecationNotice, EVIDENCE_MAX_CHARS, Field, scan};
pub use readme::README_WINDOW_CHARS;
