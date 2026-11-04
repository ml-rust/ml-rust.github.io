//! The facts a person reads to choose a project, as text for list rows and the data files.

use catalogue_core::amount_text::whole;
use catalogue_core::{
    ACTIVE_USERS_SIGNAL, CONTRIBUTORS_SIGNAL, DEPENDENTS_SIGNAL, EntryReport, LICENSE_SIGNAL,
    NATIVE_CODE_SIGNAL, NO_GPU_FEATURE, NOT_MEASURED, PYTHON_SIGNAL, RUNS_ON_SEPARATOR,
    RUNS_ON_SIGNAL,
};

use super::date_text::date_text;
use super::license_family::license_family;

/// The id of the latest release fact.
const LATEST_RELEASE_SIGNAL: &str = "last_release";

/// The value of a measured project where the check finds no native code.
const NO_NATIVE_CODE: &str = "No native code found";

/// The start of a native code value that names what the project binds.
const BINDS: &str = "Binds native code: ";

/// The start of a native code value that names only a build tool.
const BUILDS: &str = "Build uses a native toolchain: ";

/// The start of a Python bindings value that names a crate.
const HAS_PYTHON: &str = "Has Python bindings";

/// The feature label of one named hit in a native code value.
const FEATURE_LABEL: &str = " (feature ";

/// The cell word of a value where at least one named hit needs no feature.
const REQUIRED: &str = "Required";

/// The cell word of a value where every hit sits behind a feature.
const OPTIONAL: &str = "Optional";

/// The word a row tag prints for a project with Python bindings.
const PYTHON_BINDINGS: &str = "Python bindings";

/// The value of a measured project where the check finds no Python binding crate.
const NO_PYTHON_CRATE: &str = "No Python binding crate found";

/// What separates the values of a data attribute that holds several.
const VALUE_SEPARATOR: &str = "|";

/// The short form of a value that states the check found nothing.
const NONE_FOUND: &str = "None found";

/// The value of the fact `name` in `record`, or [`NOT_MEASURED`].
pub(crate) fn fact_text(record: Option<&EntryReport>, name: &str) -> String {
    record
        .and_then(|record| record.value_of(name))
        .unwrap_or(NOT_MEASURED)
        .to_owned()
}

/// The amount of the fact `name` in `record` as a whole number, or [`NOT_MEASURED`].
///
/// The signal table keeps the precise value. A list a person scans reads `38`, not `38.36`.
pub(crate) fn whole_text(record: Option<&EntryReport>, name: &str) -> String {
    whole_value(record, name).unwrap_or_else(|| NOT_MEASURED.to_owned())
}

/// The amount of the fact `name` in `record` as a whole number. `None` when not measured.
fn whole_value(record: Option<&EntryReport>, name: &str) -> Option<String> {
    record
        .and_then(|record| record.value_of(name))
        .and_then(whole)
}

/// One value in a row: short text, with the full wording when the two differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CellFact {
    /// The text the cell prints.
    pub short: String,
    /// The exact wording of the fact, when `short` shortens it.
    pub full: Option<String>,
    /// Whether the value prints in the muted colour: a missing value or a found-nothing value.
    pub quiet: bool,
}

impl CellFact {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            short: text.into(),
            full: None,
            quiet: false,
        }
    }

    fn not_measured() -> Self {
        Self {
            short: NOT_MEASURED.to_owned(),
            full: None,
            quiet: true,
        }
    }

    fn found_nothing(full: &str) -> Self {
        Self {
            short: NONE_FOUND.to_owned(),
            full: Some(full.to_owned()),
            quiet: true,
        }
    }

    fn shortened(short: impl Into<String>, full: &str) -> Self {
        Self {
            short: short.into(),
            full: Some(full.to_owned()),
            quiet: false,
        }
    }
}

/// The facts one entry row shows and the raw values its data attributes hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChoiceText {
    /// The license.
    pub license: CellFact,
    /// The GPU backends.
    pub runs_on: CellFact,
    /// The native code statement in short form.
    pub native_code: CellFact,
    /// The Python bindings tag, with the exact fact as `full`. `None` without Python bindings.
    pub python: Option<CellFact>,
    /// The active users, as a whole number.
    pub active_users: CellFact,
    /// The latest release date, as a person reads it.
    pub latest_release: CellFact,
    /// The latest release as an ISO date. `None` when not measured.
    pub released: Option<String>,
    /// The active users for a data attribute. `None` when not measured.
    pub users: Option<String>,
    /// The dependents for a data attribute. `None` when not measured.
    pub dependents: Option<String>,
    /// The contributors for a data attribute. `None` when not measured.
    pub contributors: Option<String>,
    /// The GPU backends for a data attribute, joined by `|`.
    pub gpu_values: String,
    /// The license family for a data attribute.
    pub license_family: &'static str,
    /// The Python bindings state for a data attribute.
    pub python_state: &'static str,
    /// The dormancy state for a data attribute.
    pub dormancy: &'static str,
}

impl ChoiceText {
    /// The facts of `record`. Without a record, every value is [`NOT_MEASURED`].
    pub(crate) fn of(record: Option<&EntryReport>) -> Self {
        let value = |name| record.and_then(|record| record.value_of(name));
        let users = whole_value(record, ACTIVE_USERS_SIGNAL);
        let release = value(LATEST_RELEASE_SIGNAL);
        Self {
            license: value(LICENSE_SIGNAL).map_or_else(CellFact::not_measured, CellFact::plain),
            runs_on: value(RUNS_ON_SIGNAL).map_or_else(CellFact::not_measured, runs_on_fact),
            native_code: value(NATIVE_CODE_SIGNAL).map_or_else(CellFact::not_measured, native_fact),
            python: value(PYTHON_SIGNAL).and_then(python_fact),
            active_users: users
                .clone()
                .map_or_else(CellFact::not_measured, CellFact::plain),
            latest_release: release.map_or_else(CellFact::not_measured, |text| {
                CellFact::plain(date_text(text).unwrap_or_else(|| text.to_owned()))
            }),
            released: release
                .filter(|text| date_text(text).is_some())
                .map(str::to_owned),
            users,
            dependents: whole_value(record, DEPENDENTS_SIGNAL),
            contributors: whole_value(record, CONTRIBUTORS_SIGNAL),
            gpu_values: gpu_values(value(RUNS_ON_SIGNAL)),
            license_family: license_family(value(LICENSE_SIGNAL)),
            python_state: python_state(value(PYTHON_SIGNAL)),
            dormancy: dormancy(record),
        }
    }
}

/// The values of the runs-on `value` for a data attribute: each backend, or the one word for no
/// backend. A value not measured counts as neither.
fn gpu_values(value: Option<&str>) -> String {
    match value {
        None => NOT_MEASURED.to_owned(),
        Some(NO_GPU_FEATURE) => NONE_FOUND.to_owned(),
        Some(names) => names.replace(RUNS_ON_SEPARATOR, VALUE_SEPARATOR),
    }
}

/// The state of the Python `value` for a data attribute.
///
/// Only the exact wording of a found-nothing check reads as none found. Any other value that
/// names no bindings reads as not measured.
fn python_state(value: Option<&str>) -> &'static str {
    match value {
        Some(text) if text.starts_with(HAS_PYTHON) => PYTHON_BINDINGS,
        Some(NO_PYTHON_CRATE) => NONE_FOUND,
        _ => NOT_MEASURED,
    }
}

/// The dormancy of `record` for a data attribute. A finished project is stable, not dormant.
fn dormancy(record: Option<&EntryReport>) -> &'static str {
    match record {
        Some(found) if found.stable => "stable",
        Some(found) if found.dormant == Some(true) => "dormant",
        Some(found) if found.dormant == Some(false) => "active",
        _ => "not-measured",
    }
}

/// The runs-on `value` for a table cell.
fn runs_on_fact(value: &str) -> CellFact {
    if value == NO_GPU_FEATURE {
        CellFact::found_nothing(value)
    } else {
        CellFact::plain(value)
    }
}

/// The native code `value` for a table cell. The full value stays reachable.
fn native_fact(value: &str) -> CellFact {
    if value == NO_NATIVE_CODE {
        return CellFact::found_nothing(value);
    }
    if let Some(rest) = value.strip_prefix(BINDS) {
        return CellFact::shortened(binding_word(rest), value);
    }
    if value.starts_with(BUILDS) {
        return CellFact::shortened("Build tool", value);
    }
    CellFact::plain(value)
}

/// [`REQUIRED`] when a name in `list` has no feature label, else [`OPTIONAL`].
///
/// The fact lists required names first, ahead of its `and N more` cap. A required name past the
/// cap cannot exist, so the printed names decide the word.
fn binding_word(list: &str) -> &'static str {
    let names = list.split(". ").next().unwrap_or(list);
    let shown = names
        .rsplit_once(" and ")
        .filter(|(_, tail)| {
            tail.strip_suffix(" more")
                .is_some_and(|count| count.parse::<usize>().is_ok())
        })
        .map_or(names, |(shown, _)| shown);
    if shown.split(", ").all(|name| name.contains(FEATURE_LABEL)) {
        OPTIONAL
    } else {
        REQUIRED
    }
}

/// The Python `value` for a table cell. `None` when it names no bindings.
fn python_fact(value: &str) -> Option<CellFact> {
    value
        .starts_with(HAS_PYTHON)
        .then(|| CellFact::shortened(PYTHON_BINDINGS, value))
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Date, SignalFact, SourceRef, Tier};
    use url::Url;

    use super::*;

    fn fact(name: &str, value: Option<&str>) -> SignalFact {
        SignalFact {
            name: name.to_owned(),
            label: name.to_owned(),
            value: value.map(str::to_owned),
            source: Some(SourceRef::new(
                "dump",
                Url::parse("https://crates.io/data-access").expect("url"),
            )),
            fetched: Date::new(2026, 11, 2).ok(),
            counts_toward_tier: false,
        }
    }

    fn record(signals: Vec<SignalFact>) -> EntryReport {
        EntryReport {
            tier: Tier::Listed,
            stable: false,
            dormant: None,
            demotion_pending: None,
            reasons: Vec::new(),
            unknown: Vec::new(),
            notes: Vec::new(),
            signals,
        }
    }

    #[test]
    fn a_row_prints_amounts_as_whole_numbers() {
        let found = record(vec![
            fact(LICENSE_SIGNAL, Some("MIT")),
            fact(RUNS_ON_SIGNAL, Some("CUDA, Metal")),
            fact(ACTIVE_USERS_SIGNAL, Some("38.36")),
            fact(DEPENDENTS_SIGNAL, Some("12")),
            fact(LATEST_RELEASE_SIGNAL, Some("2026-10-31")),
        ]);
        let text = ChoiceText::of(Some(&found));
        assert_eq!(text.license, CellFact::plain("MIT"));
        assert_eq!(text.runs_on, CellFact::plain("CUDA, Metal"));
        assert_eq!(text.active_users, CellFact::plain("38"));
        assert_eq!(text.users.as_deref(), Some("38"));
        assert_eq!(text.dependents.as_deref(), Some("12"));
        assert_eq!(text.latest_release, CellFact::plain("31 Oct 2026"));
        assert_eq!(text.released.as_deref(), Some("2026-10-31"));
    }

    #[test]
    fn an_unknown_value_reads_not_measured() {
        let found = record(vec![fact(LICENSE_SIGNAL, None)]);
        for text in [ChoiceText::of(Some(&found)), ChoiceText::of(None)] {
            for cell in [
                &text.license,
                &text.runs_on,
                &text.native_code,
                &text.active_users,
                &text.latest_release,
            ] {
                assert_eq!(cell, &CellFact::not_measured());
            }
            assert_eq!(text.python, None);
            assert_eq!(text.users, None);
        }
    }

    #[test]
    fn an_amount_that_is_no_number_reads_not_measured() {
        let found = record(vec![fact(ACTIVE_USERS_SIGNAL, Some("many"))]);
        assert_eq!(whole_text(Some(&found), ACTIVE_USERS_SIGNAL), NOT_MEASURED);
    }

    #[test]
    fn a_found_nothing_value_shortens_and_keeps_its_wording() {
        let gpu = runs_on_fact(NO_GPU_FEATURE);
        assert_eq!(gpu.short, NONE_FOUND);
        assert_eq!(gpu.full.as_deref(), Some(NO_GPU_FEATURE));
        assert!(gpu.quiet);
        let native = native_fact(NO_NATIVE_CODE);
        assert_eq!(native.short, NONE_FOUND);
        assert_eq!(native.full.as_deref(), Some(NO_NATIVE_CODE));
    }

    #[test]
    fn a_native_value_says_whether_the_binding_is_required() {
        for (value, short) in [
            ("Binds native code: ort-sys", REQUIRED),
            ("Binds native code: a (feature x), b", REQUIRED),
            ("Binds native code: a, b (feature x) and 5 more", REQUIRED),
            (
                "Binds native code: a. Build uses a native toolchain: cc",
                REQUIRED,
            ),
            ("Binds native code: a (feature x)", OPTIONAL),
            ("Binds native code: a (feature x), b (feature y)", OPTIONAL),
            (
                "Binds native code: a (feature x), b (feature y) and 5 more",
                OPTIONAL,
            ),
            (
                "Binds native code: a (feature x). Build uses a native toolchain: cc",
                OPTIONAL,
            ),
            ("Build uses a native toolchain: cc", "Build tool"),
        ] {
            let found = native_fact(value);
            assert_eq!(found.short, short, "{value}");
            assert_eq!(found.full.as_deref(), Some(value));
            assert!(!found.quiet);
        }
        assert_eq!(
            native_fact("Other wording"),
            CellFact::plain("Other wording")
        );
    }

    #[test]
    fn the_gpu_values_hold_each_backend_and_keep_the_two_other_answers_apart() {
        assert_eq!(gpu_values(Some("CUDA, Metal")), "CUDA|Metal");
        assert_eq!(gpu_values(Some("CUDA")), "CUDA");
        assert_eq!(gpu_values(Some(NO_GPU_FEATURE)), NONE_FOUND);
        assert_eq!(gpu_values(None), NOT_MEASURED);
    }

    #[test]
    fn the_python_state_never_turns_a_missing_value_into_an_answer() {
        assert_eq!(
            python_state(Some("Has Python bindings: a needs Python")),
            PYTHON_BINDINGS
        );
        assert_eq!(python_state(Some(NO_PYTHON_CRATE)), NONE_FOUND);
        assert_eq!(python_state(None), NOT_MEASURED);
        assert_eq!(python_state(Some("Other wording")), NOT_MEASURED);
    }

    #[test]
    fn the_dormancy_keeps_stable_and_unknown_apart_from_dormant() {
        let mut found = record(Vec::new());
        assert_eq!(dormancy(None), "not-measured");
        assert_eq!(dormancy(Some(&found)), "not-measured");
        found.dormant = Some(false);
        assert_eq!(dormancy(Some(&found)), "active");
        found.dormant = Some(true);
        assert_eq!(dormancy(Some(&found)), "dormant");
        found.stable = true;
        assert_eq!(dormancy(Some(&found)), "stable");
    }

    #[test]
    fn only_a_python_binding_prints() {
        assert_eq!(python_fact("No Python binding crate found"), None);
        let found = python_fact("Has Python bindings: a needs Python").expect("fact");
        assert_eq!(found.short, PYTHON_BINDINGS);
        assert_eq!(
            found.full.as_deref(),
            Some("Has Python bindings: a needs Python")
        );
    }
}
