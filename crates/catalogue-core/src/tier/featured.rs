use super::app::app_gate;
use super::fact::{Fact, show, show_amount, show_below};
use super::repo_route::{repo_route, route_text};
use super::signals::Signals;
use super::verdict::{Reason, Rule};
use crate::registry::{LibraryFeaturedThresholds, RepoRouteThresholds, Thresholds};

/// The Featured rule on one check, before the outside approval check.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Featured {
    pub fact: Fact,
    pub reason: Reason,
}

/// One Featured gate and the text shown when it fails.
pub(super) struct Gate {
    pub fact: Fact,
    pub failure: String,
}

/// Whether the active dependents or the new dependents reach their limit.
fn dependents_pass(signals: &Signals, t: &LibraryFeaturedThresholds) -> Fact {
    Fact::any(vec![
        Fact::of("active_users", signals.active_users, |n| {
            n >= t.min_active_users
        }),
        Fact::of("new_adopters", signals.new_adopters, |n| {
            n >= t.min_new_adopters
        }),
    ])
}

/// The library gates: adoption by active dependents, new dependents or the repository route, no
/// unpatched vulnerability, a verified repository link, `former_glory`, and docs.
fn library_gates(
    signals: &Signals,
    t: &LibraryFeaturedThresholds,
    route: &RepoRouteThresholds,
) -> Vec<Gate> {
    vec![
        Gate {
            fact: Fact::any(vec![
                dependents_pass(signals, t),
                repo_route(signals, route),
            ]),
            failure: format!(
                "active_users {} < {} and new_adopters {} < {} and {}",
                show_below(signals.active_users, t.min_active_users),
                t.min_active_users,
                show_below(signals.new_adopters, t.min_new_adopters),
                t.min_new_adopters,
                route_text(signals, route)
            ),
        },
        Gate {
            fact: Fact::of(
                "unpatched_vulnerabilities",
                signals.unpatched_vulnerabilities.as_deref(),
                <[String]>::is_empty,
            ),
            failure: "unpatched vulnerabilities exist".to_owned(),
        },
        Gate {
            fact: Fact::of("repo_link_verified", signals.repo_link_verified, |ok| ok),
            failure: "repository link not verified".to_owned(),
        },
        Gate {
            fact: Fact::of("former_glory", signals.former_glory, |share| {
                share >= t.min_former_glory
            }),
            failure: format!(
                "former_glory {} < {}",
                show_below(signals.former_glory, t.min_former_glory),
                t.min_former_glory
            ),
        },
        Gate {
            fact: Fact::of("docs_present", signals.docs_present, |ok| ok),
            failure: "docs missing".to_owned(),
        },
    ]
}

/// The release gate: the entry has a stable release, so not every version is a prerelease or
/// `0.0.x`.
///
/// A library with an unknown value fails the gate as unknown. An app has a crate only
/// when it ships one, so an app with an unknown value passes.
fn release_gate(signals: &Signals) -> Gate {
    let fact = match signals.all_prerelease {
        None if !signals.is_library() => Fact::known(true),
        known => Fact::of("all_prerelease", known, |all| !all),
    };
    Gate {
        fact,
        failure: "every version is a prerelease or 0.0.x".to_owned(),
    }
}

/// The values of a library that passes. The route leads only when it is what passed.
fn library_detail(signals: &Signals, thresholds: &Thresholds) -> String {
    let route = &thresholds.repo_route;
    let values = format!(
        "active_users {}, new_adopters {}, former_glory {}",
        show_amount(signals.active_users),
        show_amount(signals.new_adopters),
        show_amount(signals.former_glory)
    );
    if dependents_pass(signals, &thresholds.library_featured).is_true() {
        values
    } else {
        format!("{}, {values}", route_text(signals, route))
    }
}

/// The values of an app that passes. The route shows only when it is one of the use signals
/// that pass.
fn app_detail(signals: &Signals, route: &RepoRouteThresholds) -> String {
    let values = format!(
        "binary downloads {}, Homebrew {}, Repology families {}, Docker growth {}",
        show(signals.binary_downloads_last3),
        show(signals.homebrew_on_request_365d),
        show(signals.repology_families),
        show(signals.docker_pull_growth_90d)
    );
    if repo_route(signals, route).is_true() {
        format!("{values}, {}", route_text(signals, route))
    } else {
        values
    }
}

/// Rule 8: the Featured gates for the entry's kind, plus the release gate and activity: not
/// dormant, or stable.
///
/// `active` is that activity condition. Unknown gates leave the rule unknown, so the
/// entry is not featured.
pub(super) fn featured(signals: &Signals, active: Fact, thresholds: &Thresholds) -> Featured {
    let mut gates = if signals.is_library() {
        library_gates(
            signals,
            &thresholds.library_featured,
            &thresholds.repo_route,
        )
    } else {
        vec![app_gate(
            signals,
            &thresholds.app_featured,
            &thresholds.repo_route,
        )]
    };
    gates.push(release_gate(signals));
    gates.push(Gate {
        fact: active,
        failure: "dormant and not stable".to_owned(),
    });
    let failures: Vec<&str> = gates
        .iter()
        .filter(|gate| gate.fact.is_false())
        .map(|gate| gate.failure.as_str())
        .collect();
    let detail = if !failures.is_empty() {
        format!("not featured: {}", failures.join("; "))
    } else if signals.is_library() {
        library_detail(signals, thresholds)
    } else {
        app_detail(signals, &thresholds.repo_route)
    };
    let fact = Fact::all(gates.into_iter().map(|gate| gate.fact).collect());
    let detail = match fact.value {
        Some(true) => format!("featured: {detail}"),
        Some(false) => detail,
        None => format!("not featured: unknown {}", fact.needs.join(", ")),
    };
    Featured {
        fact,
        reason: Reason::new(Rule::Featured, detail),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::EntryKind;
    use crate::tier::fixtures::{featured_app, featured_library, thresholds};

    fn check(signals: &Signals) -> Featured {
        featured(signals, Fact::known(true), &thresholds())
    }

    fn fails(signals: &Signals, needle: &str) {
        let found = check(signals);
        assert!(found.fact.is_false(), "{found:?}");
        assert!(
            found.reason.detail.contains(needle),
            "{}",
            found.reason.detail
        );
    }

    #[test]
    fn healthy_library_and_app_are_featured() {
        let library = check(&featured_library());
        assert!(library.fact.is_true());
        assert!(
            library
                .reason
                .detail
                .starts_with("featured: active_users 50")
        );
        let app = check(&featured_app());
        assert!(app.fact.is_true());
        assert!(
            app.reason
                .detail
                .starts_with("featured: binary downloads 5000")
        );
    }

    #[test]
    fn library_adoption_passes_on_either_count_at_the_limit() {
        let users = Signals {
            active_users: Some(10.0),
            new_adopters: Some(0.0),
            ..featured_library()
        };
        assert!(check(&users).fact.is_true());
        let adopters = Signals {
            active_users: Some(3.0),
            new_adopters: Some(5.0),
            ..featured_library()
        };
        assert!(check(&adopters).fact.is_true());
        let neither = Signals {
            active_users: Some(9.9),
            new_adopters: Some(4.9),
            ..featured_library()
        };
        fails(
            &neither,
            "active_users 9.9 < 10 and new_adopters 4.9 < 5 and repository route stars 100 (needs 2000), forks 10 (needs 200), contributors 5 (needs 25)",
        );
    }

    /// A library with too few dependents, and the repository numbers given here.
    fn low_dependents(
        stars: Option<u64>,
        forks: Option<u64>,
        contributors: Option<u64>,
    ) -> Signals {
        Signals {
            active_users: Some(1.0),
            new_adopters: Some(0.0),
            stars,
            forks,
            contributors,
            ..featured_library()
        }
    }

    /// [`low_dependents`] with every repository number at its limit.
    fn on_the_route() -> Signals {
        let t = thresholds().repo_route;
        low_dependents(
            Some(t.min_stars),
            Some(t.min_forks),
            Some(t.min_contributors),
        )
    }

    #[test]
    fn the_repository_route_passes_the_adoption_gate_and_names_itself() {
        let found = check(&on_the_route());
        assert!(found.fact.is_true(), "{found:?}");
        assert_eq!(
            found.reason.detail,
            "featured: repository route stars 2000 (needs 2000), forks 200 (needs 200), contributors 25 (needs 25), active_users 1, new_adopters 0, former_glory 0.9"
        );
    }

    #[test]
    fn stars_alone_never_pass_the_adoption_gate() {
        let found = low_dependents(Some(u64::MAX), Some(0), Some(0));
        fails(&found, "forks 0 (needs 200), contributors 0 (needs 25)");
    }

    #[test]
    fn dependents_that_pass_keep_their_own_text_with_the_route_also_true() {
        let t = thresholds().repo_route;
        let signals = Signals {
            stars: Some(t.min_stars),
            forks: Some(t.min_forks),
            contributors: Some(t.min_contributors),
            ..featured_library()
        };
        assert!(
            check(&signals)
                .reason
                .detail
                .starts_with("featured: active_users 50")
        );
    }

    #[test]
    fn an_unknown_route_part_with_low_dependents_is_unknown_and_never_false() {
        let t = thresholds().repo_route;
        let found = check(&low_dependents(
            Some(t.min_stars),
            None,
            Some(t.min_contributors),
        ));
        assert_eq!(found.fact.value, None);
        assert_eq!(found.fact.needs, ["forks"]);
        assert_eq!(found.reason.detail, "not featured: unknown forks");
    }

    #[test]
    fn a_route_part_below_its_limit_states_the_unknown_part_too() {
        let found = low_dependents(Some(1), None, Some(1));
        fails(
            &found,
            "repository route stars 1 (needs 2000), forks unknown (needs 200), contributors 1 (needs 25)",
        );
    }

    #[test]
    fn the_route_leaves_every_other_library_gate_in_force() {
        let vulnerable = Signals {
            unpatched_vulnerabilities: Some(vec!["RUSTSEC-2026-0004".to_owned()]),
            ..on_the_route()
        };
        fails(&vulnerable, "unpatched vulnerabilities exist");
        let unverified = Signals {
            repo_link_verified: Some(false),
            ..on_the_route()
        };
        fails(&unverified, "repository link not verified");
        let faded = Signals {
            former_glory: Some(0.1),
            ..on_the_route()
        };
        fails(&faded, "former_glory 0.1 < 0.5");
        let undocumented = Signals {
            docs_present: Some(false),
            ..on_the_route()
        };
        fails(&undocumented, "docs missing");
        let prerelease = Signals {
            all_prerelease: Some(true),
            ..on_the_route()
        };
        fails(&prerelease, "every version is a prerelease or 0.0.x");
        let inactive = featured(&on_the_route(), Fact::known(false), &thresholds());
        assert!(inactive.fact.is_false());
    }

    #[test]
    fn an_app_that_passes_on_the_route_names_it_and_one_that_does_not_stays_silent() {
        let t = thresholds();
        let route = t.repo_route;
        let signals = Signals {
            homebrew_on_request_365d: Some(0),
            repology_families: Some(0),
            docker_pull_growth_90d: Some(0),
            stars: Some(route.min_stars),
            forks: Some(route.min_forks),
            contributors: Some(route.min_contributors),
            ..featured_app()
        };
        let found = check(&signals);
        assert!(found.fact.is_true(), "{found:?}");
        assert!(
            found
                .reason
                .detail
                .ends_with(", repository route stars 2000 (needs 2000), forks 200 (needs 200), contributors 25 (needs 25)"),
            "{}",
            found.reason.detail
        );
        assert!(
            !check(&featured_app())
                .reason
                .detail
                .contains("repository route")
        );
    }

    #[test]
    fn a_reason_rounds_float_noise_and_the_comparison_keeps_the_raw_value() {
        let noisy = Signals {
            active_users: Some(2.9999999999999996),
            new_adopters: Some(1.0),
            ..featured_library()
        };
        fails(&noisy, "active_users 3 < 10 and new_adopters 1 < 5");

        let thresholds = thresholds();
        let limit = thresholds.library_featured.min_active_users;
        let just_below = Signals {
            active_users: Some(limit - 1e-12),
            new_adopters: Some(0.0),
            ..featured_library()
        };
        let found = featured(&just_below, Fact::known(true), &thresholds);
        assert!(found.fact.is_false(), "{found:?}");
        assert!(
            found.reason.detail.contains("active_users 9.99 < 10"),
            "{}",
            found.reason.detail
        );

        let featured_noise = Signals {
            active_users: Some(27.999999999999996),
            new_adopters: Some(289.00000000000006),
            former_glory: Some(0.9036144578313253),
            ..featured_library()
        };
        assert_eq!(
            check(&featured_noise).reason.detail,
            "featured: active_users 28, new_adopters 289, former_glory 0.9"
        );
    }

    #[test]
    fn each_library_gate_fails_on_its_own() {
        let vulnerable = Signals {
            unpatched_vulnerabilities: Some(vec!["RUSTSEC-2026-0004".to_owned()]),
            ..featured_library()
        };
        fails(&vulnerable, "unpatched vulnerabilities exist");
        let unverified = Signals {
            repo_link_verified: Some(false),
            ..featured_library()
        };
        fails(&unverified, "repository link not verified");
        let faded = Signals {
            former_glory: Some(0.49),
            ..featured_library()
        };
        fails(&faded, "former_glory 0.49 < 0.5");
        let at_glory = Signals {
            former_glory: Some(0.5),
            ..featured_library()
        };
        assert!(check(&at_glory).fact.is_true());
        let undocumented = Signals {
            docs_present: Some(false),
            ..featured_library()
        };
        fails(&undocumented, "docs missing");
    }

    #[test]
    fn inactive_entries_are_not_featured() {
        let found = featured(&featured_library(), Fact::known(false), &thresholds());
        assert!(found.fact.is_false());
        assert!(found.reason.detail.contains("dormant and not stable"));
    }

    #[test]
    fn unknown_library_gates_are_named() {
        let signals = Signals {
            repo_link_verified: None,
            docs_present: None,
            ..featured_library()
        };
        let found = check(&signals);
        assert_eq!(found.fact.value, None);
        assert_eq!(found.fact.needs, ["repo_link_verified", "docs_present"]);
        assert_eq!(
            found.reason.detail,
            "not featured: unknown repo_link_verified, docs_present"
        );
        let unknown = check(&Signals::unknown(EntryKind::Library));
        assert_eq!(unknown.fact.value, None);
    }

    #[test]
    fn a_failed_gate_hides_unknown_ones() {
        let signals = Signals {
            docs_present: Some(false),
            repo_link_verified: None,
            ..featured_library()
        };
        let found = check(&signals);
        assert!(found.fact.is_false());
        assert!(found.fact.needs.is_empty());
    }

    #[test]
    fn all_prerelease_library_is_not_featured() {
        let signals = Signals {
            all_prerelease: Some(true),
            ..featured_library()
        };
        fails(&signals, "every version is a prerelease or 0.0.x");
    }

    #[test]
    fn unknown_all_prerelease_keeps_a_library_out_of_featured() {
        let signals = Signals {
            all_prerelease: None,
            ..featured_library()
        };
        let found = check(&signals);
        assert_eq!(found.fact.value, None);
        assert_eq!(found.fact.needs, ["all_prerelease"]);
    }

    #[test]
    fn app_gate_applies_only_when_all_prerelease_is_known() {
        let prerelease = Signals {
            all_prerelease: Some(true),
            ..featured_app()
        };
        fails(&prerelease, "every version is a prerelease or 0.0.x");
        let no_crate = Signals {
            all_prerelease: None,
            ..featured_app()
        };
        assert!(check(&no_crate).fact.is_true());
    }

    #[test]
    fn app_ignores_library_signals() {
        let signals = Signals {
            docs_present: Some(false),
            former_glory: Some(0.0),
            ..featured_app()
        };
        assert!(check(&signals).fact.is_true());
    }
}
