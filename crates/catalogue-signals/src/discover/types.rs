use std::fmt;

/// The rules that select candidate crates.
///
/// The fields hold plain text. The caller converts from its own schema.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rules {
    /// Category slugs. A slug also matches its sub-categories.
    pub categories: Vec<String>,
    /// Keywords, compared in lowercase.
    pub keywords: Vec<String>,
    /// Crate names that are candidates themselves and whose dependents become candidates.
    ///
    /// Names resolve the way crates.io resolves them: ASCII case and `-`
    /// against `_` do not matter.
    pub core_crates: Vec<String>,
}

/// Why a crate is a candidate.
///
/// The variants sort in declaration order, then by text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reason {
    /// The crate is one of the rules' core crates.
    Core,
    /// The crate lists this category slug, which equals or extends a rule category.
    Category(String),
    /// The crate lists this lowercase keyword, which equals a rule keyword.
    Keyword(String),
    /// The crate's default version depends on this core crate, as written in the rules.
    DependsOn(String),
}

impl fmt::Display for Reason {
    /// Writes `core`, `category:<slug>`, `keyword:<kw>` or `depends-on:<crate>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core => f.write_str("core"),
            Self::Category(slug) => write!(f, "category:{slug}"),
            Self::Keyword(keyword) => write!(f, "keyword:{keyword}"),
            Self::DependsOn(name) => write!(f, "depends-on:{name}"),
        }
    }
}

/// A crate that matches at least one rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The crates.io crate id.
    pub crate_id: u32,
    /// The crate name.
    pub name: String,
    /// Every rule the crate matches, sorted and without repeats.
    pub reasons: Vec<Reason>,
    /// The crate carries a deprecation notice in its description or README.
    pub deprecated: bool,
}

/// The result of one discovery run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovered {
    /// The candidates, sorted by name.
    pub candidates: Vec<Candidate>,
    /// Rule core crates with no crate of that name in the snapshot, in rule order.
    pub unknown_core_crates: Vec<String>,
}
