//! Dates and amounts as a person reads them.

use catalogue_core::Date;

/// The month names, January first.
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The ISO date `iso` as `4 Nov 2026`. `None` when `iso` is no `YYYY-MM-DD` date.
pub(crate) fn date_text(iso: &str) -> Option<String> {
    let mut parts = iso.split('-');
    let year = parts.next().filter(|year| year.len() == 4)?;
    let month: usize = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !year.bytes().all(|b| b.is_ascii_digit()) || day == 0 || day > 31 {
        return None;
    }
    let name = MONTHS.get(month.checked_sub(1)?)?;
    Some(format!("{day} {name} {year}"))
}

/// The ISO date `iso` as a [`Date`]. `None` when `iso` is no real `YYYY-MM-DD` date.
pub(crate) fn parse_date(iso: &str) -> Option<Date> {
    let mut parts = iso.split('-');
    let year = parts.next().filter(|year| year.len() == 4)?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Date::new(year, month, day).ok()
}

/// How long before `checked` the ISO date `iso` lies, in words. `None` for no date or a later one.
///
/// A page states these words beside the check date, because the site has no clock.
pub(crate) fn ago_text(iso: &str, checked: Date) -> Option<String> {
    let days = checked.days_since(parse_date(iso)?);
    let text = match days {
        ..0 => return None,
        0 => "On the check date".to_owned(),
        1 => "1 day ago".to_owned(),
        2..=59 => format!("{days} days ago"),
        60..=729 => format!("{} months ago", days / 30),
        _ => format!("{} years ago", days / 365),
    };
    Some(text)
}

/// The whole number `digits` with thousands marked: `1,840,052`. Other text stays as it is.
pub(crate) fn grouped(digits: &str) -> String {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return digits.to_owned();
    }
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_before_the_check_reads_in_words() {
        let checked = Date::new(2026, 11, 4).expect("date");
        for (iso, words) in [
            ("2026-11-04", Some("On the check date")),
            ("2026-11-03", Some("1 day ago")),
            ("2026-10-31", Some("4 days ago")),
            ("2026-09-06", Some("59 days ago")),
            ("2026-09-05", Some("2 months ago")),
            ("2024-11-05", Some("24 months ago")),
            ("2024-11-04", Some("2 years ago")),
            ("2026-11-05", None),
            ("soon", None),
        ] {
            assert_eq!(ago_text(iso, checked).as_deref(), words, "{iso}");
        }
    }

    #[test]
    fn thousands_are_marked_in_whole_numbers_only() {
        assert_eq!(grouped("1840052"), "1,840,052");
        assert_eq!(grouped("100"), "100");
        assert_eq!(grouped("1000"), "1,000");
        assert_eq!(grouped("Not measured"), "Not measured");
    }

    #[test]
    fn an_iso_date_reads_with_the_month_name() {
        assert_eq!(date_text("2026-11-04").as_deref(), Some("4 Nov 2026"));
        assert_eq!(date_text("2025-01-31").as_deref(), Some("31 Jan 2025"));
    }

    #[test]
    fn other_text_gives_no_date() {
        for text in [
            "",
            "soon",
            "2026-13-01",
            "2026-00-01",
            "2026-11-00",
            "26-11-04",
            "2026-11-04-1",
        ] {
            assert_eq!(date_text(text), None, "{text}");
        }
    }
}
