//! The license of a project as an SPDX expression, or the plain statement that it has none.

use catalogue_core::NONE_STATED;

use crate::github::RepoLicense;

/// Values that hold a license nobody can read from the text. Compared without regard to case.
///
/// crates.io writes `non-standard` for a crate that names a license file. GitHub writes
/// `NOASSERTION` for a license file it cannot match.
const UNREADABLE: [&str; 3] = ["NOASSERTION", "other", "non-standard"];

/// Where a license value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseOrigin {
    /// The `license` column of the primary crate's default version.
    Dump,
    /// The license GitHub states for the repo.
    Repo,
}

/// A license value with its origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Licensed {
    /// An SPDX expression or the statement that there is none.
    pub text: String,
    /// Where the value comes from.
    pub origin: LicenseOrigin,
}

/// The license value of the `license` column. `None` when the value cannot be read.
///
/// An empty column states no license. The old `/` form becomes `OR`, and an expression of `OR`
/// alone sorts its names, so one license reads one way in the search filter.
pub(super) fn from_dump(column: Option<&str>) -> Option<Licensed> {
    let text = match column.map(str::trim).filter(|text| !text.is_empty()) {
        None => NONE_STATED.to_owned(),
        Some(text) if is_unreadable(text) => return None,
        Some(text) => normalized(text),
    };
    Some(Licensed {
        text,
        origin: LicenseOrigin::Dump,
    })
}

/// The license value GitHub states. `None` when the repo has no readable one.
pub(super) fn from_repo(license: &RepoLicense) -> Option<Licensed> {
    let text = match license {
        RepoLicense::Stated(text) if is_unreadable(text) => return None,
        RepoLicense::Stated(text) => text.clone(),
        RepoLicense::NoneStated => NONE_STATED.to_owned(),
        RepoLicense::NotMeasured => return None,
    };
    Some(Licensed {
        text,
        origin: LicenseOrigin::Repo,
    })
}

fn is_unreadable(text: &str) -> bool {
    UNREADABLE
        .iter()
        .any(|value| text.eq_ignore_ascii_case(value))
}

/// `text` with the `/` form as `OR`, and the names of an `OR` expression in sorted order.
fn normalized(text: &str) -> String {
    let spelled: Vec<&str> = text.split('/').map(str::trim).collect();
    let spelled = spelled.join(" OR ");
    let compound = [" AND ", " WITH ", "("]
        .iter()
        .any(|part| spelled.contains(part));
    if compound {
        return spelled;
    }
    let mut names: Vec<&str> = spelled.split(" OR ").map(str::trim).collect();
    names.sort_unstable();
    names.dedup();
    names.join(" OR ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dump_text(column: Option<&str>) -> Option<String> {
        from_dump(column).map(|found| found.text)
    }

    #[test]
    fn an_expression_keeps_its_meaning_and_reads_one_way() {
        for (column, expected) in [
            ("MIT", "MIT"),
            ("MIT OR Apache-2.0", "Apache-2.0 OR MIT"),
            ("Apache-2.0 OR MIT", "Apache-2.0 OR MIT"),
            ("MIT/Apache-2.0", "Apache-2.0 OR MIT"),
            ("MIT / Apache-2.0", "Apache-2.0 OR MIT"),
            (
                "Apache-2.0 WITH LLVM-exception",
                "Apache-2.0 WITH LLVM-exception",
            ),
            (
                "(MIT OR Apache-2.0) AND BSD-3-Clause",
                "(MIT OR Apache-2.0) AND BSD-3-Clause",
            ),
        ] {
            assert_eq!(
                dump_text(Some(column)).as_deref(),
                Some(expected),
                "{column}"
            );
        }
    }

    #[test]
    fn no_value_is_a_plain_statement() {
        assert_eq!(dump_text(None).as_deref(), Some("None stated"));
        assert_eq!(dump_text(Some(" ")).as_deref(), Some("None stated"));
    }

    #[test]
    fn a_value_nobody_can_read_is_not_measured() {
        for column in ["non-standard", "NOASSERTION", "noassertion", "other"] {
            assert_eq!(dump_text(Some(column)), None, "{column}");
        }
    }

    #[test]
    fn a_repo_license_keeps_its_origin() {
        let stated = from_repo(&RepoLicense::Stated("MIT".to_owned())).expect("stated");
        assert_eq!(stated.text, "MIT");
        assert_eq!(stated.origin, LicenseOrigin::Repo);
        let none = from_repo(&RepoLicense::NoneStated).expect("none stated");
        assert_eq!(none.text, "None stated");
        assert_eq!(from_repo(&RepoLicense::NotMeasured), None);
        assert_eq!(
            from_repo(&RepoLicense::Stated("NOASSERTION".to_owned())),
            None
        );
    }
}
