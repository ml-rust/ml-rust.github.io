use serde::{Deserialize, Serialize};

use crate::date::Date;
use crate::tier::{APP_SIGNALS, ROT_SIGNALS};

/// Contents of `data/thresholds.toml`.
///
/// Every number the tier rules compare a signal against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thresholds {
    /// Version of this set of values. A change to any value raises it.
    pub version: u32,
    /// Date of the calibration against real data. `None` before calibration.
    pub calibrated: Option<Date>,
    /// Gates for the `new` tier.
    pub new: NewThresholds,
    /// Gates for a featured library.
    pub library_featured: LibraryFeaturedThresholds,
    /// Gates for a featured app.
    pub app_featured: AppFeaturedThresholds,
    /// Dormancy floor and the dead rule.
    pub dormancy: DormancyThresholds,
    /// The expected update interval by latest version.
    pub interval: IntervalThresholds,
    /// Rot signal limits.
    pub rot: RotThresholds,
    /// Adoption that keeps a dormant entry `stable`.
    pub finished: FinishedThresholds,
    /// How long a value stands in for a failed fetch.
    pub fallback: FallbackThresholds,
}

/// Gates for the `new` tier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewThresholds {
    /// A first release fewer days ago than this is new.
    pub max_age_days: u32,
    /// A library with fewer active users than this is new.
    pub min_active_users: f64,
}

/// Gates for a featured library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryFeaturedThresholds {
    /// Fewest active users. Either this or `min_new_adopters` passes.
    pub min_active_users: f64,
    /// Fewest new adopters. Either this or `min_active_users` passes.
    pub min_new_adopters: f64,
    /// Lowest `former_glory`, a share in `[0, 1]`.
    pub min_former_glory: f64,
}

/// Gates for a featured app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppFeaturedThresholds {
    /// How many of the app use signals pass.
    pub min_signals: u32,
    /// Fewest binary downloads over the last three releases.
    pub binary_downloads: u64,
    /// Fewest Homebrew installs on request over 365 days.
    pub homebrew_on_request_365d: u64,
    /// Fewest Repology repository families that package the app.
    pub repology_families: u32,
    /// Smallest Docker pull growth over 90 days.
    pub docker_pull_growth_90d: u64,
}

/// Dormancy floor and the dead rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DormancyThresholds {
    /// Smallest expected interval in days, and the interval for every app.
    pub min_days: u32,
    /// Fewest rot signals on a dormant entry that make it dead.
    pub dead_rot_signals: u32,
    /// Consecutive dead checks that archive an entry.
    pub dead_consecutive_checks: u32,
}

/// Expected update interval in days, chosen by the latest version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntervalThresholds {
    /// Patch above 3 and major at least 1.
    pub patch_gt3_major_ge1: u32,
    /// Patch above 3.
    pub patch_gt3: u32,
    /// Patch above 0.
    pub patch_gt0: u32,
    /// Major at least 1.
    pub major_ge1: u32,
    /// Minor above 3.
    pub minor_gt3: u32,
    /// Every other version.
    pub other: u32,
    /// Days per release. The interval is at most release count times this.
    pub days_per_release_cap: u32,
    /// Downloads per month above this double the interval.
    pub downloads_per_month_double: u64,
    /// Active users above this double the interval.
    pub active_users_double: f64,
}

/// Rot signal limits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RotThresholds {
    /// Age in days of an unanswered outside issue that counts as rot.
    pub unanswered_days: u32,
    /// Window in days over which maintainer replies are counted.
    pub maintainer_silence_days: u32,
    /// Days an advisory stays unpatched before it counts as rot.
    pub advisory_grace_days: u32,
    /// A `former_glory` below this share counts as rot.
    pub lost_glory_below: f64,
    /// Days a sandbox build check stays valid. An older check reads as unknown.
    pub build_check_max_age_days: u32,
}

/// Adoption that keeps a dormant entry `stable`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinishedThresholds {
    /// Lowest `former_glory` of a finished library, a share in `[0, 1]`.
    pub min_former_glory: f64,
    /// Fewest new adopters of a finished library.
    pub min_new_adopters: f64,
    /// Fewest binary downloads per release of a finished app.
    pub min_binary_downloads_per_release: u64,
}

/// How long a value stands in for a failed fetch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FallbackThresholds {
    /// Most days between a value's fetch and the check that reuses it. An older
    /// value reads as unknown.
    pub max_age_days: u32,
}

impl Thresholds {
    /// Lists every rule this file breaks.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (name, value) in [
            ("new.min_active_users", self.new.min_active_users),
            (
                "library_featured.min_active_users",
                self.library_featured.min_active_users,
            ),
            (
                "library_featured.min_new_adopters",
                self.library_featured.min_new_adopters,
            ),
            (
                "interval.active_users_double",
                self.interval.active_users_double,
            ),
            ("finished.min_new_adopters", self.finished.min_new_adopters),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                out.push(format!("{name} = {value} must be a finite number >= 0"));
            }
        }
        for (name, value) in [
            (
                "library_featured.min_former_glory",
                self.library_featured.min_former_glory,
            ),
            ("rot.lost_glory_below", self.rot.lost_glory_below),
            ("finished.min_former_glory", self.finished.min_former_glory),
        ] {
            if !(0.0..=1.0).contains(&value) {
                out.push(format!("{name} = {value} must be a share in [0, 1]"));
            }
        }
        in_range(
            "dormancy.dead_rot_signals",
            self.dormancy.dead_rot_signals,
            ROT_SIGNALS,
            &mut out,
        );
        if self.dormancy.dead_consecutive_checks == 0 {
            out.push("dormancy.dead_consecutive_checks = 0 must be at least 1".to_owned());
        }
        if self.rot.build_check_max_age_days == 0 {
            out.push("rot.build_check_max_age_days = 0 must be at least 1".to_owned());
        }
        if self.fallback.max_age_days == 0 {
            out.push("fallback.max_age_days = 0 must be at least 1".to_owned());
        }
        in_range(
            "app_featured.min_signals",
            self.app_featured.min_signals,
            APP_SIGNALS,
            &mut out,
        );
        out
    }
}

/// Reports `value` outside `1..=max`. `max` is the number of signals the rule counts.
fn in_range(name: &str, value: u32, max: u32, out: &mut Vec<String>) {
    if !(1..=max).contains(&value) {
        out.push(format!("{name} = {value} must be between 1 and {max}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::THRESHOLDS;

    fn parse(text: &str) -> Thresholds {
        toml::from_str(text).expect("parses")
    }

    fn problems_with(from: &str, to: &str) -> Vec<String> {
        assert!(THRESHOLDS.contains(from), "fixture lacks `{from}`");
        parse(&THRESHOLDS.replace(from, to)).problems()
    }

    fn has(problems: &[String], needle: &str) -> bool {
        problems.iter().any(|p| p.contains(needle))
    }

    #[test]
    fn starting_values_parse_and_pass() {
        let t = parse(THRESHOLDS);
        assert!(t.problems().is_empty(), "{:?}", t.problems());
        assert_eq!(t.version, 1);
        assert!(t.calibrated.is_none());
        assert_eq!(t.interval.patch_gt3_major_ge1, 700);
        assert_eq!(t.app_featured.docker_pull_growth_90d, 10_000);
        assert_eq!(t.rot.build_check_max_age_days, 45);
        assert_eq!(t.fallback.max_age_days, 14);
    }

    #[test]
    fn the_committed_file_parses_and_passes() {
        let t = parse(include_str!("../../../../data/thresholds.toml"));
        assert!(t.problems().is_empty(), "{:?}", t.problems());
        assert_eq!(t.version, 4);
        assert_eq!(t.rot.build_check_max_age_days, 45);
        assert_eq!(t.fallback.max_age_days, 14);
    }

    #[test]
    fn build_check_age_must_be_at_least_one_day() {
        let found = problems_with(
            "build_check_max_age_days = 45",
            "build_check_max_age_days = 0",
        );
        assert!(has(
            &found,
            "rot.build_check_max_age_days = 0 must be at least 1"
        ));
        let text = THRESHOLDS.replace(
            "build_check_max_age_days = 45",
            "build_check_max_age_days = -1",
        );
        assert!(toml::from_str::<Thresholds>(&text).is_err());
    }

    #[test]
    fn fallback_age_must_be_at_least_one_day() {
        let found = problems_with(
            "[fallback]\nmax_age_days = 14",
            "[fallback]\nmax_age_days = 0",
        );
        assert!(has(&found, "fallback.max_age_days = 0 must be at least 1"));
        let text = THRESHOLDS.replace(
            "[fallback]\nmax_age_days = 14",
            "[fallback]\nmax_age_days = -1",
        );
        assert!(toml::from_str::<Thresholds>(&text).is_err());
        let absent = THRESHOLDS.replace("[fallback]\nmax_age_days = 14\n", "");
        assert!(toml::from_str::<Thresholds>(&absent).is_err());
    }

    #[test]
    fn calibration_date_reads_as_a_date() {
        let t = parse(&format!("calibrated = 2027-01-15\n{THRESHOLDS}"));
        assert_eq!(t.calibrated, Some(Date::new(2027, 1, 15).expect("date")));
    }

    #[test]
    fn negative_and_non_finite_counts_are_reported() {
        let found = problems_with("min_new_adopters = 1.0", "min_new_adopters = -1.0");
        assert!(has(
            &found,
            "finished.min_new_adopters = -1 must be a finite number"
        ));
        let found = problems_with("active_users_double = 10.0", "active_users_double = nan");
        assert!(has(&found, "interval.active_users_double = NaN"));
        let found = problems_with("min_active_users = 2.0", "min_active_users = inf");
        assert!(has(&found, "new.min_active_users = inf"));
    }

    #[test]
    fn negative_integer_counts_fail_to_parse() {
        let text = THRESHOLDS.replace("max_age_days = 180", "max_age_days = -1");
        assert!(toml::from_str::<Thresholds>(&text).is_err());
    }

    #[test]
    fn shares_outside_zero_to_one_are_reported() {
        let found = problems_with("lost_glory_below = 0.5", "lost_glory_below = 1.5");
        assert!(has(&found, "rot.lost_glory_below = 1.5 must be a share"));
        let found = problems_with("min_former_glory = 0.8", "min_former_glory = -0.1");
        assert!(has(
            &found,
            "finished.min_former_glory = -0.1 must be a share"
        ));
        assert!(problems_with("lost_glory_below = 0.5", "lost_glory_below = 1.0").is_empty());
        assert!(problems_with("lost_glory_below = 0.5", "lost_glory_below = 0.0").is_empty());
    }

    #[test]
    fn dead_rule_counts_must_be_at_least_one() {
        let found = problems_with("dead_rot_signals = 2", "dead_rot_signals = 0");
        assert!(has(
            &found,
            "dormancy.dead_rot_signals = 0 must be between 1"
        ));
        let found = problems_with("dead_consecutive_checks = 2", "dead_consecutive_checks = 0");
        assert!(has(&found, "dormancy.dead_consecutive_checks = 0"));
    }

    #[test]
    fn signal_counts_cannot_exceed_the_signals_counted() {
        assert!(problems_with("dead_rot_signals = 2", "dead_rot_signals = 6").is_empty());
        let found = problems_with("dead_rot_signals = 2", "dead_rot_signals = 7");
        assert!(has(&found, "between 1 and 6"));
        let found = problems_with("min_signals = 2", "min_signals = 5");
        assert!(has(
            &found,
            "app_featured.min_signals = 5 must be between 1 and 4"
        ));
        let found = problems_with("min_signals = 2", "min_signals = 0");
        assert!(has(&found, "app_featured.min_signals = 0"));
    }

    #[test]
    fn missing_and_unknown_fields_fail_to_parse() {
        let missing = THRESHOLDS.replace("other = 80\n", "");
        assert!(toml::from_str::<Thresholds>(&missing).is_err());
        let extra = THRESHOLDS.replace("other = 80\n", "other = 80\nbogus = 1\n");
        assert!(toml::from_str::<Thresholds>(&extra).is_err());
        assert!(toml::from_str::<Thresholds>(&format!("bogus = 1\n{THRESHOLDS}")).is_err());
    }
}
