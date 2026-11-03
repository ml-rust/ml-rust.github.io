//! The value printed for a fact that does not exist, shared by every report.

use std::fmt::Display;

/// The value printed for a fact that does not exist.
pub(crate) const NONE: &str = "-";

/// The text of an optional value, or [`NONE`].
pub(crate) fn or_none<T: Display>(value: Option<T>) -> String {
    value.map_or_else(|| NONE.to_owned(), |value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_prints_as_its_text() {
        assert_eq!(or_none(Some(7)), "7");
        assert_eq!(or_none(Some("a")), "a");
    }

    #[test]
    fn a_missing_value_prints_a_dash() {
        assert_eq!(or_none(None::<u32>), "-");
        assert_eq!(NONE, "-");
    }
}
