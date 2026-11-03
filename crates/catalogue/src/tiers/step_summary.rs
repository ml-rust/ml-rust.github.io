//! The fallback list in the GitHub Actions job summary.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;

use catalogue_signals::fallback::FetchSummary;

/// The environment variable that names the job summary file.
pub(super) const STEP_SUMMARY_VAR: &str = "GITHUB_STEP_SUMMARY";

/// Appends `summary` as markdown to the job summary file at `path`.
///
/// # Errors
///
/// Returns the error from the operating system.
pub(super) fn append_step_summary(path: &Path, summary: &FetchSummary) -> io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(summary.markdown().as_bytes())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn scratch() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
        fs::create_dir_all(&base).expect("target dir");
        tempfile::tempdir_in(&base).expect("temp dir")
    }

    #[test]
    fn appends_to_what_the_file_holds() {
        let dir = scratch();
        let path = dir.path().join("summary.md");
        fs::write(&path, "earlier step\n").expect("writes");
        let summary = FetchSummary {
            fetches: 2,
            failures: Vec::new(),
        };
        append_step_summary(&path, &summary).expect("appends");
        let text = fs::read_to_string(&path).expect("reads");
        assert_eq!(
            text,
            "earlier step\n## Signal fallbacks: 0 of 2 network fetches\n\n"
        );
    }

    #[test]
    fn a_path_that_cannot_open_is_an_error() {
        let dir = scratch();
        let error = append_step_summary(dir.path(), &FetchSummary::default());
        assert!(error.is_err());
    }
}
