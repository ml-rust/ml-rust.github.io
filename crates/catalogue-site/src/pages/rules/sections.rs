//! The static rule content, in groups a reader follows: each tier first, then what the tiers read.

use catalogue_core::Thresholds;

use super::rows::{
    ThresholdRow, app_rows, dead_rows, fallback_rows, finished_rows, interval_rows, library_rows,
    movement_rows, new_rows, repo_route_rows, rot_rows,
};

/// One rule with its text and thresholds.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuleSection {
    /// The heading id.
    pub id: &'static str,
    /// The heading.
    pub title: &'static str,
    /// The tier the rule places an entry in.
    pub tier: Option<TierWord>,
    /// Paragraphs before the list.
    pub lead: &'static [&'static str],
    /// The list items.
    pub items: &'static [Item],
    /// Paragraphs after the list.
    pub notes: &'static [&'static str],
    /// The thresholds this rule reads. Empty without a thresholds file.
    pub rows: Vec<ThresholdRow>,
}

/// A tier name and the class that colours it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TierWord {
    /// The tier name.
    pub name: &'static str,
    /// The class of the tier.
    pub class: &'static str,
}

/// Rules that belong together, under one heading.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuleGroup {
    /// The heading id.
    pub id: &'static str,
    /// The heading.
    pub title: &'static str,
    /// The rules of the group.
    pub sections: Vec<RuleSection>,
}

/// One list item, with an optional short label such as `R1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Item {
    /// The label. Empty for none.
    pub label: &'static str,
    /// The item text.
    pub text: &'static str,
}

/// Every rule, in groups. Rows come from `thresholds`, or stay empty without it.
pub(super) fn groups(thresholds: Option<&Thresholds>) -> Vec<RuleGroup> {
    let rows =
        |make: fn(&Thresholds) -> Vec<ThresholdRow>| thresholds.map(make).unwrap_or_default();
    vec![
        RuleGroup {
            id: "tier-rules",
            title: "What each tier requires",
            sections: vec![
                RuleSection {
                    id: "featured-library",
                    title: "Featured library",
                    tier: Some(TierWord {
                        name: "Featured",
                        class: "tier-featured",
                    }),
                    lead: &[
                        "A library is Featured when it is active and passes every gate.",
                        "Active means not dormant, or finished.",
                    ],
                    items: &[
                        Item {
                            label: "",
                            text: "Adoption passes on one of three routes: active dependents reach their limit, new dependents (365 days) reach their limit, or the repository route passes.",
                        },
                        Item {
                            label: "",
                            text: "No vulnerability advisory is unpatched past the grace window.",
                        },
                        Item {
                            label: "",
                            text: "The commit in the crate’s .cargo_vcs_info.json is on a branch or tag of the repository. A commit that only a fork holds does not count. When the release records no commit, or the repository is on Codeberg or GitLab, a Cargo.toml on the default branch that declares the crate counts instead. A recorded commit that the GitHub repository lacks still fails.",
                        },
                        Item {
                            label: "",
                            text: "Former glory reaches its floor.",
                        },
                        Item {
                            label: "",
                            text: "Docs are present.",
                        },
                        Item {
                            label: "",
                            text: "At least one version is neither a prerelease nor 0.0.x. A library with only prereleases passes this gate when its active or new dependents reach their limit. The repository route never lifts it. A library whose every version is 0.0.x never passes, because that only reserves the name.",
                        },
                    ],
                    notes: &["A gate that lacks its signal does not pass."],
                    rows: rows(library_rows),
                },
                RuleSection {
                    id: "featured-app",
                    title: "Featured app",
                    tier: Some(TierWord {
                        name: "Featured",
                        class: "tier-featured",
                    }),
                    lead: &[
                        "An app is Featured when it is active and enough of these use signals pass.",
                        "An app has no dependents, so the repository route counts as one use signal.",
                    ],
                    items: &[
                        Item {
                            label: "",
                            text: "Binary downloads over the last three releases.",
                        },
                        Item {
                            label: "",
                            text: "Homebrew installs on request over 365 days.",
                        },
                        Item {
                            label: "",
                            text: "Repology repository families that package the app.",
                        },
                        Item {
                            label: "",
                            text: "Docker pull growth over 90 days.",
                        },
                        Item {
                            label: "",
                            text: "The repository route.",
                        },
                    ],
                    notes: &[
                        "An app with no crate skips the release gate. An app that publishes a crate needs a version outside prereleases and 0.0.x.",
                        "Apps have no dependents signal. ecosyste.ms lists no dependents for a repository that publishes no package.",
                    ],
                    rows: rows(app_rows),
                },
                RuleSection {
                    id: "repository-route",
                    title: "Repository route",
                    tier: Some(TierWord {
                        name: "Featured",
                        class: "tier-featured",
                    }),
                    lead: &[
                        "The repository route passes when stars, forks and contributors each reach their limit on the same check.",
                        "Stars alone never qualify a project. GitHub hides who starred, so stars cannot be screened for fakes. The route also needs forks and contributors, because faking all three together is much harder.",
                        "The route measures a whole repository, so an entry with crates must list the repository's main crate: the crate with the repository's own name that claims the repository. An entry that lists only side crates does not pass, whatever the numbers say.",
                        "A library can pass its adoption gate on this route. An app counts it as one use signal. Every other Featured gate still applies.",
                        "This is a gate of separate limits. No page adds them into a score.",
                    ],
                    items: &[
                        Item {
                            label: "Stars",
                            text: "The star count GitHub states. Raw key stars.",
                        },
                        Item {
                            label: "Forks",
                            text: "The fork count GitHub states. Raw key forks. A snapshot from before the check read forks reads as not measured.",
                        },
                        Item {
                            label: "Contributors",
                            text: "The contributors with a GitHub account, from one GitHub request. Raw key contributors. A list GitHub calls too large reads as not measured.",
                        },
                        Item {
                            label: "Commits",
                            text: "The total commits on the default branch. Raw key commits. Each entry page shows it, and no tier rule reads it.",
                        },
                    ],
                    notes: &[
                        "A value below its limit fails the route. Otherwise a value that is not measured leaves the route unknown, and an unknown route does not pass.",
                    ],
                    rows: rows(repo_route_rows),
                },
                RuleSection {
                    id: "new",
                    title: "New",
                    tier: Some(TierWord {
                        name: "New",
                        class: "tier-new",
                    }),
                    lead: &[
                        "An entry is New when its first release is younger than the age limit.",
                    ],
                    items: &[],
                    notes: &[
                        "Age alone decides. An old entry with few dependents or only prerelease versions is not New.",
                    ],
                    rows: rows(new_rows),
                },
                RuleSection {
                    id: "listed",
                    title: "Listed",
                    tier: Some(TierWord {
                        name: "Listed",
                        class: "tier-listed",
                    }),
                    lead: &["An entry that no rule above places is Listed."],
                    items: &[],
                    notes: &[],
                    rows: Vec::new(),
                },
                RuleSection {
                    id: "archived",
                    title: "Archived",
                    tier: Some(TierWord {
                        name: "Archived",
                        class: "tier-archived",
                    }),
                    lead: &[
                        "An entry is Archived when an instant archive signal fires, or when enough dead checks come in a row.",
                    ],
                    items: &[],
                    notes: &[],
                    rows: Vec::new(),
                },
                RuleSection {
                    id: "instant-archive",
                    title: "Instant archive",
                    tier: Some(TierWord {
                        name: "Archived",
                        class: "tier-archived",
                    }),
                    lead: &["Any one of these signals archives an entry at once."],
                    items: &[
                        Item {
                            label: "",
                            text: "The repository is archived.",
                        },
                        Item {
                            label: "",
                            text: "RustSec lists an unmaintained advisory for the crate.",
                        },
                        Item {
                            label: "",
                            text: "The crate carries a deprecation notice.",
                        },
                        Item {
                            label: "",
                            text: "Every version is yanked.",
                        },
                        Item {
                            label: "",
                            text: "The crate is deleted from crates.io.",
                        },
                    ],
                    notes: &["A “maintained” statement does not override these signals."],
                    rows: Vec::new(),
                },
                RuleSection {
                    id: "dead",
                    title: "Dead",
                    tier: Some(TierWord {
                        name: "Archived",
                        class: "tier-archived",
                    }),
                    lead: &[
                        "A dormant entry is dead on a check when it has enough rot signals and no “maintained” statement.",
                        "Enough dead checks in a row archive the entry.",
                        "Unknown dormancy is not dead.",
                    ],
                    items: &[],
                    notes: &[],
                    rows: rows(dead_rows),
                },
            ],
        },
        RuleGroup {
            id: "activity",
            title: "How the check reads activity",
            sections: vec![
                RuleSection {
                    id: "interval",
                    title: "Expected update interval",
                    tier: None,
                    lead: &[
                        "Each entry gets an expected update interval E, in days.",
                        "A library starts from a base chosen by its latest version. The first matching base in the table applies.",
                    ],
                    items: &[],
                    notes: &[
                        "Then E is capped at the release count times the days-per-release cap.",
                        "Then E doubles when monthly downloads pass their limit. It doubles again when active dependents pass theirs.",
                        "E is never below the dormancy floor.",
                        "An app, or a library with no known latest version, uses the dormancy floor.",
                        "A step that lacks its signal is left out.",
                    ],
                    rows: rows(interval_rows),
                },
                RuleSection {
                    id: "dormancy",
                    title: "Dormancy",
                    tier: None,
                    lead: &[
                        "An entry is dormant when the days since its latest release or latest human commit reach E.",
                        "Bot commits do not count.",
                        "With neither date known, dormancy is unknown.",
                    ],
                    items: &[],
                    notes: &[],
                    rows: Vec::new(),
                },
                RuleSection {
                    id: "rot",
                    title: "Rot signals",
                    tier: None,
                    lead: &["Each check counts six rot signals."],
                    items: &[
                        Item {
                            label: "R1",
                            text: "An outside issue is unanswered past the issue limit, and no maintainer replied within the reply window.",
                        },
                        Item {
                            label: "R2",
                            text: "A vulnerability advisory is unpatched past the grace window.",
                        },
                        Item {
                            label: "R3",
                            text: "A library’s former glory is below the lost-glory limit.",
                        },
                        Item {
                            label: "R4",
                            text: "The latest release requires a deprecated dependency.",
                        },
                        Item {
                            label: "R5",
                            text: "The repository no longer exists.",
                        },
                        Item {
                            label: "R6",
                            text: "A library’s latest release fails cargo check on current stable Rust.",
                        },
                    ],
                    notes: &[
                        "Former glory is active dependents as a share of their peak.",
                        "R3 and R6 apply to libraries only. Apps have no former glory and no crate release to check.",
                        "A sandbox build check older than its age limit counts as unknown.",
                        "A rot check that lacks a signal counts neither way.",
                    ],
                    rows: rows(rot_rows),
                },
                RuleSection {
                    id: "finished",
                    title: "Finished",
                    tier: None,
                    lead: &[
                        "A dormant entry with no rot signal that is still adopted is finished.",
                        "A “finished” maintainer statement with no rot signal also makes an entry finished.",
                        "No rot signal means every rot check is known and none fires.",
                    ],
                    items: &[],
                    notes: &[
                        "A library stays adopted on new dependents (365 days) or on former glory.",
                        "An app stays adopted on binary downloads per release.",
                        "A finished entry carries a stable label. It counts as active for Featured.",
                    ],
                    rows: rows(finished_rows),
                },
            ],
        },
        RuleGroup {
            id: "review",
            title: "Review and failed fetches",
            sections: vec![
                RuleSection {
                    id: "outside-approval",
                    title: "Outside approval",
                    tier: None,
                    lead: &[
                        "An org-affiliated entry that passes Featured needs a featured-approval judgment.",
                        "A maintainer outside the ml-rust organization approves it. Without it, the entry is Listed.",
                    ],
                    items: &[],
                    notes: &[],
                    rows: Vec::new(),
                },
                RuleSection {
                    id: "demotion-guard",
                    title: "Demotion guard",
                    tier: None,
                    lead: &[
                        "A Featured entry keeps Featured until a demotion judgment approves the drop.",
                        "An instant archive skips this guard.",
                    ],
                    items: &[],
                    notes: &[],
                    rows: Vec::new(),
                },
                RuleSection {
                    id: "refresh",
                    title: "How values refresh",
                    tier: None,
                    lead: &[
                        "A weekly scan fetches repo facts, app distribution counts, the crates.io dump and the RustSec archive.",
                        "The contributor count and the README refresh only after a newer push, after a new version, or once the value passes its refresh age.",
                        "The repo link check refreshes only after a new version, or once the value passes its refresh age.",
                        "The refresh age of an entry is between 30 and 57 days. The entry slug sets it, so refreshes spread over several scans.",
                        "A run between scans fetches new entries at most, and reuses every other value. When the last scan is over 8 days old, the next run scans instead.",
                    ],
                    items: &[],
                    notes: &[
                        "A reused value keeps its original fetch date.",
                        "A run that reuses the values of an entry is no check of that entry for the dead rule.",
                    ],
                    rows: Vec::new(),
                },
                RuleSection {
                    id: "fallback",
                    title: "Failed fetches",
                    tier: None,
                    lead: &[
                        "A network signal that fails to fetch keeps its value from the previous snapshot.",
                        "That value keeps its original fetch date, and the entry notes name the source.",
                        "A value past the age limit counts as unknown.",
                    ],
                    items: &[],
                    notes: &[
                        "An answer that a repo or a name does not exist counts as a signal. It never falls back.",
                        "Values from the crates.io dump and the RustSec archive never fall back. A run without them stops.",
                    ],
                    rows: rows(fallback_rows),
                },
            ],
        },
        RuleGroup {
            id: "facts",
            title: "Facts, lists and movement",
            sections: vec![
                RuleSection {
                    id: "active-dependents",
                    title: "Active dependents",
                    tier: None,
                    lead: &[
                        "Active dependents: crates from other owners that depend on this project in a current release. One owner counts at most once.",
                        "A crate with several owners splits its count among them, so the raw value can hold a fraction. Lists and sentences round it.",
                        "The project's own owners, its publishers and shared maintenance accounts never count. A dependency stops counting when the crate publishes a release without it, or when its latest release grows too old.",
                    ],
                    items: &[
                        Item {
                            label: "Active dependents",
                            text: "Raw key active_users. The Featured rule reads this number.",
                        },
                        Item {
                            label: "New dependents (365 days)",
                            text: "Raw key new_adopters. Active dependents whose dependency began in the last 365 days.",
                        },
                        Item {
                            label: "Peak active dependents",
                            text: "Raw key peak_users. The highest level on record, from monthly samples.",
                        },
                        Item {
                            label: "Former glory",
                            text: "Raw key former_glory. Active dependents as a share of the peak.",
                        },
                    ],
                    notes: &[
                        "The raw keys keep their names in the data files and the rule outcomes.",
                    ],
                    rows: Vec::new(),
                },
                RuleSection {
                    id: "downloads",
                    title: "Downloads",
                    tier: None,
                    lead: &[
                        "A library reads the downloads of its primary crate over the last 30 days, from the crates.io database dump. Raw key downloads_per_month.",
                        "An app with binary downloads reads them per release: GitHub release asset downloads over up to its last 3 releases, divided by those releases. Raw key binary_downloads_per_release.",
                        "An app with no binary download reads the downloads of its primary crate over the last 30 days, when it lists a crate. No tier rule reads that number.",
                        "An app with no release and no crate reads as not measured, never as 0.",
                    ],
                    items: &[],
                    notes: &[
                        "Lists sort by these numbers. The tier rules read only the limits this page names.",
                    ],
                    rows: Vec::new(),
                },
                RuleSection {
                    id: "choice-facts",
                    title: "Facts that never count",
                    tier: None,
                    lead: &[
                        "These facts help a person choose a project. They never count toward a tier.",
                        "Each comes from measured data, and each row in the signal table links its source.",
                    ],
                    items: &[
                        Item {
                            label: "License",
                            text: "A library reads the license of its primary crate's default version on crates.io, as an SPDX expression. A primary crate with no license column falls back to the license GitHub states. An app reads the license GitHub states. So does a library the dump lacks. A license nobody can name reads as not measured. No license in either source reads as none stated.",
                        },
                        Item {
                            label: "Runs on",
                            text: "Each listed crate's default version is matched against data/backends.toml, by its direct dependency names and its feature names. A feature name means the code offers the backend, and it does not prove the backend works. GPU support unknown means the dump holds the crates and no row matches. The project can still run on a GPU. A backend behind a deeper dependency stays unseen. So does a backend that needs no feature.",
                        },
                        Item {
                            label: "Native code",
                            text: "No native code found, or what the project binds. The check follows the normal dependencies of each listed crate, and theirs in turn. A hit is a crate named -sys or a crate with a links key. Crates that bind no foreign library never count. An optional dependency of a listed crate names the feature that enables it. A build dependency such as cc or cmake shows that the build uses a native toolchain. C built with no links key, no -sys crate and no such build dependency stays unseen. So does native code kept outside crates.io.",
                        },
                        Item {
                            label: "Python bindings",
                            text: "A direct dependency that data/backends.toml lists. A required one needs Python. An optional one needs Python only with the feature that enables it. No Python binding crate found means no such crate shows on crates.io. Bindings kept outside crates.io stay unseen.",
                        },
                        Item {
                            label: "Dependents",
                            text: "The distinct crates outside the entry whose default version directly depends on a listed crate. Dev dependencies and crates with every version yanked stay out. An app reads as not applicable. Active dependents weigh history and owners, so the two numbers differ.",
                        },
                    ],
                    notes: &["An unknown value reads as not measured."],
                    rows: Vec::new(),
                },
                RuleSection {
                    id: "list-order",
                    title: "List order",
                    tier: None,
                    lead: &[
                        "Lists show the strongest tier first, then the most downloads, then the name. A value not measured sorts last.",
                        "A library counts downloads over 30 days. An app counts binary downloads per release. An app with none counts its crate downloads over 30 days. The two units differ, and each row names its own.",
                        "This is an order and never a score. No page prints a rank, and no tier reads it.",
                        "A list shows one row per project family. A project that belongs to a bigger project in another repo keeps its own tier and its own page.",
                    ],
                    items: &[],
                    notes: &[],
                    rows: Vec::new(),
                },
                RuleSection {
                    id: "movement",
                    title: "Movement",
                    tier: None,
                    lead: &[
                        "Movement comes from the stored snapshots. A weekly scan adds one each week. It never affects a tier.",
                    ],
                    items: &[
                        Item {
                            label: "New",
                            text: "The entry first appears in the history inside the window. The first day of history does not count, because every entry appears then.",
                        },
                        Item {
                            label: "Rising",
                            text: "Active dependents rose by at least the set share and the set number, between the snapshot nearest the window start and the newest snapshot.",
                        },
                        Item {
                            label: "Falling",
                            text: "Active dependents fell by at least the set share and the set number, over the same two snapshots.",
                        },
                    ],
                    notes: &[
                        "A value missing at either end gives no movement. History shorter than the window gives none, and pages say movement needs more history.",
                    ],
                    rows: rows(movement_rows),
                },
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::super::fixtures::thresholds;
    use super::*;

    /// Every section of every group, in page order.
    fn sections(thresholds: Option<&Thresholds>) -> Vec<RuleSection> {
        groups(thresholds)
            .into_iter()
            .flat_map(|group| group.sections)
            .collect()
    }

    /// Every `table.field` key that serde writes for `t`, except `version` and `calibrated`.
    fn threshold_keys(t: &Thresholds) -> BTreeSet<String> {
        let value = serde_json::to_value(t).expect("thresholds serialize");
        let mut keys = BTreeSet::new();
        for (table, fields) in value.as_object().expect("object") {
            if table == "version" || table == "calibrated" {
                continue;
            }
            for field in fields.as_object().expect("table").keys() {
                keys.insert(format!("{table}.{field}"));
            }
        }
        keys
    }

    #[test]
    fn every_threshold_has_exactly_one_row() {
        let t = thresholds(None);
        let rows: Vec<String> = sections(Some(&t))
            .into_iter()
            .flat_map(|section| section.rows)
            .map(|row| row.key.to_owned())
            .collect();
        let unique: BTreeSet<String> = rows.iter().cloned().collect();
        assert_eq!(unique.len(), rows.len(), "a key repeats: {rows:?}");
        assert_eq!(unique, threshold_keys(&t));
    }

    #[test]
    fn without_thresholds_no_section_has_rows() {
        assert!(sections(None).iter().all(|section| section.rows.is_empty()));
    }

    #[test]
    fn section_and_group_ids_are_unique() {
        let all = sections(None);
        let mut ids: BTreeSet<&str> = all.iter().map(|section| section.id).collect();
        assert_eq!(ids.len(), all.len());
        let every = groups(None);
        ids.extend(every.iter().map(|group| group.id));
        assert_eq!(ids.len(), all.len() + every.len());
    }

    #[test]
    fn the_tier_rules_come_first_and_each_opens_with_a_statement() {
        let every = groups(None);
        let first = every.first().expect("a group");
        assert_eq!(first.id, "tier-rules");
        for section in &first.sections {
            assert!(section.tier.is_some(), "{}", section.id);
            assert!(!section.lead.is_empty(), "{}", section.id);
        }
    }

    #[test]
    fn the_repository_route_states_the_rule_and_its_limits() {
        let t = thresholds(None);
        let all = sections(Some(&t));
        let route = all
            .iter()
            .find(|section| section.id == "repository-route")
            .expect("the route");
        let lead = route.lead.join(" ");
        assert!(
            lead.contains("Stars alone never qualify a project."),
            "{lead}"
        );
        assert!(lead.contains("GitHub hides who starred"), "{lead}");
        assert!(lead.contains("forks and contributors"), "{lead}");
        let keys: Vec<&str> = route.rows.iter().map(|row| row.key).collect();
        assert_eq!(
            keys,
            [
                "repo_route.min_stars",
                "repo_route.min_forks",
                "repo_route.min_contributors"
            ]
        );
        let library = all
            .iter()
            .find(|section| section.id == "featured-library")
            .expect("the library rule");
        assert!(library.items[0].text.contains("repository route"));
        let app = all
            .iter()
            .find(|section| section.id == "featured-app")
            .expect("the app rule");
        assert!(
            app.items
                .iter()
                .any(|item| item.text == "The repository route.")
        );
    }

    #[test]
    fn the_rot_list_holds_r1_to_r6_in_order() {
        let all = sections(None);
        let rot = all.iter().find(|section| section.id == "rot").expect("rot");
        let labels: Vec<&str> = rot.items.iter().map(|item| item.label).collect();
        assert_eq!(labels, ["R1", "R2", "R3", "R4", "R5", "R6"]);
    }
}
