use super::context::Context;
use super::signals::Signals;
use super::verdict::{Reason, Rule};
use crate::judgment::MaintainerStatus;
use crate::registry::DormancyThresholds;

/// The dead rule on one check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Dead {
    /// Dead on this check.
    pub now: bool,
    /// Dead on enough consecutive checks to archive.
    pub archived: bool,
    pub reason: Option<Reason>,
}

/// Rule 1: the author or RustSec ends the project. Any one signal archives the entry:
/// archived repo, RustSec `unmaintained` advisory, deprecation notice, every version
/// yanked, or crate deleted. A `maintained` statement does not override them.
///
/// Returns one reason per signal found. Empty means the rule does not apply.
pub(super) fn instant(signals: &Signals) -> Vec<Reason> {
    let mut out = Vec::new();
    let mut add = |detail: String| out.push(Reason::new(Rule::InstantArchive, detail));
    if signals.repo_archived == Some(true) {
        add("repository is archived".to_owned());
    }
    if let Some(id) = &signals.rustsec_unmaintained {
        add(format!("RustSec unmaintained advisory {id}"));
    }
    if let Some(notice) = &signals.deprecation_notice {
        add(format!("deprecation notice: {notice}"));
    }
    if signals.all_yanked == Some(true) {
        add("every version is yanked".to_owned());
    }
    if signals.crate_deleted == Some(true) {
        add("crate is deleted".to_owned());
    }
    out
}

/// Rule 5: an entry is dead on this check when it is dormant, has at least
/// `dead_rot_signals` rot signals, and no `maintained` statement exists. It archives
/// when this check and the previous checks make `dead_consecutive_checks` in a row.
/// Unknown dormancy is not dead.
pub(super) fn dead(
    dormant: Option<bool>,
    rot_count: u32,
    context: &Context,
    t: &DormancyThresholds,
) -> Dead {
    let decayed = dormant == Some(true) && rot_count >= t.dead_rot_signals;
    if !decayed {
        return Dead {
            now: false,
            archived: false,
            reason: None,
        };
    }
    if context.maintainer_statement == Some(MaintainerStatus::Maintained) {
        return Dead {
            now: false,
            archived: false,
            reason: Some(Reason::new(
                Rule::Dead,
                format!(
                    "dormant with {rot_count} rot signals, but the maintainer states `maintained`: not dead"
                ),
            )),
        };
    }
    let checks = context.prior_dead_checks.saturating_add(1);
    let archived = checks >= t.dead_consecutive_checks;
    let outcome = if archived {
        "archived"
    } else {
        "not yet archived"
    };
    Dead {
        now: true,
        archived,
        reason: Some(Reason::new(
            Rule::Dead,
            format!(
                "dormant with {rot_count} rot signals (limit {}): dead on check {checks} of {}, {outcome}",
                t.dead_rot_signals, t.dead_consecutive_checks
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tier::fixtures::{context, featured_library, thresholds};

    #[test]
    fn healthy_entry_has_no_instant_archive() {
        assert!(instant(&featured_library()).is_empty());
    }

    #[test]
    fn each_signal_archives_on_its_own() {
        let cases = [
            (
                Signals {
                    repo_archived: Some(true),
                    ..featured_library()
                },
                "repository is archived",
            ),
            (
                Signals {
                    rustsec_unmaintained: Some("RUSTSEC-2026-0003".to_owned()),
                    ..featured_library()
                },
                "RustSec unmaintained advisory RUSTSEC-2026-0003",
            ),
            (
                Signals {
                    deprecation_notice: Some("use beta instead".to_owned()),
                    ..featured_library()
                },
                "deprecation notice: use beta instead",
            ),
            (
                Signals {
                    all_yanked: Some(true),
                    ..featured_library()
                },
                "every version is yanked",
            ),
            (
                Signals {
                    crate_deleted: Some(true),
                    ..featured_library()
                },
                "crate is deleted",
            ),
        ];
        for (signals, detail) in cases {
            let found = instant(&signals);
            assert_eq!(found, [Reason::new(Rule::InstantArchive, detail)]);
        }
    }

    #[test]
    fn unknown_and_false_signals_do_not_archive() {
        let signals = Signals {
            repo_archived: None,
            all_yanked: None,
            crate_deleted: None,
            ..featured_library()
        };
        assert!(instant(&signals).is_empty());
    }

    #[test]
    fn several_signals_give_several_reasons() {
        let signals = Signals {
            repo_archived: Some(true),
            crate_deleted: Some(true),
            ..featured_library()
        };
        assert_eq!(instant(&signals).len(), 2);
    }

    fn dead_with(dormant: Option<bool>, rot: u32, ctx: &Context) -> Dead {
        dead(dormant, rot, ctx, &thresholds().dormancy)
    }

    #[test]
    fn first_dead_check_does_not_archive_and_the_second_does() {
        let first = dead_with(Some(true), 2, &context());
        assert!(first.now);
        assert!(!first.archived);
        let detail = first.reason.map(|r| r.detail).unwrap_or_default();
        assert!(
            detail.contains("check 1 of 2, not yet archived"),
            "{detail}"
        );
        let ctx = Context {
            prior_dead_checks: 1,
            ..context()
        };
        let second = dead_with(Some(true), 2, &ctx);
        assert!(second.now);
        assert!(second.archived);
    }

    #[test]
    fn dead_needs_dormancy_and_enough_rot() {
        assert!(!dead_with(Some(true), 1, &context()).now);
        assert!(!dead_with(Some(false), 5, &context()).now);
        assert!(!dead_with(None, 5, &context()).now);
        assert!(dead_with(Some(true), 2, &context()).reason.is_some());
        assert!(dead_with(Some(true), 1, &context()).reason.is_none());
    }

    #[test]
    fn maintained_statement_blocks_dead() {
        let ctx = Context {
            maintainer_statement: Some(MaintainerStatus::Maintained),
            prior_dead_checks: 5,
            ..context()
        };
        let found = dead_with(Some(true), 3, &ctx);
        assert!(!found.now);
        assert!(!found.archived);
        let detail = found.reason.map(|r| r.detail).unwrap_or_default();
        assert!(detail.contains("states `maintained`"), "{detail}");
    }

    #[test]
    fn finished_statement_does_not_block_dead() {
        let ctx = Context {
            maintainer_statement: Some(MaintainerStatus::Finished),
            ..context()
        };
        assert!(dead_with(Some(true), 2, &ctx).now);
    }
}
