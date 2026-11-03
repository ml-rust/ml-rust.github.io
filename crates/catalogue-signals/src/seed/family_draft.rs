//! The draft of one family.

use std::collections::BTreeSet;

use catalogue_core::{
    CRATES_IO_PAGE, CrateName, Date, Description, DescriptionSource, Entry, EntryKind, Slug,
    Topics, entry_name, repo_link,
};
use url::Url;

use super::family::Family;
use super::fields::{https_link, matching_topics};
use super::names::description_of;
use crate::dump::DumpSnapshot;

/// Builds the draft of one family, or the reason it yields none.
///
/// The repository name gives the entry name and slug when it differs from the
/// primary crate's name and forms a valid slug; otherwise the primary does.
/// The primary gives the repository and homepage. The description comes from
/// the first member, primary first, with a usable crates.io description. The
/// topics are those matched by any member.
/// The crates are the primary, then the other members sorted by name.
pub(super) fn draft_family(
    snapshot: &DumpSnapshot,
    family: &Family<'_>,
    topics: &Topics,
    today: Date,
) -> Result<(Slug, Entry), String> {
    let members: Vec<(u32, &str)> = std::iter::once(family.primary)
        .chain(family.rest.iter().copied())
        .map(|member| (member.crate_id, member.name.as_str()))
        .collect();
    let row = snapshot
        .crates
        .get(&family.primary.crate_id)
        .ok_or_else(|| "the crate is not in the dump".to_owned())?;
    let crate_name = row.name.as_str();
    let (repo, org_affiliated) = repo_link(row.repository.as_deref())?;
    let (text, described_by) = description_of(snapshot, &members)?;
    let mut matched: BTreeSet<Slug> = BTreeSet::new();
    for (id, _) in &members {
        let keywords = snapshot.keywords.get(id).map_or(&[][..], Vec::as_slice);
        let categories = snapshot.categories.get(id).map_or(&[][..], Vec::as_slice);
        matched.extend(matching_topics(topics, keywords, categories));
    }
    if matched.is_empty() {
        return Err("no topic matches".to_owned());
    }
    let (name, slug) = entry_name(crate_name, &repo)?;
    let mut others: Vec<&str> = members.iter().skip(1).map(|(_, name)| *name).collect();
    others.sort_unstable();
    let crates = std::iter::once(crate_name)
        .chain(others)
        .map(|member| {
            CrateName::new(member).map_err(|error| format!("the crate name is not valid: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source_url = Url::parse(&format!("{CRATES_IO_PAGE}{described_by}"))
        .map_err(|_| "the crates.io page is not a valid link".to_owned())?;
    let entry = Entry {
        name,
        kind: EntryKind::Library,
        repo,
        homepage: https_link(row.homepage.as_deref()),
        crates,
        topics: matched.into_iter().collect(),
        description: Description {
            text,
            source: DescriptionSource::Crate,
            source_url: Some(source_url),
        },
        org_affiliated,
        distribution: None,
        added: today,
    };
    Ok((slug, entry))
}

#[cfg(test)]
mod tests {
    use catalogue_core::CrateName;
    use url::Url;

    use crate::dump::DependencyKind;

    use super::super::draft::draft_entries;
    use super::super::fixtures::{Spec, World, today, topics};
    use super::super::types::{Drafts, ExistingIndex};

    const REPO: &str = "https://github.com/ml-rust/fam";

    fn member(id: u32, name: &'static str, users: f64, keyword: &'static str) -> Spec {
        let mut spec = Spec::new(id, name);
        spec.repository = Some(REPO.to_owned());
        spec.users = users;
        spec.keywords = vec![keyword];
        spec
    }

    fn run(specs: Vec<Spec>) -> Drafts {
        let world = World::new(specs);
        draft_entries(
            &world.snapshot,
            &world.candidates,
            &world.usage,
            &topics(),
            &ExistingIndex::default(),
            2.0,
            today(),
        )
    }

    fn skipped(drafts: &Drafts) -> Vec<(&str, &str)> {
        drafts
            .skipped
            .iter()
            .map(|(name, reason)| (name.as_str(), reason.as_str()))
            .collect()
    }

    #[test]
    fn a_family_becomes_one_draft() {
        let mut primary = member(1, "fam", 9.0, "nlp");
        primary.description = None;
        let mut tied = member(2, "fam-derive", 9.0, "nlp");
        tied.description = Some("x".repeat(301));
        let mut small = member(3, "fam-core", 3.0, "tensor");
        small.description = Some("Core types.".to_owned());
        let drafts = run(vec![small, tied, primary]);
        assert!(drafts.skipped.is_empty(), "{:?}", drafts.skipped);
        let [(slug, entry)] = drafts.entries.as_slice() else {
            panic!("one draft expected");
        };
        assert_eq!(slug.as_str(), "fam");
        assert_eq!(entry.name, "fam");
        let crates: Vec<&str> = entry.crates.iter().map(CrateName::as_str).collect();
        assert_eq!(crates, ["fam", "fam-core", "fam-derive"]);
        let found: Vec<&str> = entry.topics.iter().map(|t| t.as_str()).collect();
        assert_eq!(found, ["foundations", "nlp"]);
        assert_eq!(entry.description.text, "Core types.");
        assert_eq!(
            entry.description.source_url.as_ref().map(Url::as_str),
            Some("https://crates.io/crates/fam-core")
        );
        assert!(entry.org_affiliated);
        assert_eq!(entry.problems(), Vec::<String>::new());
    }

    #[test]
    fn the_primary_description_wins_when_usable() {
        let drafts = run(vec![
            member(1, "fam", 9.0, "nlp"),
            member(2, "fam-x", 3.0, "nlp"),
        ]);
        let [(_, entry)] = drafts.entries.as_slice() else {
            panic!("one draft expected");
        };
        assert_eq!(
            entry.description.source_url.as_ref().map(Url::as_str),
            Some("https://crates.io/crates/fam")
        );
    }

    #[test]
    fn a_tie_goes_to_the_shortest_then_alphabetical_name() {
        let drafts = run(vec![
            member(1, "fam-bb", 5.0, "nlp"),
            member(2, "fam-aa", 5.0, "nlp"),
            member(3, "fam-c", 5.0, "nlp"),
        ]);
        assert_eq!(drafts.entries[0].0.as_str(), "fam");
        assert_eq!(drafts.entries[0].1.crates[0].as_str(), "fam-c");
        let crates: Vec<&str> = drafts.entries[0]
            .1
            .crates
            .iter()
            .map(CrateName::as_str)
            .collect();
        assert_eq!(crates, ["fam-c", "fam-aa", "fam-bb"]);
    }

    #[test]
    fn repository_spellings_join_one_family() {
        let mut other = member(2, "fam-b", 3.0, "nlp");
        other.repository = Some("https://www.GitHub.com/ML-Rust/fam.git/".to_owned());
        let drafts = run(vec![member(1, "fam-a", 9.0, "nlp"), other]);
        assert_eq!(drafts.entries.len(), 1);
        assert_eq!(drafts.entries[0].1.crates.len(), 2);
    }

    #[test]
    fn two_repositories_stay_separate() {
        let mut other = member(2, "fam-b", 3.0, "nlp");
        other.repository = Some("https://github.com/ml-rust/other".to_owned());
        let drafts = run(vec![member(1, "fam-a", 9.0, "nlp"), other]);
        let slugs: Vec<&str> = drafts.entries.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(slugs, ["fam", "other"]);
        assert!(drafts.entries.iter().all(|(_, e)| e.crates.len() == 1));
    }

    #[test]
    fn members_failing_eligibility_are_listed_and_left_out() {
        let mut yanked = member(2, "fam-old", 8.0, "nlp");
        yanked.yanked = true;
        let mut deprecated = member(3, "fam-dep", 8.0, "nlp");
        deprecated.deprecated = true;
        let drafts = run(vec![member(1, "fam", 5.0, "nlp"), yanked, deprecated]);
        let [(_, entry)] = drafts.entries.as_slice() else {
            panic!("one draft expected");
        };
        let crates: Vec<&str> = entry.crates.iter().map(CrateName::as_str).collect();
        assert_eq!(crates, ["fam"]);
        let found = skipped(&drafts);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].0, "fam-dep");
        assert!(found[0].1.contains("deprecated"), "{found:?}");
        assert_eq!(found[1].0, "fam-old");
        assert!(found[1].1.contains("yanked"), "{found:?}");
    }

    #[test]
    fn a_family_without_a_usable_description_is_skipped_once() {
        let mut a = member(1, "fam", 9.0, "nlp");
        a.description = None;
        let mut b = member(2, "fam-b", 3.0, "nlp");
        b.description = Some("x".repeat(301));
        let drafts = run(vec![a, b]);
        assert!(drafts.entries.is_empty());
        assert_eq!(skipped(&drafts), [("fam", "no description")]);
    }

    #[test]
    fn a_hub_named_like_the_repository_is_the_primary() {
        let mut hub = member(1, "fam", 1.0, "nlp");
        hub.description = Some("The hub.".to_owned());
        let mut a = member(2, "fam-a", 50.0, "nlp");
        a.deps = vec![(1, DependencyKind::Normal)];
        let mut b = member(3, "fam-b", 40.0, "nlp");
        b.deps = vec![(1, DependencyKind::Normal)];
        let drafts = run(vec![a, b, hub]);
        let [(slug, entry)] = drafts.entries.as_slice() else {
            panic!("one draft expected");
        };
        assert_eq!(slug.as_str(), "fam");
        let crates: Vec<&str> = entry.crates.iter().map(CrateName::as_str).collect();
        assert_eq!(crates, ["fam", "fam-a", "fam-b"]);
        assert_eq!(entry.description.text, "The hub.");
    }

    #[test]
    fn the_entry_is_named_after_the_repository() {
        let mut core = member(1, "fam-core", 50.0, "tensor");
        core.repository = Some("https://github.com/o/Cand_le".to_owned());
        let mut facade = member(2, "fam-facade", 1.0, "nlp");
        facade.repository = core.repository.clone();
        facade.deps = vec![(1, DependencyKind::Normal)];
        let drafts = run(vec![core, facade]);
        let [(slug, entry)] = drafts.entries.as_slice() else {
            panic!("one draft expected");
        };
        assert_eq!(slug.as_str(), "cand-le");
        assert_eq!(entry.name, "cand_le");
        let crates: Vec<&str> = entry.crates.iter().map(CrateName::as_str).collect();
        assert_eq!(crates, ["fam-facade", "fam-core"]);
        assert_eq!(entry.problems(), Vec::<String>::new());
    }

    #[test]
    fn a_repository_name_that_is_not_a_slug_falls_back_to_the_crate_name() {
        let mut spec = Spec::new(1, "tool");
        spec.repository = Some("https://github.com/o/v1.2".to_owned());
        let drafts = run(vec![spec]);
        let [(slug, entry)] = drafts.entries.as_slice() else {
            panic!("one draft expected");
        };
        assert_eq!(slug.as_str(), "tool");
        assert_eq!(entry.name, "tool");
    }

    #[test]
    fn a_gitlab_repository_drafts() {
        let mut spec = Spec::new(1, "lab");
        spec.repository = Some("https://GitLab.com/Group/Lab.git".to_owned());
        let drafts = run(vec![spec]);
        assert!(drafts.skipped.is_empty(), "{:?}", drafts.skipped);
        let entry = &drafts.entries[0].1;
        assert_eq!(entry.repo.as_str(), "https://gitlab.com/group/lab");
        assert!(!entry.org_affiliated);
    }

    #[test]
    fn a_non_https_repository_skips() {
        let mut spec = Spec::new(1, "plain");
        spec.repository = Some("http://example.com/o/r".to_owned());
        let drafts = run(vec![spec]);
        assert!(drafts.entries.is_empty());
        let found = skipped(&drafts);
        assert!(found[0].1.contains("not an https link"), "{found:?}");
    }
}
