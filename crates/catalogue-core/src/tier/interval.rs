use super::signals::Signals;
use super::verdict::{Reason, Rule};
use crate::amount_text::rounded;
use crate::registry::Thresholds;

/// The expected update interval E with the signals it lacked.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Interval {
    /// E in days.
    pub days: i64,
    pub reason: Reason,
    pub needs: Vec<&'static str>,
}

/// `major.minor.patch` of a version number.
///
/// Prerelease and build suffixes are dropped. Missing parts read as 0.
/// A part that is not a number makes the whole version read as `1.0.0`.
fn parse_version(text: &str) -> (u64, u64, u64) {
    const UNPARSED: (u64, u64, u64) = (1, 0, 0);
    let core = text.trim().split(['-', '+']).next().unwrap_or_default();
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() > 3 {
        return UNPARSED;
    }
    let mut numbers = [0_u64; 3];
    for (slot, part) in numbers.iter_mut().zip(&parts) {
        match part.parse() {
            Ok(number) => *slot = number,
            Err(_) => return UNPARSED,
        }
    }
    let [major, minor, patch] = numbers;
    (major, minor, patch)
}

/// Rule 2: the expected update interval E.
///
/// A library with a known latest version starts from the first matching base:
/// patch > 3 and major >= 1, patch > 3, patch > 0, major >= 1, minor > 3, else other.
/// E is at most `release_count * days_per_release_cap`. E doubles when downloads
/// per month exceed `downloads_per_month_double`, and again when active dependents exceed
/// `active_users_double`. E is never below `dormancy.min_days`. An app, or a library
/// without a known latest version, uses `dormancy.min_days`. An unknown cap or
/// doubling input leaves that step out and is named in `needs`.
pub(super) fn expected(signals: &Signals, thresholds: &Thresholds) -> Interval {
    let floor = i64::from(thresholds.dormancy.min_days);
    if !signals.is_library() {
        return Interval {
            days: floor,
            reason: Reason::new(Rule::ExpectedInterval, format!("app: E = {floor} days")),
            needs: Vec::new(),
        };
    }
    let Some(version) = signals.latest_version.as_deref() else {
        return Interval {
            days: floor,
            reason: Reason::new(
                Rule::ExpectedInterval,
                format!("latest version unknown: E = {floor} days"),
            ),
            needs: vec!["latest_version"],
        };
    };
    let t = &thresholds.interval;
    let (major, minor, patch) = parse_version(version);
    let base = if patch > 3 && major >= 1 {
        t.patch_gt3_major_ge1
    } else if patch > 3 {
        t.patch_gt3
    } else if patch > 0 {
        t.patch_gt0
    } else if major >= 1 {
        t.major_ge1
    } else if minor > 3 {
        t.minor_gt3
    } else {
        t.other
    };
    let mut days = i64::from(base);
    let mut steps = vec![format!("version {version} base {base}")];
    let mut needs = Vec::new();
    match signals.release_count {
        Some(count) => {
            let cap = i64::from(count).saturating_mul(i64::from(t.days_per_release_cap));
            if cap < days {
                days = cap;
                steps.push(format!("capped at {count} releases = {cap}"));
            }
        }
        None => needs.push("release_count"),
    }
    match signals.downloads_per_month {
        Some(downloads) if downloads > t.downloads_per_month_double => {
            days = days.saturating_mul(2);
            steps.push(format!("doubled for {downloads} downloads per month"));
        }
        Some(_) => {}
        None => needs.push("downloads_per_month"),
    }
    match signals.active_users {
        Some(users) if users > t.active_users_double => {
            days = days.saturating_mul(2);
            steps.push(format!("doubled for {} active users", rounded(users)));
        }
        Some(_) => {}
        None => needs.push("active_users"),
    }
    if days < floor {
        days = floor;
        steps.push(format!("raised to the {floor}-day floor"));
    }
    Interval {
        days,
        reason: Reason::new(
            Rule::ExpectedInterval,
            format!("{}: E = {days} days", steps.join(", ")),
        ),
        needs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::EntryKind;
    use crate::tier::fixtures::{featured_app, featured_library, thresholds};

    /// E for a library with `version`, many releases, and no doubling.
    fn base(version: &str) -> i64 {
        let signals = Signals {
            latest_version: Some(version.to_owned()),
            release_count: Some(1_000),
            downloads_per_month: Some(0),
            active_users: Some(0.0),
            ..featured_library()
        };
        let mut t = thresholds();
        t.dormancy.min_days = 0;
        expected(&signals, &t).days
    }

    #[test]
    fn versions_parse_by_hand() {
        assert_eq!(parse_version("1.2.3"), (1, 2, 3));
        assert_eq!(parse_version("0.4"), (0, 4, 0));
        assert_eq!(parse_version("2"), (2, 0, 0));
        assert_eq!(parse_version("1.0.0-beta.2+build"), (1, 0, 0));
        assert_eq!(parse_version("0.0.7-rc1"), (0, 0, 7));
        for bad in ["", "x", "1.x.3", "1.2.3.4", "v1.2.3", "1..2"] {
            assert_eq!(parse_version(bad), (1, 0, 0), "{bad}");
        }
    }

    #[test]
    fn base_follows_the_first_matching_version_rule() {
        assert_eq!(base("1.0.4"), 700);
        assert_eq!(base("0.2.4"), 450);
        assert_eq!(base("1.0.3"), 300);
        assert_eq!(base("0.0.1"), 300);
        assert_eq!(base("1.9.0"), 200);
        assert_eq!(base("0.4.0"), 140);
        assert_eq!(base("0.3.0"), 80);
        assert_eq!(base("weird"), 200);
    }

    #[test]
    fn release_count_caps_the_base() {
        let mut t = thresholds();
        t.dormancy.min_days = 0;
        let signals = Signals {
            latest_version: Some("1.0.4".to_owned()),
            release_count: Some(3),
            downloads_per_month: Some(0),
            active_users: Some(0.0),
            ..featured_library()
        };
        let interval = expected(&signals, &t);
        assert_eq!(interval.days, 150);
        assert!(
            interval
                .reason
                .detail
                .contains("capped at 3 releases = 150")
        );
    }

    #[test]
    fn doubling_needs_values_strictly_above_the_limits() {
        let mut t = thresholds();
        t.dormancy.min_days = 0;
        let at_limits = Signals {
            latest_version: Some("1.9.0".to_owned()),
            release_count: Some(100),
            downloads_per_month: Some(50_000),
            active_users: Some(10.0),
            ..featured_library()
        };
        assert_eq!(expected(&at_limits, &t).days, 200);
        let above = Signals {
            downloads_per_month: Some(50_001),
            active_users: Some(10.5),
            ..at_limits
        };
        assert_eq!(expected(&above, &t).days, 800);
    }

    #[test]
    fn floor_applies_after_doubling() {
        let interval = expected(&featured_library(), &thresholds());
        assert_eq!(interval.days, 2_800);
        assert!(interval.needs.is_empty());
        let small = Signals {
            latest_version: Some("0.1.0".to_owned()),
            downloads_per_month: Some(0),
            active_users: Some(0.0),
            ..featured_library()
        };
        let interval = expected(&small, &thresholds());
        assert_eq!(interval.days, 365);
        assert!(interval.reason.detail.contains("365-day floor"));
    }

    #[test]
    fn unknown_inputs_skip_their_step_and_are_named() {
        let signals = Signals {
            latest_version: Some("1.0.4".to_owned()),
            release_count: None,
            downloads_per_month: None,
            active_users: None,
            ..featured_library()
        };
        let interval = expected(&signals, &thresholds());
        assert_eq!(interval.days, 700);
        assert_eq!(
            interval.needs,
            ["release_count", "downloads_per_month", "active_users"]
        );
        let no_version = Signals {
            latest_version: None,
            ..featured_library()
        };
        let interval = expected(&no_version, &thresholds());
        assert_eq!(interval.days, 365);
        assert_eq!(interval.needs, ["latest_version"]);
    }

    #[test]
    fn apps_use_the_floor() {
        let interval = expected(&featured_app(), &thresholds());
        assert_eq!(interval.days, 365);
        assert!(interval.needs.is_empty());
        let unknown = Signals::unknown(EntryKind::App);
        assert_eq!(expected(&unknown, &thresholds()).days, 365);
    }
}
