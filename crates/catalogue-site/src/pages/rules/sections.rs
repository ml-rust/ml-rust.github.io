//! The static rule content, in the order the check applies the rules.

use catalogue_core::Thresholds;

use super::rows::{
    ThresholdRow, app_rows, dead_rows, fallback_rows, finished_rows, interval_rows, library_rows,
    new_rows, rot_rows,
};

/// One rule with its text and thresholds.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuleSection {
    /// The heading id.
    pub id: &'static str,
    /// The heading.
    pub title: &'static str,
    /// Paragraphs before the list.
    pub lead: &'static [&'static str],
    /// The list items.
    pub items: &'static [Item],
    /// Paragraphs after the list.
    pub notes: &'static [&'static str],
    /// The thresholds this rule reads. Empty without a thresholds file.
    pub rows: Vec<ThresholdRow>,
}

/// One list item, with an optional short label such as `R1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Item {
    /// The label. Empty for none.
    pub label: &'static str,
    /// The item text.
    pub text: &'static str,
}

/// Every rule in check order. Rows come from `thresholds`, or stay empty without it.
pub(super) fn sections(thresholds: Option<&Thresholds>) -> Vec<RuleSection> {
    let rows =
        |make: fn(&Thresholds) -> Vec<ThresholdRow>| thresholds.map(make).unwrap_or_default();
    vec![
        RuleSection {
            id: "instant-archive",
            title: "Instant archive",
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
            id: "interval",
            title: "Expected update interval",
            lead: &[
                "Each entry gets an expected update interval E, in days.",
                "A library starts from a base chosen by its latest version. The first matching base in the table applies.",
            ],
            items: &[],
            notes: &[
                "Then E is capped at the release count times the days-per-release cap.",
                "Then E doubles when monthly downloads pass their limit. It doubles again when active users pass theirs.",
                "E is never below the dormancy floor.",
                "An app, or a library with no known latest version, uses the dormancy floor.",
                "A step that lacks its signal is left out.",
            ],
            rows: rows(interval_rows),
        },
        RuleSection {
            id: "dormancy",
            title: "Dormancy",
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
                "Former glory is active users as a share of their peak.",
                "R3 and R6 apply to libraries only. Apps have no former glory and no crate release to check.",
                "A sandbox build check older than its age limit counts as unknown.",
                "A rot check that lacks a signal counts neither way.",
            ],
            rows: rows(rot_rows),
        },
        RuleSection {
            id: "dead",
            title: "Dead",
            lead: &[
                "A dormant entry is dead on a check when it has enough rot signals and no “maintained” statement.",
                "Enough dead checks in a row archive the entry.",
                "Unknown dormancy is not dead.",
            ],
            items: &[],
            notes: &[],
            rows: rows(dead_rows),
        },
        RuleSection {
            id: "finished",
            title: "Finished",
            lead: &[
                "A dormant entry with no rot signal that is still adopted is finished.",
                "A “finished” maintainer statement with no rot signal also makes an entry finished.",
                "No rot signal means every rot check is known and none fires.",
            ],
            items: &[],
            notes: &[
                "A library stays adopted on new adopters or on former glory.",
                "An app stays adopted on binary downloads per release.",
                "A finished entry carries a stable label. It counts as active for Featured.",
            ],
            rows: rows(finished_rows),
        },
        RuleSection {
            id: "fallback",
            title: "Failed fetches",
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
        RuleSection {
            id: "new",
            title: "New",
            lead: &["An entry is New when any one of these holds."],
            items: &[
                Item {
                    label: "",
                    text: "Its first release is younger than the age limit.",
                },
                Item {
                    label: "",
                    text: "Every version is a prerelease or 0.0.x.",
                },
                Item {
                    label: "",
                    text: "A library has fewer active users than the user floor.",
                },
            ],
            notes: &[],
            rows: rows(new_rows),
        },
        RuleSection {
            id: "featured-library",
            title: "Featured library",
            lead: &[
                "A library is Featured when it is active and passes every gate.",
                "Active means not dormant, or finished.",
            ],
            items: &[
                Item {
                    label: "",
                    text: "Active users or new adopters reach their limit.",
                },
                Item {
                    label: "",
                    text: "No vulnerability advisory is unpatched past the grace window.",
                },
                Item {
                    label: "",
                    text: "The commit in the crate’s .cargo_vcs_info.json is on a branch or tag of the repository. A commit that only a fork holds does not count.",
                },
                Item {
                    label: "",
                    text: "Former glory reaches its floor.",
                },
                Item {
                    label: "",
                    text: "Docs are present.",
                },
            ],
            notes: &["A gate that lacks its signal does not pass."],
            rows: rows(library_rows),
        },
        RuleSection {
            id: "featured-app",
            title: "Featured app",
            lead: &["An app is Featured when it is active and enough of these use signals pass."],
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
            ],
            notes: &[
                "Apps have no dependents signal. ecosyste.ms lists no dependents for a repository that publishes no package.",
            ],
            rows: rows(app_rows),
        },
        RuleSection {
            id: "outside-approval",
            title: "Outside approval",
            lead: &[
                "An org-affiliated entry that passes Featured needs a featured-approval judgment.",
                "A maintainer outside the ml-rust organization approves it. Without it, the entry is Listed.",
            ],
            items: &[],
            notes: &[],
            rows: Vec::new(),
        },
        RuleSection {
            id: "listed",
            title: "Listed",
            lead: &["An entry that no rule above places is Listed."],
            items: &[],
            notes: &[],
            rows: Vec::new(),
        },
        RuleSection {
            id: "demotion-guard",
            title: "Demotion guard",
            lead: &[
                "A Featured entry keeps Featured until a demotion judgment approves the drop.",
                "An instant archive skips this guard.",
            ],
            items: &[],
            notes: &[],
            rows: Vec::new(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::super::fixtures::thresholds;
    use super::*;

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
    fn section_ids_are_unique() {
        let all = sections(None);
        let ids: BTreeSet<&str> = all.iter().map(|section| section.id).collect();
        assert_eq!(ids.len(), all.len());
    }

    #[test]
    fn the_rot_list_holds_r1_to_r6_in_order() {
        let all = sections(None);
        let rot = all.iter().find(|section| section.id == "rot").expect("rot");
        let labels: Vec<&str> = rot.items.iter().map(|item| item.label).collect();
        assert_eq!(labels, ["R1", "R2", "R3", "R4", "R5", "R6"]);
    }
}
