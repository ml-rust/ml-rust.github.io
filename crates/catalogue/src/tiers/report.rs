//! The JSON tier report of `catalogue tiers --json`.

use std::io::{self, Write};
use std::path::Path;

use catalogue_core::{Date, EntryReport, Thresholds, TierReport};
use catalogue_signals::assemble::{choice_signal_facts, signal_facts};
use catalogue_signals::atomic::write_atomic;

use super::assess::Assessment;
use super::error::TiersError;

/// The tier report of the check on `today`: one record per assessment, keyed by slug.
///
/// Each record holds the verdict, the notes and every signal with its source and data date.
pub(crate) fn tier_report(
    assessments: &[Assessment],
    thresholds: &Thresholds,
    today: Date,
) -> TierReport {
    let entries = assessments
        .iter()
        .map(|assessment| {
            let mut facts =
                signal_facts(&assessment.signals, &assessment.sources, assessment.commits);
            facts.extend(choice_signal_facts(&assessment.choice, &assessment.sources));
            let record = EntryReport::new(&assessment.verdict, assessment.notes.clone(), facts);
            (assessment.slug.clone(), record)
        })
        .collect();
    TierReport::new(today, thresholds, entries)
}

/// The citation problems of every signal in `report`, each prefixed by its entry slug.
fn uncited(report: &TierReport) -> Vec<String> {
    report
        .entries
        .iter()
        .flat_map(|(slug, entry)| {
            entry
                .signals
                .iter()
                .flat_map(|fact| fact.problems())
                .map(move |problem| format!("{slug}: {problem}"))
        })
        .collect()
}

/// Writes `report` to `path` as pretty JSON with a final newline, replacing the file atomically.
///
/// A report with an uncited signal is never written: every published value links its source.
///
/// # Errors
///
/// Returns [`TiersError::Uncited`] when a signal has a value without a source or date.
/// Returns [`TiersError::Report`] when the directory, the temporary file or the rename fails.
pub(crate) fn write_report(path: &Path, report: &TierReport) -> Result<(), TiersError> {
    let problems = uncited(report);
    if !problems.is_empty() {
        return Err(TiersError::Uncited { problems });
    }
    write_atomic(path, |file| {
        serde_json::to_writer_pretty(&mut *file, report).map_err(io::Error::from)?;
        file.write_all(b"\n")
    })
    .map_err(|error| TiersError::Report {
        path: error.path,
        source: error.source,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use catalogue_core::REPORT_SCHEMA;

    use super::*;

    fn scratch() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
        fs::create_dir_all(&base).expect("target dir");
        tempfile::tempdir_in(&base).expect("temp dir")
    }

    #[test]
    fn writes_pretty_json_that_reads_back() {
        let dir = scratch();
        let path = dir.path().join("out").join("report.json");
        let thresholds: Thresholds =
            toml::from_str(include_str!("../../../../data/thresholds.toml")).expect("thresholds");
        let today = Date::new(2026, 11, 3).expect("date");
        let report = tier_report(&[], &thresholds, today);
        write_report(&path, &report).expect("writes");
        let text = fs::read_to_string(&path).expect("reads");
        assert!(text.starts_with("{\n  \"schema\": "), "{text}");
        assert!(text.contains("\"generated\": \"2026-11-03\""), "{text}");
        assert!(text.ends_with("}\n"), "{text}");
        let back: TierReport = serde_json::from_str(&text).expect("parses");
        assert_eq!(back, report);
        assert_eq!(back.schema, REPORT_SCHEMA);
        assert!(back.entries.is_empty());
        let names: Vec<String> = fs::read_dir(path.parent().expect("parent"))
            .expect("dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, ["report.json"]);
    }

    #[test]
    fn an_uncited_value_blocks_the_write() {
        let dir = scratch();
        let path = dir.path().join("report.json");
        let fact = catalogue_core::SignalFact {
            name: "active_users".to_owned(),
            label: "Active dependents".to_owned(),
            value: Some("3".to_owned()),
            source: None,
            fetched: None,
            counts_toward_tier: true,
        };
        let entry = EntryReport {
            tier: catalogue_core::Tier::Listed,
            stable: false,
            dormant: None,
            demotion_pending: None,
            reasons: Vec::new(),
            unknown: Vec::new(),
            notes: Vec::new(),
            signals: vec![fact],
        };
        let slug = catalogue_core::Slug::new("alpha").expect("slug");
        let report = TierReport {
            schema: REPORT_SCHEMA,
            generated: Date::new(2026, 11, 3).expect("date"),
            thresholds_version: 1,
            calibrated: None,
            entries: [(slug, entry)].into_iter().collect(),
        };
        let error = write_report(&path, &report).expect_err("uncited");
        let TiersError::Uncited { problems } = &error else {
            panic!("expected Uncited, got {error}");
        };
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(
            problems.iter().all(|p| p.starts_with("alpha: ")),
            "{problems:?}"
        );
        assert!(!path.exists());
    }

    #[test]
    fn a_path_without_a_file_name_is_an_error() {
        let report = TierReport {
            schema: REPORT_SCHEMA,
            generated: Date::new(2026, 11, 3).expect("date"),
            thresholds_version: 1,
            calibrated: None,
            entries: Default::default(),
        };
        let error = write_report(Path::new("/"), &report).expect_err("no file name");
        assert!(matches!(error, TiersError::Report { .. }));
        assert!(error.to_string().contains("--json"), "{error}");
    }
}
