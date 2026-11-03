//! Calendar helpers shared by the usage rules.

use jiff::{Span, civil};

/// `date` moved by `days`, clamped to the supported calendar range.
pub(super) fn add_days(date: civil::Date, days: i64) -> civil::Date {
    let clamp = if days < 0 {
        civil::Date::MIN
    } else {
        civil::Date::MAX
    };
    Span::new()
        .try_days(days)
        .and_then(|span| date.checked_add(span))
        .unwrap_or(clamp)
}

/// The first day of the month after the month of `date`, or `None` past the calendar end.
pub(super) fn next_month(date: civil::Date) -> Option<civil::Date> {
    Span::new()
        .try_months(1)
        .and_then(|span| date.first_of_month().checked_add(span))
        .ok()
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;

    #[test]
    fn add_days_moves_and_clamps() {
        assert_eq!(add_days(date(2026, 2, 27), 2), date(2026, 3, 1));
        assert_eq!(add_days(date(2026, 1, 1), -365), date(2025, 1, 1));
        assert_eq!(add_days(civil::Date::MAX, 1), civil::Date::MAX);
        assert_eq!(add_days(civil::Date::MIN, -1), civil::Date::MIN);
    }

    #[test]
    fn next_month_starts_the_following_month() {
        assert_eq!(next_month(date(2026, 1, 31)), Some(date(2026, 2, 1)));
        assert_eq!(next_month(date(2026, 12, 5)), Some(date(2027, 1, 1)));
        assert_eq!(next_month(civil::Date::MAX), None);
    }
}
