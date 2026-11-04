use super::app::app_gate;
use super::fact::{Fact, show, show_amount, show_below};
use super::signals::Signals;
use super::verdict::{Reason, Rule};
use crate::registry::{LibraryFeaturedThresholds, Thresholds};

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

/// The library gates: adoption by active users or new adopters, no unpatched
/// vulnerability, a verified repository link, `former_glory`, and docs.
fn library_gates(signals: &Signals, t: &LibraryFeaturedThresholds) -> Vec<Gate> {
    vec![
        Gate {
            fact: Fact::any(vec![
                Fact::of("active_users", signals.active_users, |n| {
                    n >= t.min_active_users
                }),
                Fact::of("new_adopters", signals.new_adopters, |n| {
                    n >= t.min_new_adopters
                }),
            ]),
            failure: format!(
                "active_users {} < {} and new_adopters {} < {}",
                show_below(signals.active_users, t.min_active_users),
                t.min_active_users,
                show_below(signals.new_adopters, t.min_new_adopters),
                t.min_new_adopters
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

/// Rule 8: the Featured gates for the entry's kind, plus activity: not dormant, or stable.
///
/// `active` is that activity condition. Unknown gates leave the rule unknown, so the
/// entry is not featured.
pub(super) fn featured(signals: &Signals, active: Fact, thresholds: &Thresholds) -> Featured {
    let mut gates = if signals.is_library() {
        library_gates(signals, &thresholds.library_featured)
    } else {
        vec![app_gate(signals, &thresholds.app_featured)]
    };
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
        format!(
            "active_users {}, new_adopters {}, former_glory {}",
            show_amount(signals.active_users),
            show_amount(signals.new_adopters),
            show_amount(signals.former_glory)
        )
    } else {
        format!(
            "binary downloads {}, Homebrew {}, Repology families {}, Docker growth {}",
            show(signals.binary_downloads_last3),
            show(signals.homebrew_on_request_365d),
            show(signals.repology_families),
            show(signals.docker_pull_growth_90d)
        )
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
        fails(&neither, "active_users 9.9 < 10 and new_adopters 4.9 < 5");
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
    fn app_ignores_library_signals() {
        let signals = Signals {
            docs_present: Some(false),
            former_glory: Some(0.0),
            ..featured_app()
        };
        assert!(check(&signals).fact.is_true());
    }
}
