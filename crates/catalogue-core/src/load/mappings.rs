//! The rule that one keyword, slug or term maps to one topic, or to one category.

use std::collections::BTreeMap;

use super::diagnostics::Diagnostics;
use crate::ids::Slug;

/// What the diagnostics of one data file call things.
#[derive(Debug, Clone, Copy)]
pub(super) struct MappingNames {
    /// One record of the file, such as `topic`.
    pub owner: &'static str,
    /// Several records of the file, such as `topics`.
    pub owners: &'static str,
    /// One crates.io category slug.
    pub slug: &'static str,
}

/// The mappings of one record.
#[derive(Debug, Clone, Copy)]
pub(super) struct Mapped<'a> {
    /// The record id.
    pub id: &'a Slug,
    /// The crates.io keywords.
    pub keywords: &'a [String],
    /// The crates.io category slugs.
    pub categories: &'a [String],
    /// The terms.
    pub terms: &'a [String],
}

/// Who claims each value of one mapping so far.
type Owners<'a> = BTreeMap<&'a str, &'a Slug>;

/// Reports a keyword, slug or term that two records claim, or that one record lists twice.
pub(super) fn check_mappings<'a>(
    names: MappingNames,
    records: impl IntoIterator<Item = Mapped<'a>>,
    path: &str,
    diags: &mut Diagnostics,
) {
    let mut keywords = Owners::new();
    let mut categories = Owners::new();
    let mut terms = Owners::new();
    let mut claims = Claims { names, path, diags };
    for record in records {
        claims.claim("term", record.terms, record.id, &mut terms);
        claims.claim("keyword", record.keywords, record.id, &mut keywords);
        claims.claim(names.slug, record.categories, record.id, &mut categories);
    }
}

/// What every claim of one file reports to.
struct Claims<'d> {
    names: MappingNames,
    path: &'d str,
    diags: &'d mut Diagnostics,
}

impl Claims<'_> {
    fn claim<'a>(
        &mut self,
        what: &str,
        values: &'a [String],
        id: &'a Slug,
        owners: &mut Owners<'a>,
    ) {
        let owner = self.names.owner;
        let many = self.names.owners;
        for value in values {
            match owners.insert(value.as_str(), id) {
                Some(first) if first == id => self.diags.push(
                    self.path,
                    format!("{owner} `{id}`: {what} `{value}` is listed twice"),
                ),
                Some(first) => self.diags.push(
                    self.path,
                    format!(
                        "{what} `{value}` maps to {many} `{first}` and `{id}`; a {what} maps to one {owner}"
                    ),
                ),
                None => {}
            }
        }
    }
}
