use super::diagnostics::Diagnostics;
use super::parse::parse_file;
use super::sources::SourceFile;
use crate::registry::Deprecated;

/// Parses and checks `deprecated.toml`.
///
/// Returns `None` when the file breaks a rule. Every break goes to `diags`.
pub(crate) fn check_deprecated(file: &SourceFile, diags: &mut Diagnostics) -> Option<Deprecated> {
    let value = parse_file::<Deprecated>(file, diags)?;
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
    use crate::testing::{errors, expect_error, put, valid};

    const LIST: &str = "[[requirement]]\ncrate = \"tokio\"\nbelow = \"1.0.0\"\nreason = \"r\"\nsource = \"https://example.com/r\"\n";

    #[test]
    fn missing_file_is_not_an_error() {
        let catalogue = Catalogue::load(valid()).expect("valid data");
        assert!(catalogue.deprecated().is_none());
    }

    #[test]
    fn valid_file_loads_and_is_exposed() {
        let mut src = valid();
        put(&mut src, "deprecated.toml", LIST);
        let catalogue = Catalogue::load(src).expect("valid data");
        let list = catalogue.deprecated().expect("present");
        assert_eq!(list.requirement.len(), 1);
    }

    #[test]
    fn rule_breaks_name_the_file() {
        let mut src = valid();
        put(&mut src, "deprecated.toml", &LIST.replace("1.0.0", "1.0"));
        expect_error(src, "deprecated.toml: requirement 1: below `1.0`");
    }

    #[test]
    fn parse_errors_name_the_file_once() {
        let mut src = valid();
        put(
            &mut src,
            "deprecated.toml",
            "[[requirement]]\ncrate = \"tokio\"\n",
        );
        let found = errors(src);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("deprecated.toml: "), "{found:?}");
    }
}
