//! The threshold rows each rule shows beside its text.

use std::fmt::Display;

use catalogue_core::Thresholds;

/// One threshold: its key in `data/thresholds.toml`, its value and its meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThresholdRow {
    /// `table.field` in `data/thresholds.toml`.
    pub key: &'static str,
    /// The value as written by `Display`.
    pub value: String,
    /// What the value limits.
    pub meaning: &'static str,
}

/// A threshold row for `key` with `value`.
fn row(key: &'static str, value: impl Display, meaning: &'static str) -> ThresholdRow {
    ThresholdRow {
        key,
        value: value.to_string(),
        meaning,
    }
}

/// The expected interval bases, cap, doubling limits and floor.
pub(super) fn interval_rows(t: &Thresholds) -> Vec<ThresholdRow> {
    let i = &t.interval;
    vec![
        row(
            "interval.patch_gt3_major_ge1",
            i.patch_gt3_major_ge1,
            "Base in days: patch above 3 and major at least 1.",
        ),
        row(
            "interval.patch_gt3",
            i.patch_gt3,
            "Base in days: patch above 3.",
        ),
        row(
            "interval.patch_gt0",
            i.patch_gt0,
            "Base in days: patch above 0.",
        ),
        row(
            "interval.major_ge1",
            i.major_ge1,
            "Base in days: major at least 1.",
        ),
        row(
            "interval.minor_gt3",
            i.minor_gt3,
            "Base in days: minor above 3.",
        ),
        row(
            "interval.other",
            i.other,
            "Base in days: every other version.",
        ),
        row(
            "interval.days_per_release_cap",
            i.days_per_release_cap,
            "Days per release. E is at most the release count times this.",
        ),
        row(
            "interval.downloads_per_month_double",
            i.downloads_per_month_double,
            "Monthly downloads above this double E.",
        ),
        row(
            "interval.active_users_double",
            i.active_users_double,
            "Active users above this double E.",
        ),
        row(
            "dormancy.min_days",
            t.dormancy.min_days,
            "The dormancy floor: smallest E in days, and E for every app.",
        ),
    ]
}

/// The rot signal limits.
pub(super) fn rot_rows(t: &Thresholds) -> Vec<ThresholdRow> {
    let r = &t.rot;
    vec![
        row(
            "rot.unanswered_days",
            r.unanswered_days,
            "R1 issue limit: days an outside issue stays unanswered.",
        ),
        row(
            "rot.maintainer_silence_days",
            r.maintainer_silence_days,
            "R1 reply window: days over which maintainer replies count.",
        ),
        row(
            "rot.advisory_grace_days",
            r.advisory_grace_days,
            "R2 grace window: days an advisory stays unpatched.",
        ),
        row(
            "rot.lost_glory_below",
            r.lost_glory_below,
            "R3 lost-glory limit: a former glory below this share is rot.",
        ),
        row(
            "rot.build_check_max_age_days",
            r.build_check_max_age_days,
            "R6 age limit: days a sandbox build check stays valid.",
        ),
    ]
}

/// The dead rule counts.
pub(super) fn dead_rows(t: &Thresholds) -> Vec<ThresholdRow> {
    let d = &t.dormancy;
    vec![
        row(
            "dormancy.dead_rot_signals",
            d.dead_rot_signals,
            "Fewest rot signals that make a dormant entry dead.",
        ),
        row(
            "dormancy.dead_consecutive_checks",
            d.dead_consecutive_checks,
            "Dead checks in a row that archive an entry.",
        ),
    ]
}

/// The adoption that keeps a dormant entry finished.
pub(super) fn finished_rows(t: &Thresholds) -> Vec<ThresholdRow> {
    let f = &t.finished;
    vec![
        row(
            "finished.min_former_glory",
            f.min_former_glory,
            "Lowest former glory of a finished library.",
        ),
        row(
            "finished.min_new_adopters",
            f.min_new_adopters,
            "Fewest new adopters of a finished library.",
        ),
        row(
            "finished.min_binary_downloads_per_release",
            f.min_binary_downloads_per_release,
            "Fewest binary downloads per release of a finished app.",
        ),
    ]
}

/// The New gates.
pub(super) fn new_rows(t: &Thresholds) -> Vec<ThresholdRow> {
    let n = &t.new;
    vec![
        row(
            "new.max_age_days",
            n.max_age_days,
            "Age limit: a first release fewer days ago than this is New.",
        ),
        row(
            "new.min_active_users",
            n.min_active_users,
            "User floor: a library with fewer active users than this is New.",
        ),
    ]
}

/// The Featured library gates.
pub(super) fn library_rows(t: &Thresholds) -> Vec<ThresholdRow> {
    let l = &t.library_featured;
    vec![
        row(
            "library_featured.min_active_users",
            l.min_active_users,
            "Fewest active users. This gate or the new-adopter gate passes.",
        ),
        row(
            "library_featured.min_new_adopters",
            l.min_new_adopters,
            "Fewest new adopters over 12 months. This gate or the active-user gate passes.",
        ),
        row(
            "library_featured.min_former_glory",
            l.min_former_glory,
            "Lowest former glory, a share from 0 to 1.",
        ),
    ]
}

/// The Featured app use signals.
pub(super) fn app_rows(t: &Thresholds) -> Vec<ThresholdRow> {
    let a = &t.app_featured;
    vec![
        row(
            "app_featured.min_signals",
            a.min_signals,
            "How many of the four use signals must pass.",
        ),
        row(
            "app_featured.binary_downloads",
            a.binary_downloads,
            "Fewest binary downloads over the last three releases.",
        ),
        row(
            "app_featured.homebrew_on_request_365d",
            a.homebrew_on_request_365d,
            "Fewest Homebrew installs on request over 365 days.",
        ),
        row(
            "app_featured.repology_families",
            a.repology_families,
            "Fewest Repology repository families that package the app.",
        ),
        row(
            "app_featured.docker_pull_growth_90d",
            a.docker_pull_growth_90d,
            "Smallest Docker pull growth over 90 days.",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::thresholds;
    use super::*;

    /// Every row set, in the order the rules read them.
    fn all_rows(t: &Thresholds) -> Vec<ThresholdRow> {
        [
            interval_rows,
            rot_rows,
            dead_rows,
            finished_rows,
            new_rows,
            library_rows,
            app_rows,
        ]
        .into_iter()
        .flat_map(|make| make(t))
        .collect()
    }

    #[test]
    fn rows_show_the_values() {
        let t = thresholds(None);
        let all = all_rows(&t);
        let value = |key: &str| {
            all.iter()
                .find(|row| row.key == key)
                .map(|row| row.value.clone())
        };
        assert_eq!(
            value("library_featured.min_active_users").as_deref(),
            Some("11")
        );
        assert_eq!(value("rot.lost_glory_below").as_deref(), Some("0.5"));
        assert_eq!(
            value("app_featured.docker_pull_growth_90d").as_deref(),
            Some("10000")
        );
    }
}
