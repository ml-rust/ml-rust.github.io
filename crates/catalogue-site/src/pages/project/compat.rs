//! The compatibility block: what the project needs and runs on, with each value's source.

use catalogue_core::{
    CONTRIBUTORS_SIGNAL, DEPENDENTS_SIGNAL, Entry, EntryReport, LICENSE_SIGNAL, NATIVE_CODE_SIGNAL,
    NOT_MEASURED, PYTHON_SIGNAL, RUNS_ON_EVIDENCE_SIGNAL, RUNS_ON_SIGNAL,
};

use super::sources::{SourceNote, fact_named};
use crate::pages::shared::{fact_text, native_code_fact, whole_text};

/// The separator between the backends of a runs-on evidence value.
const EVIDENCE_SEPARATOR: &str = "; ";
/// The separator between the statements of a native code value.
const NATIVE_SEPARATOR: &str = ". ";

/// One row of the block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompatRow<'a> {
    /// The fact name.
    pub label: &'static str,
    /// The value, or the plain word for it.
    pub value: String,
    /// Whether the value prints in the muted colour: a value not measured.
    pub quiet: bool,
    /// What the lines say, such as `What shows it`.
    pub lines_label: Option<&'static str>,
    /// The exact statements behind the value.
    pub lines: Vec<String>,
    /// The source and date of the value. `None` when not measured.
    pub source: Option<SourceNote<'a>>,
}

/// The data of the compatibility block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Compat<'a> {
    /// The rows.
    pub rows: Vec<CompatRow<'a>>,
    /// The line that stands for the facts of a crate, on an entry with no crate.
    pub no_crate: bool,
}

impl<'a> Compat<'a> {
    /// The block for `entry`, with the facts of `record` when given.
    pub(crate) fn new(entry: &Entry, record: Option<&'a EntryReport>) -> Self {
        let no_crate = entry.crates.is_empty();
        let mut rows = Vec::new();
        if !no_crate {
            rows.push(runs_on(record));
            rows.push(native_code(record));
            rows.push(python(record));
        }
        rows.push(fact_row("License", LICENSE_SIGNAL, record, false));
        if !no_crate {
            rows.push(fact_row("Dependents", DEPENDENTS_SIGNAL, record, true));
        }
        rows.push(fact_row("Contributors", CONTRIBUTORS_SIGNAL, record, true));
        Self { rows, no_crate }
    }
}

/// The source of the signal `name` in `record`.
fn source<'a>(record: Option<&'a EntryReport>, name: &str) -> Option<SourceNote<'a>> {
    record
        .and_then(|record| fact_named(record, name))
        .and_then(SourceNote::of)
}

/// The row of the plain fact `name`. `whole` prints an amount as a whole number.
fn fact_row<'a>(
    label: &'static str,
    name: &str,
    record: Option<&'a EntryReport>,
    whole: bool,
) -> CompatRow<'a> {
    let value = if whole {
        whole_text(record, name)
    } else {
        fact_text(record, name)
    };
    CompatRow {
        label,
        quiet: value == NOT_MEASURED,
        value,
        lines_label: None,
        lines: Vec::new(),
        source: source(record, name),
    }
}

/// The runs-on row, with what shows each backend.
fn runs_on(record: Option<&EntryReport>) -> CompatRow<'_> {
    let mut row = fact_row("Runs on", RUNS_ON_SIGNAL, record, false);
    if let Some(evidence) = record.and_then(|record| record.value_of(RUNS_ON_EVIDENCE_SIGNAL)) {
        row.lines_label = Some("What shows it");
        row.lines = evidence
            .split(EVIDENCE_SEPARATOR)
            .map(str::to_owned)
            .collect();
    }
    row
}

/// The Python row: the statement of the check, then one line per crate it names.
fn python(record: Option<&EntryReport>) -> CompatRow<'_> {
    let mut row = fact_row("Python bindings", PYTHON_SIGNAL, record, false);
    if let Some((word, crates)) = row.value.clone().split_once(": ") {
        row.value = word.to_owned();
        row.lines = crates.split(", ").map(str::to_owned).collect();
    }
    row
}

/// The native code row: the plain word first, then the exact statements of the check.
fn native_code(record: Option<&EntryReport>) -> CompatRow<'_> {
    let fact = native_code_fact(record);
    let lines = fact
        .full
        .as_deref()
        .map(|full| full.split(NATIVE_SEPARATOR).map(str::to_owned).collect())
        .unwrap_or_default();
    CompatRow {
        label: "Native code",
        value: fact.short,
        quiet: fact.quiet && fact.full.is_none(),
        lines_label: None,
        lines,
        source: source(record, NATIVE_CODE_SIGNAL),
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::{
        CrateName, Date, Description, DescriptionSource, EntryKind, SignalFact, Slug, Tier,
    };
    use url::Url;

    use super::*;

    fn entry(crates: &[&str]) -> Entry {
        let repo = Url::parse("https://github.com/o/tool").expect("repo");
        Entry {
            name: "tool".to_owned(),
            kind: EntryKind::Library,
            repo,
            homepage: None,
            crates: crates
                .iter()
                .map(|name| CrateName::new(*name).expect("crate name"))
                .collect(),
            topics: vec![Slug::new("topic").expect("slug")],
            category: None,
            part_of: None,
            description: Description {
                text: "A tool.".to_owned(),
                source: DescriptionSource::Human,
                source_url: None,
            },
            org_affiliated: false,
            distribution: None,
            added: Date::new(2026, 11, 3).expect("date"),
        }
    }

    fn fact(name: &str, value: &str) -> SignalFact {
        SignalFact {
            name: name.to_owned(),
            label: name.to_owned(),
            value: Some(value.to_owned()),
            source: None,
            fetched: None,
            counts_toward_tier: false,
        }
    }

    fn record(signals: Vec<SignalFact>) -> EntryReport {
        EntryReport {
            tier: Tier::Listed,
            stable: false,
            dormant: Some(false),
            demotion_pending: None,
            reasons: Vec::new(),
            unknown: Vec::new(),
            notes: Vec::new(),
            signals,
        }
    }

    fn row<'a>(compat: &'a Compat<'_>, label: &str) -> &'a CompatRow<'a> {
        compat
            .rows
            .iter()
            .find(|row| row.label == label)
            .expect("a row")
    }

    #[test]
    fn native_code_says_the_word_and_keeps_the_exact_names_and_feature_labels() {
        let record = record(vec![fact(
            NATIVE_CODE_SIGNAL,
            "Binds native code: onnxruntime, ort-sys. Build uses a native toolchain: pkg-config (feature pkg-config)",
        )]);
        let block = Compat::new(&entry(&["ort"]), Some(&record));
        let native = row(&block, "Native code");
        assert_eq!(native.value, "Required");
        assert_eq!(
            native.lines,
            [
                "Binds native code: onnxruntime, ort-sys",
                "Build uses a native toolchain: pkg-config (feature pkg-config)"
            ]
        );
    }

    #[test]
    fn no_gpu_feature_stays_in_the_words_of_the_check() {
        let record = record(vec![
            fact(RUNS_ON_SIGNAL, "No GPU feature found"),
            fact(
                RUNS_ON_EVIDENCE_SIGNAL,
                "no dependency or feature matches a backend row",
            ),
        ]);
        let block = Compat::new(&entry(&["ort"]), Some(&record));
        let runs = row(&block, "Runs on");
        assert_eq!(runs.value, "No GPU feature found");
        assert_eq!(
            runs.lines,
            ["no dependency or feature matches a backend row"]
        );
        assert_eq!(runs.lines_label, Some("What shows it"));
    }

    #[test]
    fn python_bindings_keep_the_words_of_the_check_with_one_line_per_crate() {
        let record = record(vec![fact(
            PYTHON_SIGNAL,
            "Has Python bindings: a needs Python only with feature pyo3, b needs Python",
        )]);
        let block = Compat::new(&entry(&["a"]), Some(&record));
        let python = row(&block, "Python bindings");
        assert_eq!(python.value, "Has Python bindings");
        assert_eq!(
            python.lines,
            ["a needs Python only with feature pyo3", "b needs Python"]
        );
    }

    #[test]
    fn each_backend_gets_its_own_line() {
        let record = record(vec![fact(
            RUNS_ON_EVIDENCE_SIGNAL,
            "CUDA: a feature cuda; Metal: a feature metal",
        )]);
        let block = Compat::new(&entry(&["a"]), Some(&record));
        assert_eq!(
            row(&block, "Runs on").lines,
            ["CUDA: a feature cuda", "Metal: a feature metal"]
        );
    }

    #[test]
    fn an_entry_with_no_crate_leaves_no_hole_for_crate_facts() {
        let block = Compat::new(&entry(&[]), None);
        assert!(block.no_crate);
        let labels: Vec<&str> = block.rows.iter().map(|row| row.label).collect();
        assert_eq!(labels, ["License", "Contributors"]);
    }

    #[test]
    fn an_unknown_value_reads_not_measured() {
        let block = Compat::new(&entry(&["a"]), None);
        assert!(
            block
                .rows
                .iter()
                .all(|row| row.value == NOT_MEASURED && row.quiet)
        );
    }
}
