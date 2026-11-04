//! The rules for the crate metadata that maps a project to a topic or a category.

use super::discovery::{is_category_slug, keyword_problem};

/// Lists every rule that `keywords`, crates.io `categories` and `terms` break.
pub(super) fn mapping_problems(
    keywords: &[String],
    categories: &[String],
    terms: &[String],
) -> Vec<String> {
    let mut out = Vec::new();
    for keyword in keywords {
        if let Some(problem) = keyword_problem(keyword) {
            out.push(problem);
        }
    }
    for category in categories {
        if !is_category_slug(category) {
            out.push(format!(
                "category `{category}` must match `[a-z0-9_-]+(::[a-z0-9_-]+)*`"
            ));
        }
    }
    for term in terms {
        if !is_term(term) {
            out.push(format!(
                "term `{term}` must be lowercase words of `[a-z0-9]` separated by one space"
            ));
        }
    }
    out
}

/// Whether `value` is words of `[a-z0-9]` separated by one space.
fn is_term(value: &str) -> bool {
    value.split(' ').all(|word| {
        !word.is_empty() && word.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9'))
    })
}
