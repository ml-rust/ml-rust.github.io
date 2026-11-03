use super::fact::add_unique;
use super::verdict::Reason;

/// Reasons and unknown signals collected in rule order.
#[derive(Default)]
pub(super) struct Notes {
    pub reasons: Vec<Reason>,
    pub unknown: Vec<&'static str>,
}

impl Notes {
    /// Appends one reason.
    pub fn reason(&mut self, reason: Reason) {
        self.reasons.push(reason);
    }

    /// Records each signal name in `names` once.
    pub fn needs(&mut self, names: &[&'static str]) {
        add_unique(&mut self.unknown, names);
    }
}

#[cfg(test)]
mod tests {
    use super::super::verdict::Rule;
    use super::*;

    #[test]
    fn reasons_keep_order_and_unknown_names_are_unique() {
        let mut notes = Notes::default();
        notes.reason(Reason::new(Rule::Dormancy, "a"));
        notes.reason(Reason::new(Rule::Rot, "b"));
        notes.needs(&["x", "y"]);
        notes.needs(&["y", "z"]);
        let rules: Vec<Rule> = notes.reasons.iter().map(|r| r.rule).collect();
        assert_eq!(rules, [Rule::Dormancy, Rule::Rot]);
        assert_eq!(notes.unknown, ["x", "y", "z"]);
    }
}
