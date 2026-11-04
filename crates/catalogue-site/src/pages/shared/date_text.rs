//! Dates as a person reads them.

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

#[cfg(test)]
mod tests {
    use super::*;

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
