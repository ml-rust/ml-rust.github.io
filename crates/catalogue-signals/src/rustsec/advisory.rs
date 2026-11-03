//! One RustSec advisory.

use jiff::civil;

/// The class of an advisory, from its `informational` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvisoryKind {
    /// A security vulnerability: the `informational` field is absent.
    Vulnerability,
    /// The crate has no maintainer.
    Unmaintained,
    /// The crate has a soundness hole.
    Unsound,
    /// Any other informational notice, including an unknown `informational` value.
    Notice,
}

impl AdvisoryKind {
    /// The lowercase name printed by the CLI.
    pub fn name(self) -> &'static str {
        match self {
            Self::Vulnerability => "vulnerability",
            Self::Unmaintained => "unmaintained",
            Self::Unsound => "unsound",
            Self::Notice => "notice",
        }
    }
}

/// An advisory file under `crates/` in the RustSec database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advisory {
    /// The advisory id, such as `RUSTSEC-2021-0072`.
    pub id: String,
    /// The crate name as the advisory spells it.
    pub package: String,
    /// The date RustSec published the advisory.
    pub date: civil::Date,
    /// The class of the advisory.
    pub kind: AdvisoryKind,
    /// The date RustSec withdrew the advisory, when it did.
    pub withdrawn: Option<civil::Date>,
    /// The version requirements that carry a fix. Empty when no fix exists.
    pub patched: Vec<String>,
    /// The version requirements the advisory does not affect.
    pub unaffected: Vec<String>,
    /// The link to the report, when the advisory has one.
    pub url: Option<String>,
}
