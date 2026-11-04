use url::Url;

use crate::ids::{Slug, canonical_crate_name};

/// Words that carry no project identity when they make up a whole repository name.
const GENERIC_WORDS: [&str; 13] = [
    "rust", "rs", "sdk", "api", "lib", "core", "client", "bindings", "crate", "crates", "main",
    "python", "py",
];

/// The last segment of the repository link.
fn repo_segment(repo: &Url) -> &str {
    repo.path_segments()
        .and_then(|mut parts| parts.next_back())
        .unwrap_or_default()
}

/// True when every word of the repository name is in the fixed generic list.
///
/// A word ends at any character that is not a letter or digit. A name with no word is not generic.
pub fn is_generic_repo_name(name: &str) -> bool {
    let mut words = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .peekable();
    words.peek().is_some()
        && words.all(|word| GENERIC_WORDS.contains(&word.to_lowercase().as_str()))
}

/// The entry name and slug: the repository name when it differs from the primary crate's.
///
/// The repository name is the last segment of the repository link. It must
/// canonicalise to a valid slug; otherwise the crate name gives both. A generic
/// repository name (see [`is_generic_repo_name`]) gives no identity, so the crate name
/// gives both.
pub fn entry_name(crate_name: &str, repo: &Url) -> Result<(String, Slug), String> {
    let segment = repo_segment(repo);
    let canonical = canonical_crate_name(segment);
    let crate_canonical = canonical_crate_name(crate_name);
    if canonical != crate_canonical
        && !is_generic_repo_name(segment)
        && let Ok(slug) = Slug::new(&canonical)
    {
        return Ok((segment.to_owned(), slug));
    }
    let slug = Slug::new(&crate_canonical)
        .map_err(|error| format!("the crate name is not a slug: {error}"))?;
    Ok((crate_name.to_owned(), slug))
}

/// The entry name and slug of an app with no crate.
///
/// The repository name gives both. A generic repository name gives `<owner>-<repo>` for both.
pub fn repo_entry_name(repo: &Url) -> Result<(String, Slug), String> {
    let segment = repo_segment(repo);
    let name = if is_generic_repo_name(segment) {
        let owner = repo.path_segments().and_then(|mut parts| parts.next());
        format!("{}-{segment}", owner.unwrap_or_default())
    } else {
        segment.to_owned()
    };
    let slug = Slug::new(canonical_crate_name(&name))
        .map_err(|error| format!("the repository name `{name}` is not a slug: {error}"))?;
    Ok((name, slug))
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

    #[test]
    fn a_generic_repository_name_gives_way_to_the_crate() {
        let (name, slug) =
            entry_name("widget", &link("https://github.com/acme/rust")).expect("names");
        assert_eq!(name, "widget");
        assert_eq!(slug.as_str(), "widget");
    }

    #[test]
    fn a_name_of_only_generic_words_gives_way_to_the_crate() {
        let (name, slug) =
            entry_name("widget", &link("https://github.com/acme/Rust-SDK")).expect("names");
        assert_eq!(name, "widget");
        assert_eq!(slug.as_str(), "widget");
    }

    #[test]
    fn a_name_with_one_distinct_word_is_not_generic() {
        let (name, slug) =
            entry_name("widget", &link("https://github.com/acme/widget-lts")).expect("names");
        assert_eq!(name, "widget-lts");
        assert_eq!(slug.as_str(), "widget-lts");
        assert!(!is_generic_repo_name("gadget"));
        assert!(!is_generic_repo_name(""));
        assert!(is_generic_repo_name("rust_sdk"));
    }

    #[test]
    fn an_app_with_a_distinct_repository_name_takes_it() {
        let (name, slug) = repo_entry_name(&link("https://github.com/acme/Gadget")).expect("names");
        assert_eq!(name, "Gadget");
        assert_eq!(slug.as_str(), "gadget");
    }

    #[test]
    fn an_app_with_a_generic_repository_name_takes_owner_and_repo() {
        let (name, slug) = repo_entry_name(&link("https://github.com/acme/rust")).expect("names");
        assert_eq!(name, "acme-rust");
        assert_eq!(slug.as_str(), "acme-rust");
    }
}
