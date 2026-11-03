//! The entry name and the description of a family draft.

use catalogue_core::{Slug, canonical_crate_name};
use url::Url;

use super::fields::description_text;
use crate::dump::DumpSnapshot;

/// The entry name and slug: the repository name when it differs from the primary crate's.
///
/// The repository name is the last segment of the repository link. It must
/// canonicalise to a valid slug; otherwise the crate name gives both.
pub(super) fn entry_name(crate_name: &str, repo: &Url) -> Result<(String, Slug), String> {
    let segment = repo
        .path_segments()
        .and_then(|mut parts| parts.next_back())
        .unwrap_or_default();
    let canonical = canonical_crate_name(segment);
    let crate_canonical = canonical_crate_name(crate_name);
    if canonical != crate_canonical
        && let Ok(slug) = Slug::new(&canonical)
    {
        return Ok((segment.to_owned(), slug));
    }
    let slug = Slug::new(&crate_canonical)
        .map_err(|error| format!("the crate name is not a slug: {error}"))?;
    Ok((crate_name.to_owned(), slug))
}

/// The first usable description among `members` and the name of its crate.
///
/// Errors with the primary's reason when no member has one.
pub(super) fn description_of<'a>(
    snapshot: &'a DumpSnapshot,
    members: &[(u32, &str)],
) -> Result<(String, &'a str), String> {
    let mut first_error = None;
    for (id, _) in members {
        let Some(row) = snapshot.crates.get(id) else {
            continue;
        };
        match description_text(row.description.as_deref()) {
            Ok(text) => return Ok((text, row.name.as_str())),
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    Err(first_error.unwrap_or_else(|| "no description".to_owned()))
}
