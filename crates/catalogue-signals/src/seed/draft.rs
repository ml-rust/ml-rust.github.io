//! The drafting of one entry per eligible repository.

use std::collections::BTreeMap;

use catalogue_core::{Date, Slug, normalize_repo_url};

use super::family::families;
use super::family_draft::draft_family;
use super::related::set_part_of;
use super::types::{Drafts, ExistingIndex, Mappings};
use crate::discover::Candidate;
use crate::dump::{DumpSnapshot, IdMap};
use crate::usage::Usage;

/// Drafts one entry for each family of eligible crates.
///
/// A crate is eligible when its active users reach `min_active_users`, no
/// existing entry lists it, its default version is not yanked and it carries no
/// deprecation notice. A crate without usage counts as having no users.
/// Eligible crates whose repository links normalise to the same
/// `host/owner/repo` and that share owners form one family. The primary crate
/// is the main crate of the repository: the one named like the repository, else
/// the crate of the family's dependency tree that connects to the most members
/// (see `families`). A family is eligible when it has a primary. A family member
/// that is yanked or deprecated, or that shares no owner with the family, is
/// left out of the entry and appears in [`Drafts::skipped`] with the reason. A
/// crate that fails the user count or is already listed is dropped without a
/// report.
///
/// Each draft is a library. Its name and slug come from the last segment of the
/// repository link, the project name, when that differs from the primary
/// crate's name and canonicalises to a valid slug. Otherwise they come from the
/// primary crate. Of two families with one slug, the family with more active
/// users keeps it. Its
/// crates are the primary, then the other members sorted by name, so
/// `crates[0]` is always the primary. Its
/// repository is the normalised `https://<host>/<owner>/<repo>` of the
/// primary. Its description is the crates.io description of the primary, or of
/// the next member with a usable one, cut to whole sentences when longer than
/// 300 characters. Its topics are the union of the topics whose keywords or
/// categories match any member. When that union is empty, they are the topics
/// with a term in the primary's name or description. Its category comes from
/// the first category in file order with a term in that text. When none has one,
/// it comes from the first category whose keywords or slugs match any member.
/// No match leaves the category unset and keeps the draft.
///
/// Any https host is accepted. GitHub-only signals stay unknown for other
/// hosts, so such entries can be Listed but not Featured. A family whose
/// primary lacks an https repository with an owner and a name, whose members
/// all lack a usable description, that matches no topic, whose repository an
/// entry holds or that has an invalid or taken slug appears in
/// [`Drafts::skipped`] under the primary's name with the reason.
///
/// A draft gets `part_of` only with evidence: a name match and a shared crate owner.
pub fn draft_entries(
    snapshot: &DumpSnapshot,
    candidates: &[Candidate],
    usage: &IdMap<Usage>,
    mappings: &Mappings<'_>,
    existing: &ExistingIndex,
    min_active_users: f64,
    today: Date,
) -> Drafts {
    let mut drafts = Drafts::default();
    let mut taken: BTreeMap<Slug, &str> = BTreeMap::new();
    for family in families(snapshot, candidates, usage, existing, min_active_users) {
        drafts.skipped.extend(family.left_out.iter().cloned());
        let outcome = draft_family(snapshot, &family, mappings, today).and_then(|(slug, entry)| {
            let holder = normalize_repo_url(entry.repo.as_str())
                .and_then(|key| existing.repo_slug(&key).map(str::to_owned));
            if let Some(holder) = holder {
                return Err(format!("entry `{holder}` holds the repository"));
            }
            if existing.has_slug(&slug) {
                return Err(format!("slug `{slug}` is already an entry"));
            }
            match taken.get(&slug) {
                Some(first) => Err(format!("slug `{slug}` is already drafted for `{first}`")),
                None => Ok((slug, entry)),
            }
        });
        match outcome {
            Ok((slug, entry)) => {
                let primary: &Candidate = family.primary;
                taken.insert(slug.clone(), &primary.name);
                drafts.entries.push((slug, entry));
            }
            Err(reason) => drafts.skipped.push((family.primary.name.clone(), reason)),
        }
    }
    set_part_of(snapshot, existing, &mut drafts.entries);
    drafts.entries.sort_by(|a, b| a.0.cmp(&b.0));
    drafts.skipped.sort();
    drafts
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Categories, CrateName, DescriptionSource, EntryKind, Topics};
    use url::Url;

    use super::super::fixtures::{Spec, World, categories, today, topics};
    use super::*;

    fn run(world: &World, existing: &ExistingIndex, min: f64) -> Drafts {
        draft_entries(
            &world.snapshot,
            &world.candidates,
            &world.usage,
            &Mappings {
                topics: &topics(),
                categories: &categories(),
            },
            existing,
            min,
            today(),
        )
    }

    fn plain(world: &World) -> Drafts {
        run(world, &ExistingIndex::default(), 2.0)
    }

    fn skipped(drafts: &Drafts) -> Vec<(&str, &str)> {
        drafts
            .skipped
            .iter()
            .map(|(name, reason)| (name.as_str(), reason.as_str()))
            .collect()
    }

    fn slugs(drafts: &Drafts) -> Vec<&str> {
        drafts
            .entries
            .iter()
            .map(|(slug, _)| slug.as_str())
            .collect()
    }

    #[test]
    fn a_draft_fills_every_field() {
        let mut spec = Spec::new(1, "Tok_Lib");
        spec.homepage = Some("https://tok.example.com");
        spec.repository = Some("https://github.com/ML-Rust/tok-lib.git".to_owned());
        spec.keywords = vec!["NLP", "tensor"];
        spec.description = Some("  Fast\n tokenizer. ".to_owned());
        let drafts = plain(&World::new(vec![spec]));
        assert!(drafts.skipped.is_empty(), "{:?}", drafts.skipped);
        let [(slug, entry)] = drafts.entries.as_slice() else {
            panic!("one draft expected");
        };
        assert_eq!(slug.as_str(), "tok-lib");
        assert_eq!(entry.name, "Tok_Lib");
        assert_eq!(entry.kind, EntryKind::Library);
        assert_eq!(entry.repo.as_str(), "https://github.com/ml-rust/tok-lib");
        assert_eq!(
            entry.homepage.as_ref().map(Url::as_str),
            Some("https://tok.example.com/")
        );
        let crates: Vec<&str> = entry.crates.iter().map(CrateName::as_str).collect();
        assert_eq!(crates, ["Tok_Lib"]);
        let found: Vec<&str> = entry.topics.iter().map(Slug::as_str).collect();
        assert_eq!(found, ["foundations", "nlp"]);
        assert_eq!(
            entry.category.as_ref().map(Slug::as_str),
            Some("zeta-tools")
        );
        assert_eq!(entry.description.text, "Fast tokenizer.");
        assert_eq!(entry.description.source, DescriptionSource::Crate);
        assert_eq!(
            entry.description.source_url.as_ref().map(Url::as_str),
            Some("https://crates.io/crates/Tok_Lib")
        );
        assert!(entry.org_affiliated);
        assert_eq!(entry.distribution, None);
        assert_eq!(entry.added, today());
        assert_eq!(entry.problems(), Vec::<String>::new());
    }

    #[test]
    fn users_below_the_minimum_are_dropped_silently() {
        let mut low = Spec::new(1, "low");
        low.users = 1.99;
        let mut edge = Spec::new(2, "edge");
        edge.users = 2.0;
        let drafts = plain(&World::new(vec![low, edge]));
        assert_eq!(slugs(&drafts), ["edge"]);
        assert!(drafts.skipped.is_empty());
    }

    #[test]
    fn a_candidate_without_usage_has_no_users() {
        let mut world = World::new(vec![Spec::new(1, "tool")]);
        world.usage.clear();
        assert!(plain(&world).entries.is_empty());
        assert_eq!(run(&world, &ExistingIndex::default(), 0.0).entries.len(), 1);
    }

    #[test]
    fn listed_crates_are_dropped_in_any_spelling() {
        let world = World::new(vec![Spec::new(1, "foo_bar"), Spec::new(2, "baz")]);
        let existing = ExistingIndex::new(["Foo-Bar"], []);
        let drafts = run(&world, &existing, 2.0);
        assert_eq!(slugs(&drafts), ["baz"]);
        assert!(drafts.skipped.is_empty());
    }

    #[test]
    fn yanked_deprecated_and_missing_default_are_dropped() {
        let mut yanked = Spec::new(1, "yanked");
        yanked.yanked = true;
        let mut deprecated = Spec::new(2, "deprecated");
        deprecated.deprecated = true;
        let mut world = World::new(vec![yanked, deprecated, Spec::new(3, "nodefault")]);
        world.snapshot.default_version.remove(&3);
        let drafts = plain(&world);
        assert!(drafts.entries.is_empty());
        assert!(drafts.skipped.is_empty());
    }

    #[test]
    fn repositories_that_do_not_fit_are_skipped_with_a_reason() {
        let repo = |id, name, repository: Option<&str>| {
            let mut spec = Spec::new(id, name);
            spec.repository = repository.map(str::to_owned);
            spec
        };
        let world = World::new(vec![
            repo(1, "a-none", None),
            repo(2, "b-http", Some("http://github.com/o/r")),
            repo(3, "c-ssh", Some("git@github.com:o/r.git")),
            repo(4, "d-short", Some("https://github.com/o")),
        ]);
        let drafts = plain(&world);
        assert!(drafts.entries.is_empty());
        let found = skipped(&drafts);
        assert_eq!(found[0], ("a-none", "no repository"));
        assert!(found[1].1.contains("not an https link"), "{found:?}");
        assert!(found[2].1.contains("not an https link"), "{found:?}");
        assert!(found[3].1.contains("no owner and name"), "{found:?}");
    }

    #[test]
    fn descriptions_that_do_not_fit_are_skipped_with_a_reason() {
        let mut none = Spec::new(1, "a-none");
        none.description = None;
        let mut blank = Spec::new(2, "b-blank");
        blank.description = Some("  ".to_owned());
        let mut over = Spec::new(3, "c-long");
        over.description = Some("x".repeat(301));
        let drafts = plain(&World::new(vec![none, blank, over]));
        assert_eq!(
            skipped(&drafts),
            [
                ("a-none", "no description"),
                ("b-blank", "no description"),
                ("c-long", "description is longer than 300 characters"),
            ]
        );
    }

    #[test]
    fn a_crate_without_a_matching_topic_is_skipped() {
        let mut spec = Spec::new(1, "tool");
        spec.keywords = vec!["zip"];
        let drafts = plain(&World::new(vec![spec]));
        assert_eq!(skipped(&drafts), [("tool", "no topic matches")]);
    }

    fn category_of(spec: Spec) -> Option<String> {
        let drafts = plain(&World::new(vec![spec]));
        assert!(drafts.skipped.is_empty(), "{:?}", drafts.skipped);
        let [(_, entry)] = drafts.entries.as_slice() else {
            panic!("one draft expected");
        };
        entry.category.as_ref().map(ToString::to_string)
    }

    #[test]
    fn no_category_match_keeps_the_draft_with_the_category_unset() {
        let mut spec = Spec::new(1, "tool");
        spec.keywords = Vec::new();
        spec.categories = vec!["science::math"];
        assert_eq!(category_of(spec), None);
    }

    #[test]
    fn the_terms_decide_the_category_when_no_metadata_does() {
        let mut spec = Spec::new(1, "tool");
        spec.keywords = Vec::new();
        spec.description = Some("A sparse matrix tool.".to_owned());
        assert_eq!(category_of(spec).as_deref(), Some("zeta-tools"));
    }

    #[test]
    fn a_term_beats_the_metadata_of_an_earlier_category() {
        let mut spec = Spec::new(1, "tool");
        spec.description = Some("A sparse matrix tool.".to_owned());
        assert_eq!(category_of(spec).as_deref(), Some("zeta-tools"));
    }

    #[test]
    fn an_existing_slug_blocks_the_draft() {
        let world = World::new(vec![Spec::new(1, "tool")]);
        let existing = ExistingIndex::new([], ["tool"]);
        let drafts = run(&world, &existing, 2.0);
        assert!(drafts.entries.is_empty());
        assert_eq!(
            skipped(&drafts),
            [("tool", "slug `tool` is already an entry")]
        );
    }

    #[test]
    fn a_repository_an_entry_holds_blocks_the_draft() {
        let world = World::new(vec![Spec::new(1, "tool")]);
        let existing = ExistingIndex::default().with_repo("github.com/someone/tool", "other");
        let drafts = run(&world, &existing, 2.0);
        assert!(drafts.entries.is_empty());
        assert_eq!(
            skipped(&drafts),
            [("tool", "entry `other` holds the repository")]
        );
    }

    #[test]
    fn a_draft_given_to_the_index_blocks_its_crates_slug_and_repository() {
        let first = plain(&World::new(vec![Spec::new(1, "tool")]));
        let existing = ExistingIndex::default().with_drafts(&first.entries);
        let again = run(&World::new(vec![Spec::new(1, "tool")]), &existing, 2.0);
        assert!(again.entries.is_empty());
        assert!(again.skipped.is_empty(), "{:?}", again.skipped);

        let mut other_crate = Spec::new(2, "tool-extra");
        other_crate.repository = Some("https://github.com/someone/tool".to_owned());
        let drafts = run(&World::new(vec![other_crate]), &existing, 2.0);
        assert_eq!(
            skipped(&drafts),
            [("tool-extra", "entry `tool` holds the repository")]
        );

        let mut same_slug = Spec::new(3, "other-crate");
        same_slug.repository = Some("https://github.com/other/tool".to_owned());
        assert_eq!(
            skipped(&run(&World::new(vec![same_slug]), &existing, 2.0)),
            [("other-crate", "slug `tool` is already an entry")]
        );
    }

    #[test]
    fn a_crate_on_another_host_drafts_with_that_link() {
        let mut spec = Spec::new(1, "whisper-rs");
        spec.repository = Some("https://codeberg.org/tazz4843/whisper-rs".to_owned());
        spec.keywords = Vec::new();
        spec.description = Some("Rust bindings for whisper.cpp".to_owned());
        let world = World::new(vec![spec]);
        let topics: Topics =
            toml::from_str(include_str!("../../../../data/topics.toml")).expect("topics parse");
        let categories: Categories =
            toml::from_str(include_str!("../../../../data/categories.toml"))
                .expect("categories parse");
        let drafts = draft_entries(
            &world.snapshot,
            &world.candidates,
            &world.usage,
            &Mappings {
                topics: &topics,
                categories: &categories,
            },
            &ExistingIndex::default(),
            2.0,
            today(),
        );
        assert!(drafts.skipped.is_empty(), "{:?}", drafts.skipped);
        let [(slug, entry)] = drafts.entries.as_slice() else {
            panic!("one draft expected");
        };
        assert_eq!(slug.as_str(), "whisper-rs");
        assert_eq!(
            entry.repo.as_str(),
            "https://codeberg.org/tazz4843/whisper-rs"
        );
        assert!(!entry.org_affiliated);
        let found: Vec<&str> = entry.topics.iter().map(Slug::as_str).collect();
        assert_eq!(found, ["speech-audio"]);
        assert_eq!(
            entry.category.as_ref().map(Slug::as_str),
            Some("speech-audio")
        );
        assert_eq!(entry.problems(), Vec::<String>::new());
    }

    #[test]
    fn two_drafts_with_one_slug_keep_the_first_by_name() {
        let world = World::new(vec![Spec::new(2, "foo_bar"), Spec::new(1, "foo-bar")]);
        let drafts = plain(&world);
        assert_eq!(slugs(&drafts), ["foo-bar"]);
        assert_eq!(
            skipped(&drafts),
            [("foo_bar", "slug `foo-bar` is already drafted for `foo-bar`")]
        );
    }

    #[test]
    fn of_two_repositories_with_one_slug_the_more_used_keeps_it() {
        let mut small = Spec::new(1, "model");
        small.repository = Some("https://github.com/a/model-rs".to_owned());
        small.users = 2.0;
        let mut large = Spec::new(2, "model-rs");
        large.repository = Some("https://github.com/b/model-rs".to_owned());
        large.users = 41.0;
        let drafts = plain(&World::new(vec![small, large]));
        assert_eq!(slugs(&drafts), ["model-rs"]);
        assert_eq!(drafts.entries[0].1.crates[0].as_str(), "model-rs");
        assert_eq!(
            skipped(&drafts),
            [("model", "slug `model-rs` is already drafted for `model-rs`")]
        );
    }

    #[test]
    fn a_name_that_is_not_a_slug_is_skipped() {
        let world = World::new(vec![Spec::new(1, "a__b")]);
        let drafts = plain(&world);
        assert!(drafts.entries.is_empty());
        let found = skipped(&drafts);
        assert!(found[0].1.contains("not a slug"), "{found:?}");
    }

    #[test]
    fn entries_sort_by_slug_regardless_of_candidate_order() {
        let mut world = World::new(vec![
            Spec::new(1, "zeta"),
            Spec::new(2, "alpha"),
            Spec::new(3, "mid_dle"),
        ]);
        world.candidates.reverse();
        assert_eq!(slugs(&plain(&world)), ["alpha", "mid-dle", "zeta"]);
    }

    #[test]
    fn a_repeated_candidate_drafts_once() {
        let mut world = World::new(vec![Spec::new(1, "tool")]);
        let again = world.candidates.clone();
        world.candidates.extend(again);
        let drafts = plain(&world);
        assert_eq!(slugs(&drafts), ["tool"]);
        assert!(drafts.skipped.is_empty());
    }
}
