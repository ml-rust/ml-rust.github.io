use url::Url;

use crate::ids::{Slug, canonical_crate_name};

/// The entry name and slug: the repository name when it differs from the primary crate's.
///
/// The repository name is the last segment of the repository link. It must
/// canonicalise to a valid slug; otherwise the crate name gives both.
pub fn entry_name(crate_name: &str, repo: &Url) -> Result<(String, Slug), String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn link(text: &str) -> Url {
        Url::parse(text).expect("link parses")
    }

    #[test]
    fn repository_name_wins_when_it_differs() {
        let (name, slug) =
            entry_name("foo-core", &link("https://github.com/o/foo")).expect("names");
        assert_eq!(name, "foo");
        assert_eq!(slug.as_str(), "foo");
    }

    #[test]
    fn crate_name_is_used_when_the_repository_name_matches() {
        let (name, slug) =
            entry_name("Foo_Bar", &link("https://github.com/o/foo-bar")).expect("names");
        assert_eq!(name, "Foo_Bar");
        assert_eq!(slug.as_str(), "foo-bar");
    }
}
