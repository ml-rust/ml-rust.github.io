//! The crates of the dump that claim the repositories of a lead run.

use std::collections::{HashMap, HashSet};

use catalogue_core::normalize_repo_url;

use crate::discover::Candidate;
use crate::dump::{DumpSnapshot, IdSet};

/// The crates that claim each wanted repository, by normalised repository link.
pub(super) struct Claims {
    by_repo: HashMap<String, Vec<Candidate>>,
}

impl Claims {
    /// Reads every crate of `snapshot` once and keeps those that claim a repository in `wanted`.
    ///
    /// A claim bypasses the discovery rules, so a candidate carries no reason.
    pub(super) fn of(snapshot: &DumpSnapshot, wanted: &HashSet<String>) -> Self {
        let mut by_repo: HashMap<String, Vec<Candidate>> = HashMap::new();
        for (&crate_id, row) in &snapshot.crates {
            let Some(key) = row.repository.as_deref().and_then(normalize_repo_url) else {
                continue;
            };
            if wanted.contains(&key) {
                by_repo.entry(key).or_default().push(Candidate {
                    crate_id,
                    name: row.name.clone(),
                    reasons: Vec::new(),
                    deprecated: row.deprecation.is_some(),
                });
            }
        }
        Self { by_repo }
    }

    /// The ids of every kept crate.
    pub(super) fn ids(&self) -> IdSet {
        self.by_repo
            .values()
            .flatten()
            .map(|candidate| candidate.crate_id)
            .collect()
    }

    /// The crates that claim one of `keys`, without repeats, sorted by name.
    pub(super) fn of_keys(&self, keys: &[String]) -> Vec<&Candidate> {
        let mut found: Vec<&Candidate> = keys
            .iter()
            .filter_map(|key| self.by_repo.get(key))
            .flatten()
            .collect();
        found.sort_by(|a, b| a.name.cmp(&b.name).then(a.crate_id.cmp(&b.crate_id)));
        found.dedup_by_key(|candidate| candidate.crate_id);
        found
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::fixtures::{Spec, World};
    use super::*;

    fn linked(id: u32, name: &'static str, link: &str) -> Spec {
        let mut spec = Spec::new(id, name);
        spec.repository = Some(link.to_owned());
        spec
    }

    fn wanted(keys: &[&str]) -> HashSet<String> {
        keys.iter().map(|key| (*key).to_owned()).collect()
    }

    #[test]
    fn any_link_form_claims_the_repository() {
        let world = World::new(vec![
            linked(1, "b", "http://github.com/O/R.git"),
            linked(2, "a", "https://www.github.com/o/r/tree/main/a"),
            linked(3, "other", "https://github.com/o/other"),
        ]);
        let claims = Claims::of(&world.snapshot, &wanted(&["github.com/o/r"]));
        let found: Vec<&str> = claims
            .of_keys(&["github.com/o/r".to_owned()])
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect();
        assert_eq!(found, ["a", "b"]);
        assert_eq!(claims.ids().len(), 2);
    }

    #[test]
    fn a_repeated_key_gives_each_crate_once() {
        let world = World::new(vec![linked(1, "a", "https://github.com/o/r")]);
        let key = "github.com/o/r".to_owned();
        let claims = Claims::of(&world.snapshot, &wanted(&[key.as_str()]));
        assert_eq!(claims.of_keys(&[key.clone(), key]).len(), 1);
    }
}
