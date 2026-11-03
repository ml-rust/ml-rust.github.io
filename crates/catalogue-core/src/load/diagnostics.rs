use std::error::Error;
use std::fmt;
use std::slice;

/// One error found in the data, tied to the file that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Path of the file.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// Every error found in a data set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diagnostics(Vec<Diagnostic>);

impl Diagnostics {
    /// Creates an empty list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one diagnostic.
    pub fn push(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.0.push(Diagnostic {
            path: path.into(),
            message: message.into(),
        });
    }

    /// Appends all diagnostics of `other`.
    pub fn extend(&mut self, other: Diagnostics) {
        self.0.extend(other.0);
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The number of diagnostics.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Iterates over the diagnostics in the order found.
    pub fn iter(&self) -> slice::Iter<'_, Diagnostic> {
        self.0.iter()
    }

    /// The diagnostics as a slice.
    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.0
    }
}

impl<'a> IntoIterator for &'a Diagnostics {
    type Item = &'a Diagnostic;
    type IntoIter = slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, item) in self.0.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{item}")?;
        }
        Ok(())
    }
}

impl Error for Diagnostics {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_one_line_per_diagnostic() {
        let mut d = Diagnostics::new();
        d.push("a.toml", "first");
        d.push("b.toml", "second");
        assert_eq!(d.to_string(), "a.toml: first\nb.toml: second");
        assert_eq!(d.len(), 2);
        assert!(!d.is_empty());
    }

    #[test]
    fn extend_appends_in_order() {
        let mut a = Diagnostics::new();
        a.push("a", "1");
        let mut b = Diagnostics::new();
        b.push("b", "2");
        a.extend(b);
        let paths: Vec<&str> = a.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(paths, ["a", "b"]);
    }

    #[test]
    fn is_an_error() {
        fn takes_error(_: &dyn Error) {}
        takes_error(&Diagnostics::new());
    }
}
