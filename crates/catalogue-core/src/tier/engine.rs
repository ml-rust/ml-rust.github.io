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
            dormant: None,
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

    let rot = rot(signals, context.today, &thresholds.rot);
    // Rot counts only toward the dead rule, which needs dormancy, and toward
    // the finished rule, which reports its own missing facts.
    if idle.dormant != Some(false) {
        notes.needs(&rot.needs);
    }
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
        dormant: idle.dormant,
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
        assert_eq!(verdict.dormant, Some(false));
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

    /// A library with too few dependents that passes the repository route.
    fn route_library() -> Signals {
        let t = thresholds().repo_route;
        Signals {
            active_users: Some(1.0),
            new_adopters: Some(0.0),
            stars: Some(t.min_stars),
            forks: Some(t.min_forks),
            contributors: Some(t.min_contributors),
            ..featured_library()
        }
    }

    #[test]
    fn the_repository_route_yields_featured_with_every_signal_known() {
        let verdict = run(&route_library(), &context());
        assert_eq!(verdict.tier, Tier::Featured);
        assert!(verdict.unknown.is_empty(), "{:?}", verdict.unknown);
    }

    #[test]
    fn stars_alone_leave_a_library_listed() {
        let signals = Signals {
            forks: Some(0),
            contributors: Some(0),
            ..route_library()
        };
        let verdict = run(&signals, &context());
        assert_eq!(verdict.tier, Tier::Listed);
    }

    #[test]
    fn a_failed_health_gate_blocks_the_repository_route() {
        for signals in [
            Signals {
                unpatched_vulnerabilities: Some(vec!["RUSTSEC-2026-0004".to_owned()]),
                ..route_library()
            },
            Signals {
                former_glory: Some(0.1),
                ..route_library()
            },
            Signals {
                repo_link_verified: Some(false),
                ..route_library()
            },
        ] {
            let verdict = run(&signals, &context());
            assert_eq!(verdict.tier, Tier::Listed, "{signals:?}");
        }
    }

    #[test]
    fn new_and_archived_take_precedence_over_the_repository_route() {
        let young = Signals {
            first_release: Some(days_ago(30)),
            ..route_library()
        };
        assert_eq!(run(&young, &context()).tier, Tier::New);
        let archived = Signals {
            repo_archived: Some(true),
            ..route_library()
        };
        assert_eq!(run(&archived, &context()).tier, Tier::Archived);
    }

    #[test]
    fn an_unknown_route_part_keeps_a_library_listed_and_names_it() {
        let signals = Signals {
            forks: None,
            ..route_library()
        };
        let verdict = run(&signals, &context());
        assert_eq!(verdict.tier, Tier::Listed);
        assert_eq!(verdict.unknown, ["forks"]);
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
        assert_eq!(verdict.dormant, None);
    }

    #[test]
    fn dead_archives_on_the_second_consecutive_check() {
        let first = run(&dead_library(), &context());
        assert!(first.dead_now);
        assert_eq!(first.dormant, Some(true));
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
    fn a_failing_build_counts_toward_dead() {
        let signals = Signals {
            repo_gone: Some(false),
            build_fails: Some(true),
            ..dead_library()
        };
        let verdict = run(&signals, &context());
        assert!(verdict.dead_now);
        assert!(
            verdict
                .reasons
                .iter()
                .any(|reason| reason.detail.contains("fails `cargo check`"))
        );
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
    fn young_library_is_new() {
        let signals = Signals {
            first_release: Some(days_ago(30)),
            ..featured_library()
        };
        let verdict = run(&signals, &context());
        assert_eq!(verdict.tier, Tier::New);
        assert_eq!(rules(&verdict).last(), Some(&Rule::New));
    }

    #[test]
    fn old_library_with_few_users_is_listed_not_new() {
        let signals = Signals {
            first_release: Some(days_ago(3000)),
            active_users: Some(0.0),
            new_adopters: Some(0.0),
            ..featured_library()
        };
        let verdict = run(&signals, &context());
        assert_eq!(verdict.tier, Tier::Listed);
        assert!(!rules(&verdict).contains(&Rule::New));
    }

    #[test]
    fn a_prerelease_only_app_with_a_crate_is_listed_and_downloads_do_not_move_it() {
        for all_zero_zero in [false, true] {
            let signals = Signals {
                all_prerelease: Some(true),
                all_zero_zero: Some(all_zero_zero),
                downloads_per_month: Some(u64::MAX),
                ..featured_app()
            };
            assert_eq!(run(&signals, &context()).tier, Tier::Listed);
        }
    }

    #[test]
    fn an_app_with_no_crate_facts_skips_the_release_gate_and_downloads_never_lift_it() {
        let signals = Signals {
            first_release: Some(days_ago(1000)),
            latest_version: None,
            release_count: None,
            all_prerelease: None,
            all_zero_zero: None,
            ..featured_app()
        };
        assert_eq!(run(&signals, &context()).tier, Tier::Featured);
        let unused = Signals {
            first_release: Some(days_ago(1000)),
            binary_downloads_last3: Some(0),
            homebrew_on_request_365d: Some(0),
            repology_families: Some(0),
            docker_pull_growth_90d: Some(0),
            downloads_per_month: Some(u64::MAX),
            ..featured_app()
        };
        assert_eq!(run(&unused, &context()).tier, Tier::Listed);
    }

    #[test]
    fn zero_zero_only_library_with_high_adoption_is_listed_with_the_gate_text() {
        let signals = Signals {
            first_release: Some(days_ago(1000)),
            all_prerelease: Some(true),
            all_zero_zero: Some(true),
            active_users: Some(400.0),
            ..featured_library()
        };
        let verdict = run(&signals, &context());
        assert_eq!(verdict.tier, Tier::Listed);
        assert!(!rules(&verdict).contains(&Rule::New));
        assert!(
            verdict
                .reasons
                .iter()
                .any(|reason| reason.detail.contains("every version is 0.0.x")),
            "{:?}",
            verdict.reasons
        );
    }

    #[test]
    fn prerelease_only_library_with_high_adoption_is_featured() {
        let signals = Signals {
            all_prerelease: Some(true),
            all_zero_zero: Some(false),
            active_users: Some(400.0),
            ..featured_library()
        };
        let verdict = run(&signals, &context());
        assert_eq!(verdict.tier, Tier::Featured);
        assert!(verdict.unknown.is_empty(), "{:?}", verdict.unknown);
    }

    #[test]
    fn prerelease_only_library_on_the_route_alone_is_listed() {
        let signals = Signals {
            all_prerelease: Some(true),
            all_zero_zero: Some(false),
            ..route_library()
        };
        assert_eq!(run(&signals, &context()).tier, Tier::Listed);
    }

    #[test]
    fn the_route_without_the_main_crate_leaves_a_library_listed() {
        let signals = Signals {
            main_crate_left_out: Some("wasmtime".to_owned()),
            ..route_library()
        };
        let verdict = run(&signals, &context());
        assert_eq!(verdict.tier, Tier::Listed);
        assert!(verdict.unknown.is_empty(), "{:?}", verdict.unknown);
    }

    #[test]
    fn an_active_entry_lists_no_unknown_rot_fact() {
        let mut signals = featured_library();
        signals.build_fails = None;
        signals.repo_gone = None;
        let verdict = run(&signals, &context());
        assert_eq!(verdict.dormant, Some(false));
        assert!(
            !verdict.unknown.contains(&"build_fails"),
            "{:?}",
            verdict.unknown
        );
        assert!(
            !verdict.unknown.contains(&"repo_gone"),
            "{:?}",
            verdict.unknown
        );

        let mut idle = dormant_library();
        idle.build_fails = None;
        let verdict = run(&idle, &context());
        assert_eq!(verdict.dormant, Some(true));
        assert!(
            verdict.unknown.contains(&"build_fails"),
            "{:?}",
            verdict.unknown
        );
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
