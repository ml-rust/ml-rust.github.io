//! Reads the install analytics of a Homebrew formula.

use std::collections::HashMap;

use serde::Deserialize;

use super::error::DistributionError;
use super::json::parse_json;
use crate::http::{Paced, Pacer, Transport, percent_encode};

/// The API name used in errors.
const API: &str = "Homebrew";

/// The formula endpoint of the Homebrew API.
const ENDPOINT: &str = "https://formulae.brew.sh/api/formula";

/// Install counts per formula key, such as `ripgrep` and `ripgrep --HEAD`.
type Counts = HashMap<String, u64>;

/// The counts of one analytics category by window.
#[derive(Deserialize, Default)]
struct Windows {
    #[serde(rename = "30d", default)]
    d30: Counts,
    #[serde(rename = "90d", default)]
    d90: Counts,
    #[serde(rename = "365d", default)]
    d365: Counts,
}

/// The analytics categories the lookup reads.
#[derive(Deserialize, Default)]
struct Analytics {
    #[serde(default)]
    install: Windows,
    #[serde(default)]
    install_on_request: Windows,
}

/// The formula fields the lookup reads. Unknown fields are ignored.
#[derive(Deserialize)]
struct Formula {
    #[serde(default)]
    analytics: Option<Analytics>,
}

/// The install counts of a Homebrew formula.
///
/// Each count sums every key of its window, so `name --HEAD` installs count.
/// A formula with no analytics gives zeros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HomebrewInstalls {
    /// Installs in the last 30 days.
    pub install_30d: u64,
    /// Installs in the last 90 days.
    pub install_90d: u64,
    /// Installs in the last 365 days.
    pub install_365d: u64,
    /// Installs a user asked for, in the last 30 days.
    pub on_request_30d: u64,
    /// Installs a user asked for, in the last 90 days.
    pub on_request_90d: u64,
    /// Installs a user asked for, in the last 365 days.
    pub on_request_365d: u64,
}

/// Reads the install analytics of the Homebrew formula `name`.
///
/// Returns `None` when Homebrew does not know the formula.
///
/// # Errors
///
/// Returns a [`DistributionError`] when no response arrives, the status is not
/// 200 or 404, the body breaks off or is not the expected JSON, or the rate
/// limit stays after the retries.
pub fn homebrew_installs(
    transport: &dyn Transport,
    name: &str,
    pacer: &mut dyn Pacer,
) -> Result<Option<HomebrewInstalls>, DistributionError> {
    let url = format!("{ENDPOINT}/{}.json", percent_encode(name));
    let headers = [("Accept", "application/json".to_owned())];
    let mut paced = Paced::new(pacer);
    let Some(bytes) = paced
        .fetch_optional(&url, || transport.get(&url, &headers))
        .map_err(|error| DistributionError::paced(API, error))?
    else {
        return Ok(None);
    };
    let formula: Formula = parse_json(API, &url, &bytes)?;
    let analytics = formula.analytics.unwrap_or_default();
    Ok(Some(HomebrewInstalls {
        install_30d: sum(&analytics.install.d30),
        install_90d: sum(&analytics.install.d90),
        install_365d: sum(&analytics.install.d365),
        on_request_30d: sum(&analytics.install_on_request.d30),
        on_request_90d: sum(&analytics.install_on_request.d90),
        on_request_365d: sum(&analytics.install_on_request.d365),
    }))
}

/// The sum of every count, capped at the largest value.
fn sum(counts: &Counts) -> u64 {
    counts.values().fold(0, |total, n| total.saturating_add(*n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_is_summed_and_overflow_is_capped() {
        let counts = Counts::from([("a".to_owned(), 5), ("a --HEAD".to_owned(), 2)]);
        assert_eq!(sum(&counts), 7);
        let big = Counts::from([("a".to_owned(), u64::MAX), ("b".to_owned(), 1)]);
        assert_eq!(sum(&big), u64::MAX);
        assert_eq!(sum(&Counts::new()), 0);
    }
}
