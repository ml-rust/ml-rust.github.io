//! Reads a JSON lines file of sandbox records.

use std::io::BufRead;
use std::path::Path;

use serde::de::DeserializeOwned;

use super::error::SandboxError;

/// Reads one `T` per non-blank line of `reader`.
///
/// `path` names the file in errors.
///
/// # Errors
///
/// Returns [`SandboxError::Read`] when a line cannot be read and
/// [`SandboxError::Corrupt`] when a line is not a valid `T`.
pub fn read_lines<T: DeserializeOwned>(
    reader: impl BufRead,
    path: &Path,
) -> Result<Vec<T>, SandboxError> {
    let mut out = Vec::new();
    for (index, text) in reader.lines().enumerate() {
        let text = text.map_err(|source| SandboxError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        if text.trim().is_empty() {
            continue;
        }
        let value = serde_json::from_str(&text).map_err(|error| SandboxError::Corrupt {
            path: path.to_path_buf(),
            line: index.saturating_add(1),
            reason: error.to_string(),
        })?;
        out.push(value);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::CheckItem;

    #[test]
    fn blank_lines_are_skipped_and_a_bad_line_names_its_number() {
        let good = "{\"entry\":\"a\",\"crate\":\"a\",\"version\":\"1.0.0\"}\n\n";
        let items = read_lines::<CheckItem>(good.as_bytes(), Path::new("list.jsonl"));
        assert_eq!(items.expect("reads").len(), 1);
        let bad = format!("{good}{{\"entry\":1}}\n");
        let error =
            read_lines::<CheckItem>(bad.as_bytes(), Path::new("list.jsonl")).expect_err("corrupt");
        let message = error.to_string();
        assert!(
            message.contains("list.jsonl is corrupt at line 3"),
            "{message}"
        );
    }
}
