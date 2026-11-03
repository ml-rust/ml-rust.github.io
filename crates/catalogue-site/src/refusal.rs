//! The checks that run before any page renders.

use catalogue_core::REPORT_SCHEMA;

use crate::error::SiteError;
use crate::inputs::SiteInputs;

/// Checks that `inputs` can publish tiers.
///
/// Without a report every check passes, and no page shows a tier.
///
/// # Errors
///
/// With a report, returns the first of these that holds:
/// - [`SiteError::NoThresholds`]: the catalogue holds no thresholds.
/// - [`SiteError::Uncalibrated`]: the thresholds carry no `calibrated` date.
/// - [`SiteError::ReportSchema`]: the report has another schema.
/// - [`SiteError::StaleReport`]: the report applied another thresholds version.
/// - [`SiteError::ReportMismatch`]: the report and the catalogue hold different slugs.
pub(crate) fn check(inputs: &SiteInputs<'_>) -> Result<(), SiteError> {
    let Some(report) = inputs.report else {
        return Ok(());
    };
    let thresholds = inputs
        .catalogue
        .thresholds()
        .ok_or(SiteError::NoThresholds)?;
    if thresholds.calibrated.is_none() {
        return Err(SiteError::Uncalibrated);
    }
    if report.schema != REPORT_SCHEMA {
        return Err(SiteError::ReportSchema {
            found: report.schema,
            expected: REPORT_SCHEMA,
        });
    }
    if report.thresholds_version != thresholds.version {
        return Err(SiteError::StaleReport {
            report: report.thresholds_version,
            data: thresholds.version,
        });
    }
    report.check(inputs.catalogue)?;
    Ok(())
}
