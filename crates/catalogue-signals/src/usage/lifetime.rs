//! How long a depender's latest release keeps counting as a use.

/// Days a `0.0.x` or `0.1.0` latest release counts.
pub(super) const FIRST_DAYS: i64 = 60;
/// Extra days a `0.0.x` or `0.1.0` latest release counts when the crate has more than one owner.
pub(super) const FIRST_MULTI_OWNER_DAYS: i64 = 14;
/// Days a `0.x` latest release counts when the crate has fewer than [`FEW_RELEASES`] releases.
pub(super) const YOUNG_DAYS: i64 = 180;
/// Extra [`YOUNG_DAYS`] days when the crate has more than one owner.
pub(super) const YOUNG_MULTI_OWNER_DAYS: i64 = 30;
/// Days a `0.x` latest release, or a crate with fewer than [`FEW_RELEASES`] releases, counts.
pub(super) const UNSTABLE_DAYS: i64 = 365;
/// Extra [`UNSTABLE_DAYS`] days when the crate has more than one owner.
pub(super) const UNSTABLE_MULTI_OWNER_DAYS: i64 = 90;
/// Days a stable latest release of a crate with [`FEW_RELEASES`] or more releases counts.
pub(super) const STABLE_DAYS: i64 = 730;
/// Extra [`STABLE_DAYS`] days when the crate has more than one owner.
pub(super) const STABLE_MULTI_OWNER_DAYS: i64 = 365;
/// A crate with fewer unyanked releases than this counts as young.
pub(super) const FEW_RELEASES: usize = 3;

/// The days after its date that a crate's latest release keeps counting.
///
/// `latest` is the latest unyanked version number, `releases` the number of
/// unyanked releases, and `multi_owner` whether the crate has more than one owner.
pub(super) fn lifetime_days(latest: &str, releases: usize, multi_owner: bool) -> i64 {
    let leading = Leading::parse(latest);
    let (days, extra) = if leading.is_first() {
        (FIRST_DAYS, FIRST_MULTI_OWNER_DAYS)
    } else if leading.zero_major && releases < FEW_RELEASES {
        (YOUNG_DAYS, YOUNG_MULTI_OWNER_DAYS)
    } else if leading.zero_major || releases < FEW_RELEASES {
        (UNSTABLE_DAYS, UNSTABLE_MULTI_OWNER_DAYS)
    } else {
        (STABLE_DAYS, STABLE_MULTI_OWNER_DAYS)
    };
    if multi_owner { days + extra } else { days }
}

/// The leading numeric parts of a version number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Leading {
    /// The number starts with `0.`.
    zero_major: bool,
    /// The number starts with `0.0.`.
    zero_minor: bool,
    /// The number is exactly `0.1.0`.
    first_minor: bool,
}

impl Leading {
    /// Reads the major and minor parts. A non-numeric major part reads as stable.
    fn parse(num: &str) -> Self {
        let mut parts = num.split('.');
        let major = parts.next().and_then(numeric);
        let minor = parts.next();
        let zero_major = major == Some(0) && minor.is_some();
        let zero_minor = zero_major && minor.and_then(numeric) == Some(0) && parts.next().is_some();
        Self {
            zero_major,
            zero_minor,
            first_minor: num == "0.1.0",
        }
    }

    /// The number is `0.0.x` or exactly `0.1.0`.
    fn is_first(self) -> bool {
        self.zero_minor || self.first_minor
    }
}

/// The value of a part made only of ASCII digits.
fn numeric(part: &str) -> Option<u64> {
    if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    part.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_releases_count_60_days() {
        for latest in ["0.0.1", "0.0.0", "0.0.7-alpha", "0.1.0"] {
            assert_eq!(lifetime_days(latest, 9, false), 60, "{latest}");
            assert_eq!(lifetime_days(latest, 9, true), 74, "{latest}");
        }
    }

    #[test]
    fn young_zero_major_counts_180_days() {
        for latest in ["0.1.1", "0.2.0", "0.1.0-rc.1", "0.3.0+build"] {
            assert_eq!(lifetime_days(latest, 2, false), 180, "{latest}");
            assert_eq!(lifetime_days(latest, 2, true), 210, "{latest}");
        }
    }

    #[test]
    fn zero_major_or_few_releases_counts_365_days() {
        assert_eq!(lifetime_days("0.5.0", 3, false), 365);
        assert_eq!(lifetime_days("0.5.0", 3, true), 455);
        assert_eq!(lifetime_days("1.0.0", 2, false), 365);
        assert_eq!(lifetime_days("1.0.0", 2, true), 455);
    }

    #[test]
    fn stable_counts_730_days() {
        assert_eq!(lifetime_days("1.2.3", 3, false), 730);
        assert_eq!(lifetime_days("1.2.3", 3, true), 1095);
        assert_eq!(lifetime_days("10.0.0", 40, false), 730);
    }

    #[test]
    fn non_numeric_major_reads_as_stable() {
        assert_eq!(lifetime_days("x.1.0", 5, false), 730);
        assert_eq!(lifetime_days("", 5, false), 730);
        assert_eq!(lifetime_days("0", 5, false), 730, "no `0.` prefix");
        assert_eq!(lifetime_days("00x.1", 5, false), 730);
    }

    #[test]
    fn leading_parts_are_read_by_hand() {
        let parsed = Leading::parse("0.0.3");
        assert!(parsed.zero_major && parsed.zero_minor && !parsed.first_minor);
        let parsed = Leading::parse("0.0");
        assert!(parsed.zero_major && !parsed.zero_minor, "no patch part");
        assert!(Leading::parse("0.1.0").is_first());
        assert!(!Leading::parse("0.1.0-alpha").is_first());
    }
}
