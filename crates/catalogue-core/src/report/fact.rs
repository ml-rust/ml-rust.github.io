//! One signal as an entry page shows it: raw value, source link and fetch date.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::date::Date;
use crate::date_text::plain_option;

/// The id of the GitHub star count fact. Stars count only with forks and contributors, on the
/// repository route. Stars alone never qualify a project.
pub const STARS_SIGNAL: &str = "stars";

/// The id of the GitHub fork count fact. A part of the repository route.
pub const FORKS_SIGNAL: &str = "forks";

/// The id of the total commits on the default branch. No tier rule reads it.
pub const COMMITS_SIGNAL: &str = "commits";

/// The id of the active dependents fact. A tier rule reads it.
pub const ACTIVE_USERS_SIGNAL: &str = "active_users";

/// The id of the new dependents fact. A tier rule reads it.
pub const NEW_ADOPTERS_SIGNAL: &str = "new_adopters";

/// The id of the peak active dependents fact. A tier rule reads it.
pub const PEAK_USERS_SIGNAL: &str = "peak_users";

/// The id of the share of the peak that stays active. A tier rule reads it.
pub const FORMER_GLORY_SIGNAL: &str = "former_glory";

/// The id of the crates.io downloads over 30 days of a library.
pub const DOWNLOADS_SIGNAL: &str = "downloads_per_month";

/// The id of the GitHub release downloads per release of an app.
pub const BINARY_PER_RELEASE_SIGNAL: &str = "binary_downloads_per_release";

/// What a person reads for [`ACTIVE_USERS_SIGNAL`].
pub const ACTIVE_DEPENDENTS_LABEL: &str = "Active dependents";

/// What a person reads for [`NEW_ADOPTERS_SIGNAL`].
pub const NEW_DEPENDENTS_LABEL: &str = "New dependents (365 days)";

/// What a person reads for [`PEAK_USERS_SIGNAL`].
pub const PEAK_DEPENDENTS_LABEL: &str = "Peak active dependents";

/// What a person reads for [`FORMER_GLORY_SIGNAL`].
pub const FORMER_GLORY_LABEL: &str = "Active dependents as a share of the peak";

/// The id of the license fact: an SPDX expression or [`NONE_STATED`].
pub const LICENSE_SIGNAL: &str = "license";

/// The id of the backends fact: names joined with [`RUNS_ON_SEPARATOR`], or [`NO_GPU_FEATURE`].
pub const RUNS_ON_SIGNAL: &str = "runs_on";

/// The id of the fact that names the crate and the feature or dependency behind each backend.
pub const RUNS_ON_EVIDENCE_SIGNAL: &str = "runs_on_evidence";

/// The id of the native code fact.
pub const NATIVE_CODE_SIGNAL: &str = "native_code";

/// The words in a crate name that mark a native ML engine, as opposed to a support library such as
/// `ring` or `zstd-sys`.
const NATIVE_ENGINE_WORDS: [&str; 18] = [
    "ctranslate2",
    "diffusion",
    "ggml",
    "leptonica",
    "libtorch",
    "llama",
    "mlx",
    "mnn",
    "onnxruntime",
    "opencv",
    "ort",
    "paddle",
    "pdfium",
    "sherpa",
    "tensorflow",
    "tesseract",
    "torch",
    "whisper",
];

/// Whether the native code fact `fact` shows a project whose work runs in a native ML engine: a
/// binding that no feature gates, to one of the engines above.
pub fn wraps_native_engine(fact: &str) -> bool {
    let Some(list) = fact.strip_prefix("Binds native code: ") else {
        return false;
    };
    list.split(". ")
        .next()
        .unwrap_or(list)
        .split(", ")
        .filter(|name| !name.contains("(feature"))
        .any(|name| {
            name.split(|c: char| !c.is_ascii_alphanumeric())
                .any(|word| NATIVE_ENGINE_WORDS.contains(&word))
        })
}

/// The id of the Python bindings fact.
pub const PYTHON_SIGNAL: &str = "python_bindings";

/// The id of the dependents fact.
pub const DEPENDENTS_SIGNAL: &str = "dependents";

/// The id of the contributors fact. A part of the repository route.
pub const CONTRIBUTORS_SIGNAL: &str = "contributors";

/// The text between two backend names in the [`RUNS_ON_SIGNAL`] value.
pub const RUNS_ON_SEPARATOR: &str = ", ";

/// The [`RUNS_ON_SIGNAL`] value of a measured project with no backend row.
///
/// The check finds no feature or dependency that names a GPU backend. The project can still run
/// on a GPU, so the value states no answer.
pub const NO_GPU_FEATURE: &str = "GPU support unknown";

/// The [`LICENSE_SIGNAL`] value of a project that states no license.
pub const NONE_STATED: &str = "None stated";

/// What a page shows for a fact with no value.
pub const NOT_MEASURED: &str = "Not measured";

/// What a page shows for a fact the entry kind cannot have, such as the dependents of an app.
pub const NOT_APPLICABLE: &str = "Not applicable";

/// The facts that help a person choose a project. No tier rule reads them.
const NEVER_COUNT: [&str; 7] = [
    COMMITS_SIGNAL,
    LICENSE_SIGNAL,
    RUNS_ON_SIGNAL,
    RUNS_ON_EVIDENCE_SIGNAL,
    NATIVE_CODE_SIGNAL,
    PYTHON_SIGNAL,
    DEPENDENTS_SIGNAL,
];

/// Whether the fact `name` stays out of every tier rule.
pub fn never_counts(name: &str) -> bool {
    NEVER_COUNT.contains(&name)
}

/// Where a signal value comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    /// The source name shown beside the link, such as `crates.io database dump`.
    pub label: String,
    /// The page that holds the value.
    pub url: Url,
}

impl SourceRef {
    /// A source named `label` at `url`.
    pub fn new(label: impl Into<String>, url: Url) -> Self {
        Self {
            label: label.into(),
            url,
        }
    }
}

/// One signal of one entry, with its provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalFact {
    /// The stable signal id, such as `last_release`.
    pub name: String,
    /// The signal name shown to readers.
    pub label: String,
    /// The raw value as text, with float noise removed from a weighted amount.
    /// `None` when the check did not measure it.
    pub value: Option<String>,
    /// Where the value comes from. `None` when there is no value.
    pub source: Option<SourceRef>,
    /// The date of the data behind the value. `None` when there is no value.
    #[serde(serialize_with = "plain_option")]
    pub fetched: Option<Date>,
    /// Whether a tier rule reads the value. False for a display-only fact, such as commits.
    pub counts_toward_tier: bool,
}

impl SignalFact {
    /// The name a page prints. A report written before a label changed keeps its old text, so
    /// the four adoption signals always read their current label.
    pub fn shown_label(&self) -> &str {
        match self.name.as_str() {
            ACTIVE_USERS_SIGNAL => ACTIVE_DEPENDENTS_LABEL,
            NEW_ADOPTERS_SIGNAL => NEW_DEPENDENTS_LABEL,
            PEAK_USERS_SIGNAL => PEAK_DEPENDENTS_LABEL,
            FORMER_GLORY_SIGNAL => FORMER_GLORY_LABEL,
            _ => &self.label,
        }
    }

    /// Every provenance rule this fact breaks, one line each.
    ///
    /// - A value needs a source and a fetch date.
    /// - A source link uses `https`.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.value.is_some() {
            if self.source.is_none() {
                out.push(format!("signal `{}` has a value but no source", self.name));
            }
            if self.fetched.is_none() {
                out.push(format!(
                    "signal `{}` has a value but no fetch date",
                    self.name
                ));
            }
        }
        if let Some(source) = &self.source
            && source.url.scheme() != "https"
        {
            out.push(format!(
                "signal `{}` cites `{}`: a source link must use https",
                self.name, source.url
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_required_binding_to_an_ml_engine_counts_as_a_wrapper() {
        for wrapper in [
            "Binds native code: ort-sys, libz-sys (feature x)",
            "Binds native code: llama-cpp-sys-2. Build uses a native toolchain: cc",
            "Binds native code: diffusion-rs-sys, stable-diffusion",
            "Binds native code: whisper-rs-sys",
        ] {
            assert!(wraps_native_engine(wrapper), "{wrapper}");
        }
        for other in [
            "Binds native code: ring. Build uses a native toolchain: pkg-config",
            "Binds native code: blas-sys, lapack-sys",
            "Binds native code: transport-sys, zstd-sys",
            "Binds native code: torch-sys (feature tch)",
            "Build uses a native toolchain: cc",
            "None found",
        ] {
            assert!(!wraps_native_engine(other), "{other}");
        }
    }

    fn fact(value: Option<&str>, url: Option<&str>, fetched: bool) -> SignalFact {
        SignalFact {
            name: "last_release".to_owned(),
            label: "Latest release".to_owned(),
            value: value.map(str::to_owned),
            source: url.map(|url| SourceRef::new("dump", Url::parse(url).expect("url"))),
            fetched: fetched.then(|| Date::new(2026, 11, 2).expect("date")),
            counts_toward_tier: true,
        }
    }

    #[test]
    fn an_adoption_signal_reads_its_current_label_and_any_other_keeps_its_own() {
        let mut stale = fact(None, None, false);
        assert_eq!(stale.shown_label(), "Latest release");
        stale.name = ACTIVE_USERS_SIGNAL.to_owned();
        stale.label = "Active users".to_owned();
        assert_eq!(stale.shown_label(), "Active dependents");
        stale.name = NEW_ADOPTERS_SIGNAL.to_owned();
        assert_eq!(stale.shown_label(), "New dependents (365 days)");
    }

    #[test]
    fn the_repository_route_facts_count_and_commits_do_not() {
        for name in [STARS_SIGNAL, FORKS_SIGNAL, CONTRIBUTORS_SIGNAL] {
            assert!(!never_counts(name), "{name}");
        }
        assert!(never_counts(COMMITS_SIGNAL));
        assert!(never_counts(LICENSE_SIGNAL));
    }

    #[test]
    fn a_cited_dated_value_and_an_empty_fact_pass() {
        let full = fact(
            Some("2026-09-01"),
            Some("https://crates.io/data-access"),
            true,
        );
        assert!(full.problems().is_empty());
        assert!(fact(None, None, false).problems().is_empty());
    }

    #[test]
    fn a_value_needs_a_source_and_a_date() {
        let bare = fact(Some("1"), None, false).problems();
        assert_eq!(
            bare,
            [
                "signal `last_release` has a value but no source",
                "signal `last_release` has a value but no fetch date",
            ]
        );
    }

    #[test]
    fn a_source_link_needs_https() {
        let plain = fact(Some("1"), Some("http://crates.io/data-access"), true).problems();
        assert_eq!(
            plain,
            [
                "signal `last_release` cites `http://crates.io/data-access`: a source link must use https"
            ]
        );
    }
}
