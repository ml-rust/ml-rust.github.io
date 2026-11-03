//! Repository signals from the GitHub facts of an entry's repo.

use catalogue_core::Signals;

use crate::history::RepoValues;

/// Sets the repository signals from `facts`.
///
/// - [`RepoValues::Found`]: `repo_archived`, `repo_gone = false`,
///   `last_human_commit`, `oldest_unanswered_days` and `maintainer_replies_365`.
///   A repo with no unanswered outside item has an oldest unanswered age of 0.
///   A newest human commit that lies beyond the sampled commits stays unknown.
/// - [`RepoValues::Gone`]: `repo_gone = true`. The other repository signals stay unknown.
/// - `None`: every repository signal stays unknown.
pub(super) fn apply_facts(facts: Option<&RepoValues>, signals: &mut Signals) {
    match facts {
        Some(RepoValues::Found {
            archived,
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
        }
        Some(RepoValues::Gone) => signals.repo_gone = Some(true),
        None => {}
    }
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
