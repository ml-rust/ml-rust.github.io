use std::fmt;
use std::path::Path;

use catalogue_core::{Catalogue, Diagnostics};

use crate::data_dir::{DataDirError, read_data_dir};

/// Counts of what a valid data set holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    /// Number of entries.
    pub entries: usize,
    /// Number of judgments.
    pub judgments: usize,
    /// Number of topics.
    pub topics: usize,
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ok: {} entries, {} judgments, {} topics",
            self.entries, self.judgments, self.topics
        )
    }
}

/// The result of checking a data directory.
#[derive(Debug)]
pub enum Outcome {
    /// Every rule holds.
    Valid(Summary),
    /// At least one rule fails. Holds every error found.
    Invalid(Diagnostics),
}

/// Reads the data directory and checks every rule.
///
/// Read errors return `Err`. Data errors return `Ok(Outcome::Invalid)`.
pub fn validate(root: &Path) -> Result<Outcome, DataDirError> {
    let data = read_data_dir(root)?;
    let mut diagnostics = data.diagnostics;
    match Catalogue::load(data.sources) {
        Ok(catalogue) if diagnostics.is_empty() => Ok(Outcome::Valid(Summary {
            entries: catalogue.entries().len(),
            judgments: catalogue.judgments().len(),
            topics: catalogue.topics().len(),
        })),
        Ok(_) => Ok(Outcome::Invalid(diagnostics)),
        Err(found) => {
            diagnostics.extend(found);
            Ok(Outcome::Invalid(diagnostics))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_has_a_fixed_format() {
        let summary = Summary {
            entries: 3,
            judgments: 2,
            topics: 5,
        };
        assert_eq!(summary.to_string(), "ok: 3 entries, 2 judgments, 5 topics");
    }
}
