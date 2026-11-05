use serde::{Deserialize, Serialize};

/// The most days of push history the `active_days` limit accepts: ten years.
const ACTIVE_DAYS_MAX: u32 = 3650;

/// The `[below_floor]` table of `data/discovery.toml`.
///
/// The limits of `catalogue discover below-floor`, a review list that no tier reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BelowFloor {
    /// The fewest stars a repo has to be listed. Stars only sort a list for a person.
    pub min_stars: u32,
    /// The most days since the last push of a listed repo.
    pub active_days: u32,
}

impl BelowFloor {
    /// Lists every rule this table breaks.
    pub(super) fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.min_stars == 0 {
            out.push("below_floor.min_stars must be at least 1".to_owned());
        }
        if self.active_days == 0 || self.active_days > ACTIVE_DAYS_MAX {
            out.push(format!(
                "below_floor.active_days must be from 1 to {ACTIVE_DAYS_MAX}, found {}",
                self.active_days
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(min_stars: u32, active_days: u32) -> BelowFloor {
        BelowFloor {
            min_stars,
            active_days,
        }
    }

    #[test]
    fn the_limits_at_each_edge_pass() {
        assert!(table(1, 1).problems().is_empty());
        assert!(table(100, 730).problems().is_empty());
        assert!(table(u32::MAX, ACTIVE_DAYS_MAX).problems().is_empty());
    }

    #[test]
    fn zero_stars_are_reported() {
        let found = table(0, 730).problems();
        assert_eq!(found, ["below_floor.min_stars must be at least 1"]);
    }

    #[test]
    fn days_outside_the_range_are_reported() {
        for days in [0, ACTIVE_DAYS_MAX + 1] {
            let found = table(100, days).problems();
            assert_eq!(found.len(), 1, "{days}");
            assert!(found[0].contains("active_days"), "{found:?}");
            assert!(found[0].contains(&days.to_string()), "{found:?}");
        }
    }

    #[test]
    fn a_missing_field_and_an_unknown_field_fail_to_parse() {
        assert!(toml::from_str::<BelowFloor>("min_stars = 100").is_err());
        assert!(toml::from_str::<BelowFloor>("min_stars = 1\nactive_days = 1\nx = 1").is_err());
        assert!(toml::from_str::<BelowFloor>("min_stars = -1\nactive_days = 1").is_err());
    }
}
