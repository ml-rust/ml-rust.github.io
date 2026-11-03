use super::diagnostics::Diagnostics;
use super::parse::parse_file;
use super::sources::SourceFile;
use crate::registry::Thresholds;

/// Parses and checks `thresholds.toml`.
///
/// Returns `None` when the file breaks a rule. Every break goes to `diags`.
pub(crate) fn check_thresholds(file: &SourceFile, diags: &mut Diagnostics) -> Option<Thresholds> {
    let value = parse_file::<Thresholds>(file, diags)?;
    let problems = value.problems();
    if problems.is_empty() {
        return Some(value);
    }
    for problem in problems {
        diags.push(&file.path, problem);
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::catalogue::Catalogue;
    use crate::testing::{THRESHOLDS, errors, expect_error, put, valid};

    #[test]
    fn missing_file_is_not_an_error() {
        let catalogue = Catalogue::load(valid()).expect("valid data");
        assert!(catalogue.thresholds().is_none());
    }

    #[test]
    fn valid_file_loads_and_is_exposed() {
        let mut src = valid();
        put(&mut src, "thresholds.toml", THRESHOLDS);
        let catalogue = Catalogue::load(src).expect("valid data");
        let thresholds = catalogue.thresholds().expect("present");
        assert_eq!(thresholds.dormancy.min_days, 365);
    }

    #[test]
    fn rule_breaks_name_the_file() {
        let mut src = valid();
        put(
            &mut src,
            "thresholds.toml",
            &THRESHOLDS.replace("dead_consecutive_checks = 2", "dead_consecutive_checks = 0"),
        );
        expect_error(src, "thresholds.toml: dormancy.dead_consecutive_checks = 0");
    }

    #[test]
    fn parse_errors_name_the_file_once() {
        let mut src = valid();
        put(&mut src, "thresholds.toml", "version = 1\n");
        let found = errors(src);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("thresholds.toml: "), "{found:?}");
    }
}
