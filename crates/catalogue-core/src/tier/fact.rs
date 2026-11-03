use std::fmt::Display;
use std::ops::Not;

/// A rule condition that is true, false, or unknown for lack of signals.
///
/// `needs` names the missing signals. It is empty whenever `value` is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Fact {
    pub value: Option<bool>,
    pub needs: Vec<&'static str>,
}

impl Fact {
    /// A known condition.
    pub fn known(value: bool) -> Self {
        Self {
            value: Some(value),
            needs: Vec::new(),
        }
    }

    /// An unknown condition that lacks `needs`.
    pub fn unknown(needs: Vec<&'static str>) -> Self {
        Self { value: None, needs }
    }

    /// `test(value)` for the signal `name`, or unknown when the signal is `None`.
    pub fn of<T>(name: &'static str, signal: Option<T>, test: impl FnOnce(T) -> bool) -> Self {
        match signal {
            Some(value) => Self::known(test(value)),
            None => Self::unknown(vec![name]),
        }
    }

    /// True when any fact is true, false when every fact is false, otherwise unknown.
    pub fn any(facts: Vec<Fact>) -> Self {
        if facts.iter().any(Fact::is_true) {
            Self::known(true)
        } else if facts.iter().all(Fact::is_false) {
            Self::known(false)
        } else {
            Self::unknown(facts.into_iter().flat_map(|fact| fact.needs).collect())
        }
    }

    /// False when any fact is false, true when every fact is true, otherwise unknown.
    pub fn all(facts: Vec<Fact>) -> Self {
        if facts.iter().any(Fact::is_false) {
            Self::known(false)
        } else if facts.iter().all(Fact::is_true) {
            Self::known(true)
        } else {
            Self::unknown(facts.into_iter().flat_map(|fact| fact.needs).collect())
        }
    }

    /// Whether the condition is known true.
    pub fn is_true(&self) -> bool {
        self.value == Some(true)
    }

    /// Whether the condition is known false.
    pub fn is_false(&self) -> bool {
        self.value == Some(false)
    }
}

impl Not for Fact {
    type Output = Fact;

    /// The opposite condition. Unknown stays unknown.
    fn not(self) -> Fact {
        Fact {
            value: self.value.map(|value| !value),
            needs: self.needs,
        }
    }
}

/// `value` as text, or `unknown`.
pub(super) fn show<T: Display>(value: Option<T>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "unknown".to_owned(),
    }
}

/// Appends each name in `names` that `list` lacks.
pub(super) fn add_unique(list: &mut Vec<&'static str>, names: &[&'static str]) {
    for name in names {
        if !list.contains(name) {
            list.push(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yes() -> Fact {
        Fact::known(true)
    }

    fn no() -> Fact {
        Fact::known(false)
    }

    fn missing(name: &'static str) -> Fact {
        Fact::of::<bool>(name, None, |value| value)
    }

    #[test]
    fn of_reads_a_present_signal_and_names_a_missing_one() {
        assert_eq!(Fact::of("a", Some(3), |v| v > 2), yes());
        assert_eq!(Fact::of("a", Some(2), |v| v > 2), no());
        assert_eq!(missing("a"), Fact::unknown(vec!["a"]));
    }

    #[test]
    fn any_is_true_on_one_true_and_ignores_unknowns() {
        assert_eq!(Fact::any(vec![missing("a"), yes()]), yes());
        assert_eq!(Fact::any(vec![no(), no()]), no());
        assert_eq!(
            Fact::any(vec![no(), missing("a")]),
            Fact::unknown(vec!["a"])
        );
        assert_eq!(Fact::any(Vec::new()), no());
    }

    #[test]
    fn all_is_false_on_one_false_and_ignores_unknowns() {
        assert_eq!(Fact::all(vec![missing("a"), no()]), no());
        assert_eq!(Fact::all(vec![yes(), yes()]), yes());
        assert_eq!(
            Fact::all(vec![missing("a"), yes(), missing("b")]),
            Fact::unknown(vec!["a", "b"])
        );
        assert_eq!(Fact::all(Vec::new()), yes());
    }

    #[test]
    fn not_flips_known_values_only() {
        assert_eq!(!yes(), no());
        assert_eq!(!missing("a"), Fact::unknown(vec!["a"]));
    }

    #[test]
    fn show_and_add_unique() {
        assert_eq!(show(Some(1.5)), "1.5");
        assert_eq!(show::<u32>(None), "unknown");
        let mut list = vec!["a"];
        add_unique(&mut list, &["b", "a", "b"]);
        assert_eq!(list, ["a", "b"]);
    }
}
