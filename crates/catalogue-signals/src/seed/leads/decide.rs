//! What one resolved lead becomes.

use catalogue_core::{Date, DescriptionSource, Entry, EntryKind, Slug};

use super::super::family::{Family, family};
use super::super::owners::Member;
use super::super::primary::RepoName;
use super::claims::Claims;
use super::entry::build_entry;
use super::kind::{Kind, classify};
use super::types::LeadInputs;
use crate::discover::Candidate;
use crate::dump::IdMap;
use crate::github::{RepoProfile, RepoRef};
use crate::usage::Usage;

/// The name GitHub gives the Rust language.
pub(super) const RUST: &str = "Rust";

/// What every decision of one run reads.
pub(super) struct Context<'a> {
    pub(super) inputs: &'a LeadInputs<'a>,
    pub(super) claims: &'a Claims,
    pub(super) usage: &'a IdMap<Usage>,
    pub(super) added: Date,
}

/// A lead that yields an entry.
pub(super) struct Drafted {
    pub(super) slug: Slug,
    pub(super) entry: Entry,
    pub(super) users: f64,
    pub(super) notes: Vec<String>,
    pub(super) canonical: RepoRef,
    pub(super) keys: [String; 2],
}

/// What one lead becomes.
pub(super) enum Outcome {
    /// A draft for review.
    Draft(Box<Drafted>),
    /// An existing entry covers the lead. The text says how.
    Listed(String),
    /// No draft. The text is the reason.
    Skip(String),
}

/// The outcome of one lead and the crates that claim its repository and stay out.
pub(super) struct Decision {
    pub(super) outcome: Outcome,
    pub(super) left_out: Vec<(String, String)>,
}

impl Decision {
    fn alone(outcome: Outcome) -> Self {
        Self {
            outcome,
            left_out: Vec::new(),
        }
    }
}

/// The normalised key of the GitHub repository `repo`.
pub(super) fn github_key(repo: &RepoRef) -> String {
    format!("github.com/{repo}").to_lowercase()
}

/// Decides what the lead `lead` with the profile `profile` becomes.
///
/// The order: an entry that holds the repository, a fork, the language rule, an entry that lists
/// a family crate, then the draft. The crates that claim the canonical repository or the name in
/// the lead form the family, with no discovery rule and no active-user floor.
///
/// The language rule passes a repo whose primary language is Rust, a repo with a Rust-named
/// family, and a repo with no family when `lists_rust` says GitHub lists Rust among its
/// languages. The primary language alone misjudges a Rust repo that also holds many scripts. The
/// last case makes an app draft with the label `language-list:rust`.
pub(super) fn decide(
    ctx: &Context<'_>,
    lead: &RepoRef,
    profile: &RepoProfile,
    lists_rust: bool,
) -> Decision {
    let Ok(canonical) = RepoRef::parse(&profile.name_with_owner) else {
        return Decision::alone(Outcome::Skip(format!(
            "GitHub names the repository `{}`, which is not owner/name",
            profile.name_with_owner
        )));
    };
    let inputs = ctx.inputs;
    let keys = [github_key(&canonical), github_key(lead)];
    if let Some(slug) = keys.iter().find_map(|key| inputs.existing.repo_slug(key)) {
        return Decision::alone(Outcome::Listed(format!(
            "entry `{slug}` holds the repository"
        )));
    }
    if profile.fork {
        return Decision::alone(Outcome::Skip("the repository is a fork".to_owned()));
    }
    let members: Vec<Member<'_>> = ctx
        .claims
        .of_keys(&keys)
        .into_iter()
        .map(|candidate| (candidate, users_of(ctx, candidate)))
        .collect();
    let (members, unnamed) = split_unnamed(&canonical, members);
    let claimed = !members.is_empty();
    let found = family(inputs.snapshot, &keys[0], members, 0.0);
    let rust_listed = lists_rust && found.is_none();
    if !is_rust(profile) && !rust_listed && !has_rust_named_crate(found.as_ref(), &canonical) {
        return Decision::alone(Outcome::Skip(language_reason(profile)));
    }
    let mut left_out = found
        .as_ref()
        .map_or_else(Vec::new, |family| family.left_out.clone());
    left_out.extend(unnamed);
    let mut outcome = match &found {
        None if claimed => Outcome::Skip(
            "every crate that claims the repository is deprecated, yanked or has no default version"
                .to_owned(),
        ),
        found => draft(ctx, canonical, keys, profile, found.as_ref()),
    };
    if rust_listed
        && !is_rust(profile)
        && let Outcome::Draft(drafted) = &mut outcome
    {
        drafted.notes.push("language-list:rust".to_owned());
    }
    Decision { outcome, left_out }
}

/// Splits the crates that claim a repository into the ones to keep and the ones to leave out.
///
/// A repository link alone does not make a crate part of a project. When no claiming crate
/// shares a word with the repository name (see [`RepoName::names_repo`]), every claimant is left
/// out with the reason, and the lead drafts as an app with no crates.
fn split_unnamed<'a>(
    canonical: &RepoRef,
    members: Vec<Member<'a>>,
) -> (Vec<Member<'a>>, Vec<(String, String)>) {
    let repo = RepoName::new(&canonical.name);
    if members
        .iter()
        .any(|(candidate, _)| repo.names_repo(&candidate.name))
    {
        return (members, Vec::new());
    }
    let left_out = members
        .iter()
        .map(|(candidate, _)| {
            (
                candidate.name.clone(),
                format!(
                    "name shares no word with repository `{}`, so no crate joins the draft",
                    canonical.name
                ),
            )
        })
        .collect();
    (Vec::new(), left_out)
}

fn draft(
    ctx: &Context<'_>,
    canonical: RepoRef,
    keys: [String; 2],
    profile: &RepoProfile,
    found: Option<&Family<'_>>,
) -> Outcome {
    let inputs = ctx.inputs;
    let listed = found.and_then(|family| {
        family
            .members()
            .find(|member| inputs.existing.lists_crate(&member.name))
    });
    if let Some(member) = listed {
        return Outcome::Listed(format!("crate `{}` is in an existing entry", member.name));
    }
    let kind = found.map_or(Kind::NO_CRATE, |family| classify(inputs.snapshot, family));
    let built = build_entry(
        inputs.snapshot,
        &inputs.mappings,
        &canonical,
        profile,
        found,
        kind.entry,
        ctx.added,
    );
    match built {
        Ok((slug, entry)) => {
            let users = found.map_or(0.0, |family| family.users);
            let notes = notes(kind, users, inputs.min_active_users, profile, &entry);
            Outcome::Draft(Box::new(Drafted {
                slug,
                entry,
                users,
                notes,
                canonical,
                keys,
            }))
        }
        Err(reason) => Outcome::Skip(reason),
    }
}

fn users_of(ctx: &Context<'_>, candidate: &Candidate) -> f64 {
    ctx.usage
        .get(&candidate.crate_id)
        .map_or(0.0, |usage| usage.current)
}

/// Whether GitHub names Rust as the primary language of the repo.
pub(super) fn is_rust(profile: &RepoProfile) -> bool {
    profile
        .language
        .as_deref()
        .is_some_and(|language| language.eq_ignore_ascii_case(RUST))
}

fn has_rust_named_crate(found: Option<&Family<'_>>, canonical: &RepoRef) -> bool {
    let repo = RepoName::new(&canonical.name);
    found.is_some_and(|family| {
        family
            .members()
            .any(|member| repo.is_rust_named(&member.name))
    })
}

fn language_reason(profile: &RepoProfile) -> String {
    let language = profile
        .language
        .as_deref()
        .map_or_else(|| "unknown".to_owned(), |language| format!("`{language}`"));
    format!("primary language is {language}, and no Rust-named crate family claims the repository")
}

/// The labels a reviewer reads beside a draft.
fn notes(kind: Kind, users: f64, floor: f64, profile: &RepoProfile, entry: &Entry) -> Vec<String> {
    let mut notes = vec![kind.basis.label().to_owned()];
    if users < floor {
        notes.push("below-floor".to_owned());
    }
    if entry.kind == EntryKind::Library && entry.description.source == DescriptionSource::Repo {
        notes.push("description:repo".to_owned());
    }
    if profile.archived {
        notes.push("archived".to_owned());
    }
    if !is_rust(profile)
        && let Some(language) = profile.language.as_deref()
    {
        notes.push(format!("language:{language}"));
    }
    if let Some(license) = profile.license.as_deref() {
        notes.push(format!("license:{license}"));
    }
    notes
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::super::super::fixtures::{Spec, World, categories, topics};
    use super::super::super::types::{ExistingIndex, Mappings};
    use super::*;

    fn profile(language: Option<&str>) -> RepoProfile {
        RepoProfile {
            name_with_owner: "o/proj".to_owned(),
            archived: false,
            fork: false,
            description: Some("A sparse matrix tool.".to_owned()),
            homepage: None,
            topics: Vec::new(),
            language: language.map(str::to_owned),
            license: Some("MIT".to_owned()),
        }
    }

    fn run(specs: Vec<Spec>, existing: &ExistingIndex, found: &RepoProfile) -> Decision {
        decide_lead("o/proj", specs, existing, found, false)
    }

    fn run_for(lead: &str, specs: Vec<Spec>, found: &RepoProfile) -> Decision {
        let mut found = found.clone();
        found.name_with_owner = lead.to_owned();
        decide_lead(lead, specs, &ExistingIndex::default(), &found, false)
    }

    fn listing_rust(specs: Vec<Spec>, found: &RepoProfile) -> Decision {
        decide_lead("o/proj", specs, &ExistingIndex::default(), found, true)
    }

    fn decide_lead(
        lead: &str,
        specs: Vec<Spec>,
        existing: &ExistingIndex,
        found: &RepoProfile,
        lists_rust: bool,
    ) -> Decision {
        let world = World::new(specs);
        let keys = HashSet::from([format!("github.com/{}", lead.to_lowercase())]);
        let claims = Claims::of(&world.snapshot, &keys);
        let topics = topics();
        let categories = categories();
        let inputs = LeadInputs {
            snapshot: &world.snapshot,
            mappings: Mappings {
                topics: &topics,
                categories: &categories,
            },
            existing,
            min_active_users: 2.0,
        };
        let ctx = Context {
            inputs: &inputs,
            claims: &claims,
            usage: &world.usage,
            added: Date::new(2026, 11, 3).expect("date"),
        };
        decide(
            &ctx,
            &RepoRef::parse(lead).expect("lead"),
            found,
            lists_rust,
        )
    }

    fn claim(id: u32, name: &'static str) -> Spec {
        let mut spec = Spec::new(id, name);
        spec.repository = Some("https://github.com/o/proj".to_owned());
        spec.users = 0.0;
        spec
    }

    fn skip_reason(decision: &Decision) -> &str {
        match &decision.outcome {
            Outcome::Skip(reason) => reason,
            Outcome::Listed(text) => panic!("listed: {text}"),
            Outcome::Draft(drafted) => panic!("draft: {}", drafted.slug),
        }
    }

    fn drafted(decision: &Decision) -> &Drafted {
        match &decision.outcome {
            Outcome::Draft(drafted) => drafted,
            Outcome::Skip(reason) | Outcome::Listed(reason) => panic!("no draft: {reason}"),
        }
    }

    #[test]
    fn a_small_family_drafts_and_says_it_is_below_the_floor() {
        let decision = run(
            vec![claim(1, "proj")],
            &ExistingIndex::default(),
            &profile(Some("Rust")),
        );
        let found = drafted(&decision);
        assert_eq!(found.entry.kind, EntryKind::Library);
        assert_eq!(found.notes, ["basis:has_lib", "below-floor", "license:MIT"]);
    }

    #[test]
    fn a_repo_in_another_language_needs_a_rust_named_family() {
        let reason = run(Vec::new(), &ExistingIndex::default(), &profile(Some("C++"))).outcome;
        let Outcome::Skip(reason) = reason else {
            panic!("expected a skip");
        };
        assert!(reason.contains("`C++`"), "{reason}");
        let foreign = run(
            vec![claim(1, "unrelated")],
            &ExistingIndex::default(),
            &profile(Some("C++")),
        );
        assert!(skip_reason(&foreign).contains("Rust-named"));
        let named = run(
            vec![claim(1, "proj-sys")],
            &ExistingIndex::default(),
            &profile(Some("C++")),
        );
        assert!(drafted(&named).notes.contains(&"language:C++".to_owned()));
    }

    #[test]
    fn a_repo_without_a_language_is_not_rust() {
        let decision = run(Vec::new(), &ExistingIndex::default(), &profile(None));
        assert!(skip_reason(&decision).contains("unknown"));
    }

    #[test]
    fn a_repo_that_lists_rust_with_no_crate_drafts_as_an_app() {
        let decision = listing_rust(Vec::new(), &profile(Some("Shell")));
        let found = drafted(&decision);
        assert_eq!(found.entry.kind, EntryKind::App);
        assert!(found.entry.crates.is_empty());
        assert!(found.notes.contains(&"basis:no-crate".to_owned()));
        assert!(found.notes.contains(&"language:Shell".to_owned()));
        assert!(found.notes.contains(&"language-list:rust".to_owned()));
    }

    #[test]
    fn a_language_list_does_not_lift_a_family_of_dead_crates() {
        let mut dead = claim(1, "proj");
        dead.yanked = true;
        let decision = listing_rust(vec![dead], &profile(Some("Shell")));
        assert!(skip_reason(&decision).contains("every crate"));
    }

    #[test]
    fn a_rust_repo_carries_no_language_list_label() {
        let decision = listing_rust(Vec::new(), &profile(Some("Rust")));
        assert!(
            !drafted(&decision)
                .notes
                .contains(&"language-list:rust".to_owned())
        );
    }

    #[test]
    fn a_family_with_a_listed_crate_is_listed() {
        let existing = ExistingIndex::new(["Proj"], []);
        let decision = run(vec![claim(1, "proj")], &existing, &profile(Some("Rust")));
        let Outcome::Listed(text) = decision.outcome else {
            panic!("expected listed");
        };
        assert!(text.contains("`proj`"), "{text}");
    }

    #[test]
    fn an_entry_on_the_repository_is_listed() {
        let existing = ExistingIndex::default().with_repo("github.com/o/proj", "proj-entry");
        let decision = run(Vec::new(), &existing, &profile(Some("Rust")));
        let Outcome::Listed(text) = decision.outcome else {
            panic!("expected listed");
        };
        assert!(text.contains("`proj-entry`"), "{text}");
    }

    #[test]
    fn a_family_of_dead_crates_is_skipped_and_never_an_app() {
        let mut dead = claim(1, "proj");
        dead.yanked = true;
        let decision = run(
            vec![dead],
            &ExistingIndex::default(),
            &profile(Some("Rust")),
        );
        assert!(skip_reason(&decision).contains("every crate"));
    }

    #[test]
    fn a_crate_of_another_owner_stays_out_of_the_draft() {
        let mut stray = claim(2, "stray");
        stray.owners = vec![super::super::super::fixtures::user(9)];
        let decision = run(
            vec![claim(1, "proj"), stray],
            &ExistingIndex::default(),
            &profile(Some("Rust")),
        );
        assert_eq!(drafted(&decision).entry.crates.len(), 1);
        assert_eq!(decision.left_out.len(), 1);
    }

    #[test]
    fn a_lead_whose_only_claimant_shares_no_name_word_drafts_as_an_app_with_no_crates() {
        let world_claim = |name: &'static str| {
            let mut spec = Spec::new(1, name);
            spec.repository = Some("https://github.com/search/engine".to_owned());
            spec.users = 0.0;
            spec
        };
        let decision = run_for(
            "search/engine",
            vec![world_claim("unrelated")],
            &profile(Some("Rust")),
        );
        let found = drafted(&decision);
        assert_eq!(found.entry.kind, EntryKind::App);
        assert!(found.entry.crates.is_empty());
        assert!(found.notes.contains(&"basis:no-crate".to_owned()));
        let [(name, reason)] = decision.left_out.as_slice() else {
            panic!("one crate left out");
        };
        assert_eq!(name, "unrelated");
        assert!(reason.contains("shares no word"), "{reason}");
    }

    #[test]
    fn a_marker_or_lib_prefix_in_a_crate_name_keeps_a_library() {
        for (repo, name) in [("foo-rs", "foo"), ("oxen", "liboxen")] {
            let mut spec = Spec::new(1, name);
            spec.repository = Some(format!("https://github.com/o/{repo}"));
            spec.users = 0.0;
            let decision = run_for(&format!("o/{repo}"), vec![spec], &profile(Some("Rust")));
            let found = drafted(&decision);
            assert_eq!(found.entry.kind, EntryKind::Library, "{repo} {name}");
            assert_eq!(found.entry.crates.len(), 1, "{repo} {name}");
        }
    }

    #[test]
    fn a_name_github_gives_that_is_not_owner_and_name_is_skipped() {
        let mut bad = profile(Some("Rust"));
        bad.name_with_owner = "nope".to_owned();
        let decision = run(Vec::new(), &ExistingIndex::default(), &bad);
        assert!(skip_reason(&decision).contains("`nope`"));
    }
}
