//! Why a site build stops.

use catalogue_core::ReportMismatch;
use thiserror::Error;

/// Why [`crate::build_site`] wrote no file.
#[derive(Debug, Error)]
pub enum SiteError {
    /// A tier report is given, but the catalogue holds no thresholds.
    #[error(
        "a tier report needs data/thresholds.toml, and the catalogue holds none. Add the file, or build without tiers"
    )]
    NoThresholds,
    /// A tier report is given, but the thresholds carry no calibration date.
    #[error(
        "data/thresholds.toml has no `calibrated` date, so tiers are not published. Set `calibrated` after calibration, or build without tiers"
    )]
    Uncalibrated,
    /// The report was written in another report format.
    #[error(
        "the tier report has schema {found}, and this build reads schema {expected}. Rerun `catalogue tiers --json` on this data"
    )]
    ReportSchema {
        /// The `schema` of the report.
        found: u32,
        /// [`catalogue_core::REPORT_SCHEMA`].
        expected: u32,
    },
    /// The report applied another version of the thresholds.
    #[error(
        "the tier report applied thresholds version {report}, and data/thresholds.toml is version {data}. Rerun `catalogue tiers --json` on this data"
    )]
    StaleReport {
        /// The `thresholds_version` of the report.
        report: u32,
        /// The `version` of `data/thresholds.toml`.
        data: u32,
    },
    /// The report and the catalogue hold different slugs.
    #[error(transparent)]
    ReportMismatch(#[from] ReportMismatch),
    /// The search index failed to encode.
    #[error("encoding the search index failed: {0}")]
    Search(#[source] oxidoc_search::SearchError),
    /// A data file failed to encode.
    #[error(
        "encoding {path} failed: {source}. Check the report and the data files, then rerun `catalogue build`"
    )]
    Encode {
        /// The output path of the file.
        path: &'static str,
        /// The JSON error.
        #[source]
        source: serde_json::Error,
    },
    /// A template failed to render.
    #[error("rendering {path} failed: {source}")]
    Render {
        /// The output path of the page.
        path: String,
        /// The template error.
        #[source]
        source: askama::Error,
    },
}
