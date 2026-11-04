//! A weighted signal amount as text for people.
//!
//! Active dependents and new dependents are sums of fractional weights, so a value
//! such as 3 can arrive as `2.9999999999999996`. These functions change only
//! the text. Rules compare the raw value.

/// `value` rounded to two decimal places, with no trailing zeros. For reason text.
pub fn rounded(value: f64) -> String {
    trimmed(format!("{value:.2}"))
}

/// `value` with float noise removed: four decimal places, no trailing zeros.
/// For a published signal value, which keeps its real precision.
pub fn precise(value: f64) -> String {
    trimmed(format!("{value:.4}"))
}

/// `text`, a fixed-point number, with no trailing zeros.
fn trimmed(mut text: String) -> String {
    let len = text.trim_end_matches('0').trim_end_matches('.').len();
    text.truncate(len);
    if text == "-0" {
        text.clear();
        text.push('0');
    }
    text
}

/// `value` as text for a statement that it is below `limit`.
///
/// Rounding can lift a value to the limit and print `11 < 11`. Such a value
/// rounds down instead, so the statement stays true: `10.99 < 11`.
pub fn rounded_below(value: f64, limit: f64) -> String {
    let text = rounded(value);
    match text.parse::<f64>() {
        Ok(shown) if shown >= limit => rounded((value * 100.0).floor() / 100.0),
        _ => text,
    }
}

/// The published amount `text` as a whole number, for a list a person scans.
///
/// `None` when `text` is no finite amount.
pub fn whole(text: &str) -> Option<String> {
    let value = text.parse::<f64>().ok().filter(|value| value.is_finite())?;
    let rounded = value.round();
    Some(if rounded == 0.0 {
        "0".to_owned()
    } else {
        format!("{rounded:.0}")
    })
}

/// The units [`compact`] prints, smallest first.
const COMPACT_UNITS: [(f64, &str); 3] = [(1e3, "k"), (1e6, "M"), (1e9, "B")];

/// The published count `text` in a short form for a list: `987`, `1.5k`, `24k`, `1.8M`.
///
/// Rounds to the nearest shown digit, with one decimal place below 10 of a unit. `None` when
/// `text` is no finite count of zero or more.
pub fn compact(text: &str) -> Option<String> {
    let value = text
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)?;
    if value.round() < 1e3 {
        return whole(text);
    }
    let mut out = None;
    for (size, unit) in COMPACT_UNITS {
        let scaled = value / size;
        let shown = if scaled < 10.0 {
            (scaled * 10.0).round() / 10.0
        } else {
            scaled.round()
        };
        out = Some(format!("{}{unit}", trimmed(format!("{shown:.1}"))));
        if shown < 1e3 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_compact_count_keeps_the_first_digits_and_names_its_unit() {
        for (text, shown) in [
            ("0", "0"),
            ("987", "987"),
            ("999.6", "1k"),
            ("1000", "1k"),
            ("1500", "1.5k"),
            ("9949", "9.9k"),
            ("9950", "10k"),
            ("24312", "24k"),
            ("999499", "999k"),
            ("999500", "1M"),
            ("1840052", "1.8M"),
            ("12400000", "12M"),
            ("999500000", "1B"),
            ("2300000000", "2.3B"),
            ("5000000000000", "5000B"),
        ] {
            assert_eq!(compact(text).as_deref(), Some(shown), "{text}");
        }
    }

    #[test]
    fn text_that_is_no_count_has_no_compact_form() {
        for bad in ["", "yes", "NaN", "inf", "-5"] {
            assert_eq!(compact(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_whole_amount_drops_the_fraction() {
        assert_eq!(whole("38.36").as_deref(), Some("38"));
        assert_eq!(whole("38.5").as_deref(), Some("39"));
        assert_eq!(whole("38").as_deref(), Some("38"));
        assert_eq!(whole("100").as_deref(), Some("100"));
        assert_eq!(whole("0").as_deref(), Some("0"));
        assert_eq!(whole("0.2").as_deref(), Some("0"));
    }

    #[test]
    fn text_that_is_no_amount_has_no_whole_form() {
        for bad in ["", "yes", "NaN", "inf"] {
            assert_eq!(whole(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_published_value_keeps_its_precision_without_the_noise() {
        assert_eq!(precise(2.9999999999999996), "3");
        assert_eq!(precise(289.00000000000006), "289");
        assert_eq!(precise(0.9036144578313253), "0.9036");
        assert_eq!(precise(38.357142857142854), "38.3571");
    }

    #[test]
    fn float_noise_rounds_away() {
        assert_eq!(rounded(2.9999999999999996), "3");
        assert_eq!(rounded(27.999999999999996), "28");
        assert_eq!(rounded(289.00000000000006), "289");
    }

    #[test]
    fn a_long_fraction_keeps_two_decimal_places() {
        assert_eq!(rounded(38.357142857142854), "38.36");
        assert_eq!(rounded(0.9036144578313253), "0.9");
    }

    #[test]
    fn a_short_value_prints_as_is() {
        assert_eq!(rounded(9.9), "9.9");
        assert_eq!(rounded(0.49), "0.49");
        assert_eq!(rounded(50.0), "50");
        assert_eq!(rounded(100.0), "100");
        assert_eq!(rounded(0.0), "0");
    }

    #[test]
    fn a_tiny_negative_value_prints_as_zero() {
        assert_eq!(rounded(-0.0), "0");
        assert_eq!(rounded(-0.001), "0");
    }

    #[test]
    fn a_value_below_the_limit_never_prints_as_the_limit() {
        assert_eq!(rounded_below(2.9999999999999996, 11.0), "3");
        assert_eq!(rounded_below(10.999999999999998, 11.0), "10.99");
        assert_eq!(rounded_below(0.4999, 0.5), "0.49");
        assert_eq!(rounded_below(1.006, 1.0061), "1");
    }
}
