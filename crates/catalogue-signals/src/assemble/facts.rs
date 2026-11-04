//! Repository signals from the GitHub facts of an entry's repo.

use catalogue_core::Signals;

use crate::github::Contributors;
use crate::history::RepoValues;

/// Sets the repository signals from `facts`.
///
/// - [`RepoValues::Found`]: `repo_archived`, `repo_gone = false`, `last_human_commit`,
///   `oldest_unanswered_days`, `maintainer_replies_365`, `stars` and `forks`.
///   A repo with no unanswered outside item has an oldest unanswered age of 0.
///   A newest human commit that lies beyond the sampled commits stays unknown.
///   A snapshot written before `forks` existed leaves it unknown.
/// - [`RepoValues::Gone`]: `repo_gone = true`. The other repository signals stay unknown.
/// - `None`: every repository signal stays unknown.
pub(super) fn apply_facts(facts: Option<&RepoValues>, signals: &mut Signals) {
    match facts {
        Some(RepoValues::Found {
            archived,
            stars,
            forks,
            last_human_commit,
            oldest_unanswered_days,
            maintainer_replies_365,
            ..
        }) => {
            signals.repo_archived = Some(*archived);
            signals.repo_gone = Some(false);
            signals.last_human_commit = *last_human_commit;
            signals.oldest_unanswered_days = Some(oldest_unanswered_days.unwrap_or(0));
            signals.maintainer_replies_365 = Some(*maintainer_replies_365);
            signals.stars = Some(u64::from(*stars));
            signals.forks = forks.map(u64::from);
        }
        Some(RepoValues::Gone) => signals.repo_gone = Some(true),
        None => {}
    }
}

/// Sets `contributors` from the count GitHub gave.
///
/// A list GitHub calls too large and a missing count both leave the signal unknown.
pub fn apply_contributors(contributors: Option<&Contributors>, signals: &mut Signals) {
    signals.contributors = contributors.and_then(|found| match found {
        Contributors::Counted { count } => Some(*count),
        Contributors::TooLarge => None,
    });
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Date, EntryKind};

    use super::*;
    use crate::assemble::fixtures::{day, found_facts};
    use crate::github::{Facts, RepoFacts, RepoRef};

    fn applied(facts: Option<&RepoFacts>) -> Signals {
        let mut signals = Signals::unknown(EntryKind::Library);
        let values = facts.map(RepoValues::from);
        apply_facts(values.as_ref(), &mut signals);
        signals
    }

    #[test]
    fn found_facts_set_every_repository_signal() {
        let signals = applied(Some(&RepoFacts::Found(found_facts())));
        assert_eq!(signals.repo_archived, Some(false));
        assert_eq!(signals.repo_gone, Some(false));
        assert_eq!(
            signals.last_human_commit,
            Some(Date::from(day(2026, 10, 1)))
        );
        assert_eq!(signals.oldest_unanswered_days, Some(12));
        assert_eq!(signals.maintainer_replies_365, Some(7));
    }

    #[test]
    fn found_facts_set_stars_and_forks_and_old_snapshots_leave_forks_unknown() {
        let facts = Facts {
            stars: 1_200,
            forks: Some(80),
            ..found_facts()
        };
        let signals = applied(Some(&RepoFacts::Found(facts)));
        assert_eq!(signals.stars, Some(1_200));
        assert_eq!(signals.forks, Some(80));
        let old: RepoValues = serde_json::from_str(
            r#"{"state":"found","archived":false,"stars":1,"last_human_commit":null,"oldest_unanswered_days":null,"maintainer_replies_365":0}"#,
        )
        .expect("an old snapshot reads");
        let mut signals = Signals::unknown(EntryKind::Library);
        apply_facts(Some(&old), &mut signals);
        assert_eq!(signals.stars, Some(1));
        assert_eq!(signals.forks, None);
    }

    #[test]
    fn a_contributor_list_too_large_is_unknown() {
        let mut signals = Signals::unknown(EntryKind::Library);
        apply_contributors(Some(&Contributors::Counted { count: 38 }), &mut signals);
        assert_eq!(signals.contributors, Some(38));
        apply_contributors(Some(&Contributors::TooLarge), &mut signals);
        assert_eq!(signals.contributors, None);
        apply_contributors(None, &mut signals);
        assert_eq!(signals.contributors, None);
    }

    #[test]
    fn no_unanswered_item_is_age_zero_and_a_sampled_out_commit_is_unknown() {
        let facts = Facts {
            oldest_unanswered_days: None,
            last_human_commit: None,
            last_human_commit_beyond_sample: true,
            archived: true,
            ..found_facts()
        };
        let signals = applied(Some(&RepoFacts::Found(facts)));
        assert_eq!(signals.oldest_unanswered_days, Some(0));
        assert_eq!(signals.last_human_commit, None);
        assert_eq!(signals.repo_archived, Some(true));
    }

    #[test]
    fn a_gone_repo_sets_only_repo_gone() {
        let requested = RepoRef::parse("ml-rust/tool").expect("repo");
        let signals = applied(Some(&RepoFacts::Gone { requested }));
        assert_eq!(signals.repo_gone, Some(true));
        assert_eq!(signals.repo_archived, None);
        assert_eq!(signals.maintainer_replies_365, None);
    }

    #[test]
    fn no_facts_leave_every_repository_signal_unknown() {
        assert_eq!(applied(None), Signals::unknown(EntryKind::Library));
    }
}
