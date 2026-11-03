use thiserror::Error;

/// A rejected identifier value, with the rule it broke.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid {kind} `{value}`: {rule}")]
pub struct IdError {
    kind: &'static str,
    value: String,
    rule: &'static str,
}

impl IdError {
    pub(super) fn new(kind: &'static str, value: &str, rule: &'static str) -> Self {
        Self {
            kind,
            value: value.to_owned(),
            rule,
        }
    }

    /// The identifier type that rejected the value.
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    /// The rejected value.
    pub fn value(&self) -> &str {
        &self.value
    }
}
