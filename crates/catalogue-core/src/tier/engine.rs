use super::admit::admit;
use super::archive::{dead, instant};
use super::context::Context;
use super::dormancy::dormancy;
use super::fact::Fact;
use super::finished::finished;
use super::guard::guard;
use super::interval::expected;
use super::level::Tier;
use super::notes::Notes;
use super::rot::rot;
use super::signals::Signals;
use super::verdict::Verdict;
use crate::registry::Thresholds;

/// Applies the tier rules in order to one entry on one check.
///
/// 1. Instant archive ends the check.
/// 2. to 4. Expected interval, dormancy, rot.
/// 5. Dead on enough consecutive checks archives.
/// 6. Finished sets `stable`.
/// 7. to 9. New, then Featured, then Listed. An org affiliated entry needs an
///    outside approval for Featured.
/// 10. A Featured entry keeps its tier until a demotion judgment approves the drop.
///     Instant archive bypasses this guard.
pub fn assess(signals: &Signals, context: &Context, thresholds: &Thresholds) -> Verdict {
    let ended = instant(signals);
    if !ended.is_empty() {
        return Verdict {
            tier: Tier::Archived,
            stable: false,
            dead_now: false,
            reasons: ended,
            unknown: Vec::new(),
            demotion_pending: None,
        };
    }
    let mut notes = Notes::default();

    let interval = expected(signals, thresholds);
    notes.reason(interval.reason);
    notes.needs(&interval.needs);

    let idle = dormancy(signals, context.today, interval.days);
    notes.reason(idle.reason);
    notes.needs(&idle.needs);
    let dormant = match idle.dormant {
        Some(value) => Fact::known(value),
        None => Fact::unknown(idle.needs),
    };

    let rot = rot(signals, &thresholds.rot);
    notes.needs(&rot.needs);
    notes.reasons.extend(rot.found.iter().cloned());

    let dead = dead(idle.dormant, rot.count(), context, &thresholds.dormancy);
    if let Some(reason) = dead.reason {
        notes.reason(reason);
    }

    let done = finished(signals, context, &dormant, &rot, &thresholds.finished);
    if let Some(reason) = done.reason {
        notes.reason(reason);
    }
    notes.needs(&done.needs);

    let computed = if dead.archived {
        Tier::Archived
    } else {
        let admitted = admit(signals, context, thresholds, dormant, done.stable);
        notes.needs(&admitted.needs);
        notes.reasons.extend(admitted.reasons);
        admitted.tier
    };
    let held = guard(context, computed);
    if let Some(reason) = held.reason {
        notes.reason(reason);
    }
    Verdict {
        tier: held.tier,
        stable: done.stable,
        dead_now: dead.now,
        reasons: notes.reasons,
        unknown: notes.unknown,
        demotion_pending: held.pending,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::EntryKind;
    use crate::judgment::MaintainerStatus;
    use crate::tier::fixtures::{
        context, days_ago, dead_library, dormant_library, featured_app, featured_library,
        thresholds,
    };
    use crate::tier::verdict::Rule;

    fn run(signals: &Signals, ctx: &Context) -> Verdict {
        assess(signals, ctx, &thresholds())
    }

    fn rules(verdict: &Verdict) -> Vec<Rule> {
        verdict.reasons.iter().map(|reason| reason.rule).collect()
    }

    #[test]
    fn healthy_library_is_featured_with_every_signal_known() {
        let verdict = run(&featured_library(), &context());
        assert_eq!(verdict.tier, Tier::Featured);
        assert!(!verdict.stable);
        assert!(!verdict.dead_now);
        assert!(verdict.unknown.is_empty(), "{:?}", verdict.unknown);
        assert_eq!(
            rules(&verdict),
            [Rule::ExpectedInterval, Rule::Dormancy, Rule::Featured]
        );
        assert!(verdict.demotion_pending.is_none());
    }

    #[test]
    fn healthy_app_is_featured() {
        let verdict = run(&featured_app(), &context());
        assert_eq!(verdict.tier, Tier::Featured);
        assert!(verdict.unknown.is_empty(), "{:?}", verdict.unknown);
    }

    #[test]
    fn instant_archive_ignores_statements_and_bypasses_the_guard() {
        let signals = Signals {
            repo_archived: Some(true),
            ..featured_library()
        };
        let ctx = Context {
            previous_tier: Some(Tier::Featured),
            maintainer_statement: Some(MaintainerStatus::Maintained),
            ..context()
        };
        let verdict = run(&signals, &ctx);
        assert_eq!(verdict.tier, Tier::Archived);
        assert!(verdict.demotion_pending.is_none());
        assert_eq!(rules(&verdict), [Rule::InstantArchive]);
        assert!(!verdict.dead_now);
    }

    #[test]
    fn dead_archives_on_the_second_consecutive_check() {
        let first = run(&dead_library(), &context());
        assert!(first.dead_now);
        assert_eq!(first.tier, Tier::Listed);
        assert!(first.has(Rule::Dead));
        assert_eq!(rules(&first).iter().filter(|r| **r == Rule::Rot).count(), 2);
        let ctx = Context {
            prior_dead_checks: 1,
            ..context()
        };
        let second = run(&dead_library(), &ctx);
        assert!(second.dead_now);
        assert_eq!(second.tier, Tier::Archived);
        assert!(!second.has(Rule::Listed));
    }

    #[test]
    fn one_rot_signal_is_not_dead() {
        let signals = Signals {
            repo_gone: Some(false),
            ..dead_library()
        };
        let ctx = Context {
            prior_dead_checks: 1,
            ..context()
        };
        let verdict = run(&signals, &ctx);
        assert!(!verdict.dead_now);
        assert_eq!(verdict.tier, Tier::Listed);
    }

    #[test]
    fn maintained_statement_blocks_dead() {
        let ctx = Context {
            prior_dead_checks: 3,
            maintainer_statement: Some(MaintainerStatus::Maintained),
            ..context()
        };
        let verdict = run(&dead_library(), &ctx);
        assert!(!verdict.dead_now);
        assert_eq!(verdict.tier, Tier::Listed);
        assert!(verdict.has(Rule::Dead));
    }

    #[test]
    fn dead_archive_waits_for_an_approved_demotion() {
        let ctx = Context {
            prior_dead_checks: 1,
            previous_tier: Some(Tier::Featured),
            ..context()
        };
        let held = run(&dead_library(), &ctx);
        assert_eq!(held.tier, Tier::Featured);
        assert_eq!(held.demotion_pending, Some(Tier::Archived));
        assert!(held.dead_now);
        assert!(held.has(Rule::DemotionPending));
        let approved = Context {
            demotion_approved: true,
            ..ctx
        };
        let dropped = run(&dead_library(), &approved);
        assert_eq!(dropped.tier, Tier::Archived);
        assert!(dropped.demotion_pending.is_none());
    }

    #[test]
    fn dormant_but_finished_library_stays_featured_and_stable() {
        let signals = Signals {
            last_release: Some(days_ago(2_900)),
            last_human_commit: Some(days_ago(2_900)),
            ..featured_library()
        };
        let verdict = run(&signals, &context());
        assert!(verdict.stable);
        assert_eq!(verdict.tier, Tier::Featured);
        assert!(verdict.has(Rule::Finished));
    }

    #[test]
    fn finished_statement_marks_stable_without_dormancy() {
        let ctx = Context {
            maintainer_statement: Some(MaintainerStatus::Finished),
            ..context()
        };
        let verdict = run(&dormant_library(), &ctx);
        assert!(verdict.stable);
        assert_eq!(verdict.tier, Tier::Listed);
    }

    #[test]
    fn young_library_drop_from_featured_waits_for_approval() {
        let signals = Signals {
            first_release: Some(days_ago(30)),
            ..featured_library()
        };
        let ctx = Context {
            previous_tier: Some(Tier::Featured),
            ..context()
        };
        let held = run(&signals, &ctx);
        assert_eq!(held.tier, Tier::Featured);
        assert_eq!(held.demotion_pending, Some(Tier::New));
    }

    #[test]
    fn unknown_library_is_listed_and_names_what_it_lacked() {
        let verdict = run(&Signals::unknown(EntryKind::Library), &context());
        assert_eq!(verdict.tier, Tier::Listed);
        assert!(!verdict.dead_now);
        assert!(!verdict.stable);
        for name in [
            "latest_version",
            "last_release",
            "last_human_commit",
            "repo_gone",
            "first_release",
            "active_users",
        ] {
            assert!(
                verdict.unknown.contains(&name),
                "{name}: {:?}",
                verdict.unknown
            );
        }
        let mut sorted = verdict.unknown.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), verdict.unknown.len());
    }

    #[test]
    fn app_with_unknown_use_signals_is_listed() {
        let signals = Signals {
            binary_downloads_last3: None,
            homebrew_on_request_365d: None,
            repology_families: None,
            docker_pull_growth_90d: Some(0),
            ..featured_app()
        };
        let verdict = run(&signals, &context());
        assert_eq!(verdict.tier, Tier::Listed);
        assert_eq!(
            verdict.unknown,
            [
                "binary_downloads_last3",
                "homebrew_on_request_365d",
                "repology_families"
            ]
        );
    }
}
