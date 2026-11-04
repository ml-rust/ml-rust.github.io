//! The run over one lead list.

use std::collections::{BTreeMap, HashMap, HashSet};

use catalogue_core::{Date, EntryKind, Slug};

use super::super::related::set_part_of;
use super::channels::lookup_channels;
use super::claims::Claims;
use super::decide::{Context, Drafted, Outcome, decide, github_key};
use super::resolve::{Answer, resolve};
use super::types::{DraftDetail, LeadDrafts, LeadInputs, LeadPacers};
use crate::dump::utc_date;
use crate::github::{GithubError, RepoRef};
use crate::http::{Headers, Transport};
use crate::usage;

/// Drafts one entry per lead.
///
/// The run resolves every lead with one paced GitHub batch and reads the dump once. Then it
/// decides each lead in list order and looks up the channels of each app draft. The `added` date
/// is the date of the dump, as in the dump flow. A repo whose primary language is not Rust adds
/// one language list request. Two leads that resolve to one repository make one draft from the
/// claims of both names, and the first of two drafts with one slug keeps it.
///
/// A lead that yields no draft appears in `drafts.skipped` with the reason. So does a lead whose
/// repository `excluded_repos` lists. A lead an entry covers appears in `listed`.
///
/// # Errors
///
/// Returns the [`GithubError`] of a 401 answer, which stops the run. Every other failed fetch
/// stays on its own lead.
pub fn seed_leads(
    inputs: &LeadInputs<'_>,
    leads: &[RepoRef],
    transport: &dyn Transport,
    token: &str,
    pacers: &mut LeadPacers<'_>,
) -> Result<LeadDrafts, GithubError> {
    let mut out = LeadDrafts::default();
    let unique = unique_leads(leads, &mut out.drafts.skipped);
    let answers = resolve(transport, token, &unique, &mut *pacers.github)?;

    let claims = Claims::of(inputs.snapshot, &wanted_keys(&unique, &answers));
    let today = utc_date(inputs.snapshot.meta.timestamp);
    let signals = usage::compute(inputs.snapshot, &claims.ids(), today);
    let ctx = Context {
        inputs,
        claims: &claims,
        usage: &signals,
        added: Date::from(today),
    };

    let spellings = spellings_by_repo(&unique, &answers);
    let mut pending: Vec<(String, Drafted)> = Vec::new();
    let mut resolved_by: HashMap<String, String> = HashMap::new();
    let mut taken: BTreeMap<Slug, String> = BTreeMap::new();
    for (lead, answer) in unique.iter().zip(&answers) {
        let name = lead.to_string();
        let found = match answer {
            Answer::Found(found) => found,
            Answer::Gone => {
                let reason = "GitHub holds no repository of this name".to_owned();
                out.drafts.skipped.push((name, reason));
                continue;
            }
            Answer::Failed(error) => {
                let reason = format!("fetch failed: {error}");
                out.drafts.skipped.push((name, reason));
                continue;
            }
        };
        out.warnings.extend(found.warning.iter().cloned());
        let key = found.profile.name_with_owner.to_lowercase();
        if let Some(first) = resolved_by.get(&key) {
            let reason = format!("resolves to `{key}`, the same repository as lead `{first}`");
            out.drafts.skipped.push((name, reason));
            continue;
        }
        let names = spellings.get(&key).map_or(&[][..], Vec::as_slice);
        resolved_by.insert(key, name.clone());
        let decision = decide(&ctx, names, &found.profile, found.lists_rust);
        out.drafts.skipped.extend(decision.left_out);
        match decision.outcome {
            Outcome::Listed(text) => out.listed.push((name, text)),
            Outcome::Skip(reason) => out.drafts.skipped.push((name, reason)),
            Outcome::Draft(drafted) => {
                let slug = drafted.slug.clone();
                if inputs.existing.has_slug(&slug) {
                    let reason = format!("slug `{slug}` is already an entry");
                    out.drafts.skipped.push((name, reason));
                } else if let Some(first) = taken.get(&slug) {
                    let reason = format!("slug `{slug}` is already drafted for `{first}`");
                    out.drafts.skipped.push((name, reason));
                } else {
                    taken.insert(slug, name.clone());
                    pending.push((name, *drafted));
                }
            }
        }
    }
    finish(&mut out, pending, transport, pacers);
    set_part_of(inputs.snapshot, inputs.existing, &mut out.drafts.entries);
    out.drafts.entries.sort_by(|a, b| a.0.cmp(&b.0));
    out.drafts.skipped.sort();
    out.listed.sort();
    Ok(out)
}

/// The leads without repeats, in list order, ignoring case. A repeat goes to `skipped`.
fn unique_leads(leads: &[RepoRef], skipped: &mut Vec<(String, String)>) -> Vec<RepoRef> {
    let mut seen = HashSet::new();
    let mut unique = Vec::with_capacity(leads.len());
    for lead in leads {
        if seen.insert(github_key(lead)) {
            unique.push(lead.clone());
        } else {
            skipped.push((
                lead.to_string(),
                "repeats an earlier line of the lead list".to_owned(),
            ));
        }
    }
    unique
}

/// The keys of the leads that resolve to each repository, by lowercase `owner/name`.
fn spellings_by_repo(leads: &[RepoRef], answers: &[Answer]) -> HashMap<String, Vec<String>> {
    let mut spellings: HashMap<String, Vec<String>> = HashMap::new();
    for (lead, answer) in leads.iter().zip(answers) {
        if let Answer::Found(found) = answer {
            spellings
                .entry(found.profile.name_with_owner.to_lowercase())
                .or_default()
                .push(github_key(lead));
        }
    }
    spellings
}

/// The keys of every lead and of every canonical name.
fn wanted_keys(leads: &[RepoRef], answers: &[Answer]) -> HashSet<String> {
    let mut wanted: HashSet<String> = leads.iter().map(github_key).collect();
    for answer in answers {
        if let Answer::Found(found) = answer
            && let Ok(canonical) = RepoRef::parse(&found.profile.name_with_owner)
        {
            wanted.insert(github_key(&canonical));
        }
    }
    wanted
}

/// Looks up the channels of each app draft and moves every draft into `out`.
fn finish(
    out: &mut LeadDrafts,
    pending: Vec<(String, Drafted)>,
    transport: &dyn Transport,
    pacers: &mut LeadPacers<'_>,
) {
    let mut asked = false;
    for (lead, drafted) in pending {
        let Drafted {
            slug,
            mut entry,
            users,
            mut notes,
            canonical,
            keys,
        } = drafted;
        if entry.kind == EntryKind::App {
            if asked {
                let headers = Headers::default();
                pacers.homebrew.wait(&headers);
                pacers.repology.wait(&headers);
                pacers.docker.wait(&headers);
            }
            asked = true;
            let found = lookup_channels(transport, &canonical, &keys, pacers);
            entry.distribution = found.distribution;
            notes.extend(found.notes);
            out.warnings.extend(found.warnings);
        }
        out.details
            .insert(slug.clone(), DraftDetail { lead, users, notes });
        out.drafts.entries.push((slug, entry));
    }
}

#[cfg(test)]
mod tests {
    use super::super::resolve::Found;
    use super::*;
    use crate::github::RepoProfile;

    fn found(name_with_owner: &str) -> Answer {
        Answer::Found(Found {
            profile: RepoProfile {
                name_with_owner: name_with_owner.to_owned(),
                archived: false,
                fork: false,
                description: None,
                homepage: None,
                topics: Vec::new(),
                language: None,
                license: None,
            },
            lists_rust: false,
            warning: None,
        })
    }

    fn lead(text: &str) -> RepoRef {
        RepoRef::parse(text).expect("lead")
    }

    #[test]
    fn leads_of_one_repository_share_their_spellings_in_either_order() {
        let leads = [
            lead("chelsea0x3b/dfdx"),
            lead("coreylowman/dfdx"),
            lead("acme/gone"),
        ];
        let answers = [
            found("chelsea0x3b/dfdx"),
            found("Chelsea0x3b/dfdx"),
            Answer::Gone,
        ];
        let spellings = spellings_by_repo(&leads, &answers);
        assert_eq!(spellings.len(), 1);
        assert_eq!(
            spellings["chelsea0x3b/dfdx"],
            ["github.com/chelsea0x3b/dfdx", "github.com/coreylowman/dfdx"]
        );
    }
}
