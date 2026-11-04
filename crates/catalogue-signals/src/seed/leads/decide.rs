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

/// The name GitHub gives the primary language of a Rust repo.
const RUST: &str = "Rust";

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
pub(super) fn decide(ctx: &Context<'_>, lead: &RepoRef, profile: &RepoProfile) -> Decision {
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
    let claimed = !members.is_empty();
    let found = family(inputs.snapshot, &keys[0], members, 0.0);
    if !is_rust(profile) && !has_rust_named_crate(found.as_ref(), &canonical) {
        return Decision::alone(Outcome::Skip(language_reason(profile)));
    }
    let left_out = found
        .as_ref()
        .map_or_else(Vec::new, |family| family.left_out.clone());
    let outcome = match &found {
        None if claimed => Outcome::Skip(
            "every crate that claims the repository is deprecated, yanked or has no default version"
                .to_owned(),
        ),
        found => draft(ctx, canonical, keys, profile, found.as_ref()),
    };
    Decision { outcome, left_out }
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

fn is_rust(profile: &RepoProfile) -> bool {
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
        let world = World::new(specs);
        let keys = HashSet::from(["github.com/o/proj".to_owned()]);
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
        decide(&ctx, &RepoRef::parse("o/proj").expect("lead"), found)
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
    fn a_name_github_gives_that_is_not_owner_and_name_is_skipped() {
        let mut bad = profile(Some("Rust"));
        bad.name_with_owner = "nope".to_owned();
        let decision = run(Vec::new(), &ExistingIndex::default(), &bad);
        assert!(skip_reason(&decision).contains("`nope`"));
    }
}
