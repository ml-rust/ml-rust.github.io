//! The draft of one family.

use std::collections::BTreeSet;

use catalogue_core::{
    Categories, CrateName, Date, Description, DescriptionSource, Entry, EntryKind, Slug, Topics,
    crate_page, entry_name, repo_link,
};

use super::family::Family;
use super::fields::{
    category_by_metadata, category_by_terms, https_link, matching_topics, topics_by_terms,
};
use super::names::description_of;
use super::types::Mappings;
use crate::dump::DumpSnapshot;

/// Builds the draft of one family, or the reason it yields none.
///
/// The repository name gives the entry name and slug when it differs from the
/// primary crate's name and forms a valid slug; otherwise the primary does.
/// The primary gives the repository and homepage. The description comes from
/// the first member, primary first, with a usable crates.io description. The
/// topics are those whose keywords or categories match any member. When none
/// does, they are those with a term in the primary's name or description.
/// The category comes from the same two steps and can stay unset.
/// The crates are the primary, then the other members sorted by name.
pub(super) fn draft_family(
    snapshot: &DumpSnapshot,
    family: &Family<'_>,
    mappings: &Mappings<'_>,
    today: Date,
) -> Result<(Slug, Entry), String> {
    let topics = mappings.topics;
    let members = family_members(family);
    let row = snapshot
        .crates
        .get(&family.primary.crate_id)
        .ok_or_else(|| "the crate is not in the dump".to_owned())?;
    let crate_name = row.name.as_str();
    let (repo, org_affiliated) = repo_link(row.repository.as_deref())?;
    let (text, described_by) = description_of(snapshot, &members)?;
    let mut matched = member_topics(snapshot, &members, topics);
    if matched.is_empty() {
        matched.extend(topics_by_terms(
            topics,
            crate_name,
            row.description.as_deref(),
        ));
    }
    if matched.is_empty() {
        return Err("no topic matches".to_owned());
    }
    let category = member_category(snapshot, &members, mappings.categories).or_else(|| {
        category_by_terms(
            mappings.categories,
            &[crate_name, row.description.as_deref().unwrap_or_default()],
        )
    });
    let (name, slug) = entry_name(crate_name, &repo)?;
    let crates = crate_names(crate_name, &members)?;
    let source_url = crate_page(described_by)
        .ok_or_else(|| "the crates.io page is not a valid link".to_owned())?;
    let entry = Entry {
        name,
        kind: EntryKind::Library,
        repo,
        homepage: https_link(row.homepage.as_deref()),
        crates,
        topics: matched.into_iter().collect(),
        category,
        part_of: None,
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

/// The id and name of each member, the primary first.
pub(super) fn family_members<'a>(family: &Family<'a>) -> Vec<(u32, &'a str)> {
    family
        .members()
        .map(|member| (member.crate_id, member.name.as_str()))
        .collect()
}

/// The topics whose keywords or categories match any of `members`.
pub(super) fn member_topics(
    snapshot: &DumpSnapshot,
    members: &[(u32, &str)],
    topics: &Topics,
) -> BTreeSet<Slug> {
    let mut matched = BTreeSet::new();
    for (id, _) in members {
        let keywords = snapshot.keywords.get(id).map_or(&[][..], Vec::as_slice);
        let categories = snapshot.categories.get(id).map_or(&[][..], Vec::as_slice);
        matched.extend(matching_topics(topics, keywords, categories));
    }
    matched
}

/// The first category, in file order, whose keywords or slugs match any of `members`.
pub(super) fn member_category(
    snapshot: &DumpSnapshot,
    members: &[(u32, &str)],
    categories: &Categories,
) -> Option<Slug> {
    let mut keywords = Vec::new();
    let mut slugs = Vec::new();
    for (id, _) in members {
        keywords.extend(snapshot.keywords.get(id).into_iter().flatten().cloned());
        slugs.extend(snapshot.categories.get(id).into_iter().flatten().cloned());
    }
    category_by_metadata(categories, &keywords, &slugs)
}

/// The crate list of a draft: `primary`, then the other `members` sorted by name.
pub(super) fn crate_names(
    primary: &str,
    members: &[(u32, &str)],
) -> Result<Vec<CrateName>, String> {
    let mut others: Vec<&str> = members.iter().skip(1).map(|(_, name)| *name).collect();
    others.sort_unstable();
    std::iter::once(primary)
        .chain(others)
        .map(|member| {
            CrateName::new(member).map_err(|error| format!("the crate name is not valid: {error}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use catalogue_core::CrateName;
    use url::Url;

    use crate::dump::DependencyKind;

    use super::super::draft::draft_entries;
    use super::super::fixtures::{Spec, World, categories, today, topics, user};
    use super::super::types::{Drafts, ExistingIndex, Mappings};

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
            &Mappings {
                topics: &topics(),
                categories: &categories(),
            },
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
    fn a_family_takes_the_first_category_in_file_order_for_any_member_order() {
        for reverse in [false, true] {
            let mut specs = vec![
                member(1, "fam", 9.0, "nlp"),
                member(2, "fam-core", 3.0, "tensor"),
            ];
            if reverse {
                specs.reverse();
            }
            let drafts = run(specs);
            let [(_, entry)] = drafts.entries.as_slice() else {
                panic!("one draft expected");
            };
            assert_eq!(
                entry.category.as_ref().map(ToString::to_string).as_deref(),
                Some("zeta-tools")
            );
        }
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
        assert_eq!(crates, ["fam-core", "fam-facade"]);
        assert_eq!(entry.problems(), Vec::<String>::new());
    }

    /// A candle-shaped family: an examples crate on top of a core crate, and a fork.
    fn examples_over_core() -> Vec<Spec> {
        let mut core = member(1, "fam-core", 300.0, "nlp");
        core.description = Some("Core types.".to_owned());
        let mut examples = member(2, "fam-examples", 1.0, "nlp");
        examples.description = Some("Examples.".to_owned());
        examples.deps = vec![(1, DependencyKind::Normal), (3, DependencyKind::Normal)];
        let mut nn = member(3, "fam-nn", 200.0, "nlp");
        nn.description = Some("Layers.".to_owned());
        nn.deps = vec![(1, DependencyKind::Normal)];
        let mut fork = member(4, "fork-fam-core", 0.0, "tensor");
        fork.description = Some("A fork.".to_owned());
        fork.owners = vec![user(9)];
        vec![examples, core, nn, fork]
    }

    #[test]
    fn crates_description_source_and_primary_stay_in_step() {
        let drafts = run(examples_over_core());
        let [(slug, entry)] = drafts.entries.as_slice() else {
            panic!("one draft expected");
        };
        assert_eq!(slug.as_str(), "fam");
        assert_eq!(entry.crates[0].as_str(), "fam-core");
        assert_eq!(entry.description.text, "Core types.");
        assert_eq!(
            entry.description.source_url.as_ref().map(Url::as_str),
            Some("https://crates.io/crates/fam-core")
        );
    }

    #[test]
    fn a_crate_of_another_owner_is_named_and_left_out() {
        let drafts = run(examples_over_core());
        let [(_, entry)] = drafts.entries.as_slice() else {
            panic!("one draft expected");
        };
        let crates: Vec<&str> = entry.crates.iter().map(CrateName::as_str).collect();
        assert_eq!(crates, ["fam-core", "fam-examples", "fam-nn"]);
        assert_eq!(
            skipped(&drafts),
            [(
                "fork-fam-core",
                "shares no owner with `fam-core`, so left out of the family"
            )]
        );
    }

    #[test]
    fn a_left_out_crate_gives_the_entry_no_topic() {
        let drafts = run(examples_over_core());
        let found: Vec<&str> = drafts.entries[0]
            .1
            .topics
            .iter()
            .map(|t| t.as_str())
            .collect();
        assert_eq!(found, ["nlp"]);
    }

    #[test]
    fn a_contributor_crate_on_the_primary_stays_in_the_family() {
        let mut contributor = member(2, "fam-extra", 1.0, "nlp");
        contributor.owners = vec![user(9)];
        contributor.deps = vec![(1, DependencyKind::Normal)];
        let drafts = run(vec![member(1, "fam", 5.0, "nlp"), contributor]);
        assert!(drafts.skipped.is_empty(), "{:?}", drafts.skipped);
        let crates: Vec<&str> = drafts.entries[0]
            .1
            .crates
            .iter()
            .map(CrateName::as_str)
            .collect();
        assert_eq!(crates, ["fam", "fam-extra"]);
    }

    #[test]
    fn a_fork_with_users_does_not_lift_a_project_over_the_floor() {
        let mut fork = member(2, "fork-fam", 90.0, "nlp");
        fork.owners = vec![user(9)];
        let drafts = run(vec![member(1, "fam", 0.0, "nlp"), fork]);
        assert!(drafts.entries.is_empty(), "{:?}", drafts.entries);
        assert!(drafts.skipped.is_empty(), "{:?}", drafts.skipped);
    }

    #[test]
    fn a_term_maps_a_crate_when_no_keyword_does() {
        let mut spec = member(1, "fam", 5.0, "zip");
        spec.description = Some("A Sparse-Matrix library.".to_owned());
        let drafts = run(vec![spec]);
        let [(_, entry)] = drafts.entries.as_slice() else {
            panic!("one draft expected, skipped {:?}", drafts.skipped);
        };
        let found: Vec<&str> = entry.topics.iter().map(|t| t.as_str()).collect();
        assert_eq!(found, ["foundations"]);
    }

    #[test]
    fn a_keyword_match_stops_the_term_step() {
        let mut spec = member(1, "fam", 5.0, "nlp");
        spec.description = Some("A sparse matrix library.".to_owned());
        let drafts = run(vec![spec]);
        let found: Vec<&str> = drafts.entries[0]
            .1
            .topics
            .iter()
            .map(|t| t.as_str())
            .collect();
        assert_eq!(found, ["nlp"]);
    }

    #[test]
    fn terms_read_the_primary_and_not_the_other_members() {
        let primary = member(1, "fam", 5.0, "zip");
        let mut other = member(2, "fam-x", 1.0, "zip");
        other.description = Some("A sparse matrix library.".to_owned());
        let drafts = run(vec![primary, other]);
        assert!(drafts.entries.is_empty());
        assert_eq!(skipped(&drafts), [("fam", "no topic matches")]);
    }

    #[test]
    fn a_long_description_drafts_from_its_leading_sentences() {
        let mut spec = member(1, "fam", 5.0, "nlp");
        spec.description = Some(format!(
            "Reads the format. {}.",
            "Word ".repeat(70).trim_end()
        ));
        let drafts = run(vec![spec]);
        assert_eq!(drafts.entries[0].1.description.text, "Reads the format.");
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
