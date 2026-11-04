//! The entry of one lead.

use catalogue_core::{
    Date, Description, DescriptionSource, Entry, EntryKind, Slug, crate_page, entry_name, repo_link,
};
use url::Url;

use super::super::family::Family;
use super::super::family_draft::{crate_names, family_members, member_category, member_topics};
use super::super::fields::{category_by_terms, https_link, topics_by_terms_in};
use super::super::names::{description_of, usable_text};
use super::super::primary::words;
use super::super::types::Mappings;
use crate::dump::{CrateRow, DumpSnapshot};
use crate::github::{RepoProfile, RepoRef};

/// Builds the entry of a lead, or the reason it yields none.
///
/// The canonical repository gives `repo`, so a crate that still names an old link changes
/// nothing. The primary crate gives the name and slug as in the dump flow. Without a crate the
/// repository name gives them. No category match leaves the category unset and keeps the draft.
/// An install channel of an app plays no part in the category.
pub(super) fn build_entry(
    snapshot: &DumpSnapshot,
    mappings: &Mappings<'_>,
    canonical: &RepoRef,
    profile: &RepoProfile,
    family: Option<&Family<'_>>,
    kind: EntryKind,
    added: Date,
) -> Result<(Slug, Entry), String> {
    let link = format!("https://github.com/{canonical}");
    let (repo, org_affiliated) = repo_link(Some(link.as_str()))?;
    let members = family.map(family_members).unwrap_or_default();
    let primary = family.and_then(|found| snapshot.crates.get(&found.primary.crate_id));
    let texts = lead_texts(primary, canonical, profile);
    let mut matched: Vec<Slug> = member_topics(snapshot, &members, mappings.topics)
        .into_iter()
        .collect();
    if matched.is_empty() {
        matched = topics_by_terms_in(mappings.topics, &texts);
    }
    if matched.is_empty() {
        return Err("no topic matches".to_owned());
    }
    let category = member_category(snapshot, &members, mappings.categories)
        .or_else(|| category_by_terms(mappings.categories, &texts));
    let (name, slug) = match primary {
        Some(row) => entry_name(&row.name, &repo)?,
        None => (canonical.name.clone(), repo_slug(&canonical.name)?),
    };
    let crates = match primary {
        Some(row) => crate_names(&row.name, &members)?,
        None => Vec::new(),
    };
    let description = entry_description(snapshot, &members, kind, profile, &repo)?;
    let homepage = primary
        .and_then(|row| https_link(row.homepage.as_deref()))
        .or_else(|| https_link(profile.homepage.as_deref()));
    let entry = Entry {
        name,
        kind,
        repo,
        homepage,
        crates,
        topics: matched,
        category,
        description,
        org_affiliated,
        distribution: None,
        added,
    };
    Ok((slug, entry))
}

/// The texts the terms of a lead read: the repository name, the repository description, each
/// repository topic, and the name and description of the primary crate.
fn lead_texts<'a>(
    primary: Option<&'a CrateRow>,
    canonical: &'a RepoRef,
    profile: &'a RepoProfile,
) -> Vec<&'a str> {
    let mut texts: Vec<&str> = vec![canonical.name.as_str()];
    texts.extend(profile.description.as_deref());
    texts.extend(profile.topics.iter().map(String::as_str));
    if let Some(row) = primary {
        texts.push(row.name.as_str());
        texts.extend(row.description.as_deref());
    }
    texts
}

/// A library keeps a crate description when a member has one. Every other draft takes the GitHub
/// repository description, under the rules of a crate description.
fn entry_description(
    snapshot: &DumpSnapshot,
    members: &[(u32, &str)],
    kind: EntryKind,
    profile: &RepoProfile,
    repo: &Url,
) -> Result<Description, String> {
    if kind == EntryKind::Library
        && let Ok((text, described_by)) = description_of(snapshot, members)
    {
        let source_url = crate_page(described_by)
            .ok_or_else(|| "the crates.io page is not a valid link".to_owned())?;
        return Ok(Description {
            text,
            source: DescriptionSource::Crate,
            source_url: Some(source_url),
        });
    }
    Ok(Description {
        text: usable_text(profile.description.as_deref())?,
        source: DescriptionSource::Repo,
        source_url: Some(repo.clone()),
    })
}

/// The slug of a repository name: its words joined with `-`, so `tool.rs` gives `tool-rs`.
fn repo_slug(name: &str) -> Result<Slug, String> {
    Slug::new(words(name).join("-"))
        .map_err(|error| format!("the repository name `{name}` is not a slug: {error}"))
}

#[cfg(test)]
mod tests {
    use catalogue_core::CrateName;

    use super::super::super::family::family;
    use super::super::super::fixtures::{Spec, World, categories, today, topics};
    use super::*;

    fn mappings_of<T>(run: impl FnOnce(&Mappings<'_>) -> T) -> T {
        run(&Mappings {
            topics: &topics(),
            categories: &categories(),
        })
    }

    fn profile(description: Option<&str>, topics: &[&str]) -> RepoProfile {
        RepoProfile {
            name_with_owner: "o/Tool.rs".to_owned(),
            archived: false,
            fork: false,
            description: description.map(str::to_owned),
            homepage: Some("https://tool.example".to_owned()),
            topics: topics.iter().map(|topic| (*topic).to_owned()).collect(),
            language: Some("Rust".to_owned()),
            license: None,
        }
    }

    fn canonical() -> RepoRef {
        RepoRef::parse("o/Tool.rs").expect("parses")
    }

    fn app(found: &RepoProfile) -> Result<(Slug, Entry), String> {
        let world = World::new(Vec::new());
        mappings_of(|mappings| {
            build_entry(
                &world.snapshot,
                mappings,
                &canonical(),
                found,
                None,
                EntryKind::App,
                today(),
            )
        })
    }

    #[test]
    fn an_app_takes_every_field_from_the_repository() {
        let found = profile(Some("A  sparse matrix\ntool."), &[]);
        let (slug, entry) = app(&found).expect("drafts");
        assert_eq!(slug.as_str(), "tool-rs");
        assert_eq!(entry.name, "Tool.rs");
        assert_eq!(entry.kind, EntryKind::App);
        assert_eq!(entry.repo.as_str(), "https://github.com/o/tool.rs");
        assert_eq!(
            entry.homepage.as_ref().map(Url::as_str),
            Some("https://tool.example/")
        );
        assert!(entry.crates.is_empty());
        assert_eq!(entry.description.text, "A sparse matrix tool.");
        assert_eq!(entry.description.source, DescriptionSource::Repo);
        assert_eq!(entry.description.source_url.as_ref(), Some(&entry.repo));
        assert_eq!(entry.problems(), Vec::<String>::new());
    }

    #[test]
    fn a_repository_topic_maps_an_app_to_a_topic() {
        let found = profile(Some("A tool."), &["sparse-matrix"]);
        let (_, entry) = app(&found).expect("drafts");
        let ids: Vec<&str> = entry.topics.iter().map(Slug::as_str).collect();
        assert_eq!(ids, ["foundations"]);
    }

    #[test]
    fn no_topic_and_no_description_give_a_reason() {
        let no_topic = profile(Some("A tool."), &["parallelism"]);
        assert_eq!(app(&no_topic).err().as_deref(), Some("no topic matches"));
        let no_text = profile(None, &["sparse-matrix"]);
        assert_eq!(app(&no_text).err().as_deref(), Some("no description"));
    }

    #[test]
    fn a_long_repository_description_keeps_its_leading_sentences() {
        let long = format!("A sparse matrix tool. {}.", "Word ".repeat(70).trim_end());
        let found = profile(Some(&long), &[]);
        let (_, entry) = app(&found).expect("drafts");
        assert_eq!(entry.description.text, "A sparse matrix tool.");
    }

    fn library(spec: Spec, found: &RepoProfile) -> Result<(Slug, Entry), String> {
        let world = World::new(vec![spec]);
        let members = world.candidates.iter().map(|c| (c, 0.0)).collect();
        let family = family(&world.snapshot, "github.com/o/tool.rs", members, 0.0).expect("family");
        mappings_of(|mappings| {
            build_entry(
                &world.snapshot,
                mappings,
                &canonical(),
                found,
                Some(&family),
                EntryKind::Library,
                today(),
            )
        })
    }

    #[test]
    fn a_lead_without_a_category_match_keeps_its_draft() {
        let (_, entry) =
            library(Spec::new(1, "toolkit"), &profile(Some("A tool."), &[])).expect("drafts");
        assert_eq!(
            entry.category.as_ref().map(Slug::as_str),
            Some("alpha-tools")
        );
        let mut spec = Spec::new(1, "toolkit");
        spec.keywords = Vec::new();
        spec.categories = vec!["science::math"];
        let (_, entry) = library(spec, &profile(Some("A tool."), &[])).expect("drafts");
        assert_eq!(entry.category, None);
    }

    #[test]
    fn a_repository_topic_can_set_the_category_of_an_app() {
        let (_, entry) = app(&profile(Some("A tool."), &["sparse-matrix"])).expect("drafts");
        assert_eq!(
            entry.category.as_ref().map(Slug::as_str),
            Some("zeta-tools")
        );
    }

    #[test]
    fn a_library_keeps_its_crate_description_and_the_canonical_repository() {
        let mut spec = Spec::new(1, "toolkit");
        spec.repository = Some("https://github.com/old/tool".to_owned());
        let found = profile(Some("Repository text."), &[]);
        let (_, entry) = library(spec, &found).expect("drafts");
        assert_eq!(entry.repo.as_str(), "https://github.com/o/tool.rs");
        assert_eq!(entry.description.text, "A tokenizer.");
        assert_eq!(entry.description.source, DescriptionSource::Crate);
        let crates: Vec<&str> = entry.crates.iter().map(CrateName::as_str).collect();
        assert_eq!(crates, ["toolkit"]);
    }

    #[test]
    fn a_library_without_a_crate_description_takes_the_repository_description() {
        let mut spec = Spec::new(1, "toolkit");
        spec.description = None;
        let found = profile(Some("Repository text."), &[]);
        let (_, entry) = library(spec, &found).expect("drafts");
        assert_eq!(entry.description.text, "Repository text.");
        assert_eq!(entry.description.source, DescriptionSource::Repo);
        assert_eq!(entry.problems(), Vec::<String>::new());
    }

    #[test]
    fn a_repository_name_without_a_word_is_no_slug() {
        let error = repo_slug("...").expect_err("no slug");
        assert!(error.contains("`...`"), "{error}");
    }
}
