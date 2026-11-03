//! The release-download samples of app repos.

use crate::distribution::ReleaseDownloads;

/// The `total_binary_downloads` of each repo that has at least one release.
///
/// A repo that GitHub does not know (`None`) or that lists no release adds no
/// sample. The sample order follows `downloads`.
pub fn app_samples(downloads: &[(String, Option<ReleaseDownloads>)]) -> Vec<f64> {
    downloads
        .iter()
        .filter_map(|(_, found)| found.as_ref())
        .filter(|found| !found.releases.is_empty())
        .map(|found| found.total_binary_downloads as f64)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distribution::ReleaseSummary;

    fn found(downloads: &[u64]) -> ReleaseDownloads {
        ReleaseDownloads {
            releases: downloads
                .iter()
                .enumerate()
                .map(|(index, &binary_downloads)| ReleaseSummary {
                    tag: format!("v{index}"),
                    published: None,
                    prerelease: false,
                    binary_downloads,
                })
                .collect(),
            total_binary_downloads: downloads.iter().sum(),
        }
    }

    #[test]
    fn only_repos_with_a_release_give_a_sample() {
        let downloads = vec![
            ("a/a".to_owned(), Some(found(&[10, 5]))),
            ("b/b".to_owned(), None),
            ("c/c".to_owned(), Some(found(&[]))),
            ("d/d".to_owned(), Some(found(&[0]))),
        ];
        assert_eq!(app_samples(&downloads), [15.0, 0.0]);
    }

    #[test]
    fn no_repos_give_no_samples() {
        assert!(app_samples(&[]).is_empty());
    }
}
