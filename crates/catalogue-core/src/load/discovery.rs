use super::diagnostics::Diagnostics;
use super::parse::Parsed;
use crate::registry::Discovery;

/// Checks the discovery rules.
pub(crate) fn check_discovery(file: &Parsed<Discovery>, diags: &mut Diagnostics) {
    for problem in file.value.problems() {
        diags.push(&file.path, problem);
    }
}

#[cfg(test)]
mod tests {
    use crate::catalogue::Catalogue;
    use crate::testing::{errors, expect_error, put, valid};

    #[test]
    fn missing_file_is_not_an_error() {
        let catalogue = Catalogue::load(valid()).expect("valid data");
        assert!(catalogue.discovery().is_none());
    }

    #[test]
    fn valid_file_loads_and_is_exposed() {
        let mut src = valid();
        put(
            &mut src,
            "discovery.toml",
            "categories = [\"science::ml\"]\ncore_crates = [\"burn\"]\n",
        );
        let catalogue = Catalogue::load(src).expect("valid data");
        let rules = catalogue.discovery().expect("present");
        assert_eq!(rules.categories, ["science::ml"]);
        assert!(rules.keywords.is_empty());
        assert_eq!(rules.core_crates.len(), 1);
    }

    #[test]
    fn rule_breaks_name_the_file() {
        let mut src = valid();
        put(&mut src, "discovery.toml", "keywords = [\"LLM\"]\n");
        expect_error(src, "discovery.toml: keyword `LLM` must be lowercase");
    }

    #[test]
    fn empty_file_is_reported() {
        let mut src = valid();
        put(&mut src, "discovery.toml", "");
        expect_error(
            src,
            "discovery.toml: categories, keywords, core_crates, named_crates and github_topics",
        );
    }

    #[test]
    fn parse_errors_name_the_file_once() {
        let mut src = valid();
        put(&mut src, "discovery.toml", "bogus = 1\n");
        let found = errors(src);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("discovery.toml: line 1:"), "{found:?}");
    }
}
