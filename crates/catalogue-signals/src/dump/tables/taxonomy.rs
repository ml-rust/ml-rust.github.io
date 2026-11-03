//! The `categories`, `keywords`, `crates_categories` and `crates_keywords` tables.

use std::io::Read;

use crate::dump::error::DumpError;
use crate::dump::names::TableName;
use crate::dump::snapshot::IdMap;
use crate::dump::table::{for_each_row, id_names};

/// Reads `categories` into a map from category id to slug.
pub(crate) fn read_categories(input: impl Read) -> Result<IdMap<String>, DumpError> {
    id_names(input, TableName::Categories, &["id", "slug"])
}

/// Reads `keywords` into a map from keyword id to keyword.
pub(crate) fn read_keywords(input: impl Read) -> Result<IdMap<String>, DumpError> {
    id_names(input, TableName::Keywords, &["id", "keyword"])
}

/// Reads `crates_categories` into a map from crate id to category slugs.
///
/// A row whose category id is not in `slugs` adds one to `unknown`.
pub(crate) fn read_crate_categories(
    input: impl Read,
    slugs: &IdMap<String>,
    unknown: &mut u64,
) -> Result<IdMap<Vec<String>>, DumpError> {
    read_links(
        input,
        TableName::CratesCategories,
        &["crate_id", "category_id"],
        slugs,
        unknown,
    )
}

/// Reads `crates_keywords` into a map from crate id to keywords.
///
/// A row whose keyword id is not in `keywords` adds one to `unknown`.
pub(crate) fn read_crate_keywords(
    input: impl Read,
    keywords: &IdMap<String>,
    unknown: &mut u64,
) -> Result<IdMap<Vec<String>>, DumpError> {
    read_links(
        input,
        TableName::CratesKeywords,
        &["crate_id", "keyword_id"],
        keywords,
        unknown,
    )
}

/// Reads a join table of `crate_id` and a lookup id, resolving the lookup id.
fn read_links(
    input: impl Read,
    table: TableName,
    names: &'static [&'static str; 2],
    lookup: &IdMap<String>,
    unknown: &mut u64,
) -> Result<IdMap<Vec<String>>, DumpError> {
    let [crate_column, link_column] = *names;
    let mut out: IdMap<Vec<String>> = IdMap::default();
    for_each_row(input, table, names, |row| {
        let crate_id = row.id(crate_column)?;
        match lookup.get(&row.id(link_column)?) {
            Some(name) => out.entry(crate_id).or_default().push(name.clone()),
            None => *unknown = unknown.saturating_add(1),
        }
        Ok(())
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_categories_and_counts_unknown_ids() {
        let slugs = read_categories(
            "category,crates_cnt,id,slug\nScience,5,1,science\nML,3,2,science::ml\n".as_bytes(),
        )
        .expect("categories");
        let mut unknown = 0;
        let links = read_crate_categories(
            "category_id,crate_id\n2,10\n1,10\n9,11\n".as_bytes(),
            &slugs,
            &mut unknown,
        )
        .expect("links");
        assert_eq!(
            links.get(&10).map(Vec::as_slice),
            Some(&["science::ml".to_owned(), "science".to_owned()][..])
        );
        assert_eq!(links.get(&11), None);
        assert_eq!(unknown, 1);
    }

    #[test]
    fn resolves_keywords() {
        let keywords =
            read_keywords("crates_cnt,id,keyword\n4,5,tensor\n".as_bytes()).expect("keywords");
        let mut unknown = 0;
        let links = read_crate_keywords(
            "crate_id,keyword_id\n10,5\n".as_bytes(),
            &keywords,
            &mut unknown,
        )
        .expect("links");
        assert_eq!(
            links.get(&10).map(Vec::as_slice),
            Some(&["tensor".to_owned()][..])
        );
        assert_eq!(unknown, 0);
    }
}
