//! The description rule.

/// Phrases that mark a crate as abandoned when its description says them of itself.
const DESCRIPTION_PHRASES: [&str; 6] = [
    "no longer maintained",
    "temporary fork",
    "unmaintained",
    "deprecated",
    "unfinished",
    "do not use",
];

/// Leading words that make a clause refer to the crate itself.
const SELF_PREFIXES: [&str; 13] = [
    "this crate is ",
    "this project is ",
    "this library is ",
    "this package is ",
    "this repository is ",
    "this repo is ",
    "this is ",
    "it is ",
    "now ",
    "currently ",
    "officially ",
    "a ",
    "an ",
];

/// Words accepted after a description phrase in a self-referring clause.
const CONTINUATIONS: [&str; 13] = [
    "in favor of",
    "in favour of",
    "use",
    "please",
    "see",
    "of",
    "since",
    "as of",
    "for now",
    "fork",
    "this",
    "it",
    "yet",
];

/// The first clause of `text` that marks the crate itself as deprecated.
pub(super) fn find(text: &str) -> Option<&str> {
    clauses(text).find(|clause| clause_refers_to_self(clause))
}

/// Splits `text` into clauses at punctuation and dash separators.
fn clauses(text: &str) -> impl Iterator<Item = &str> {
    text.split(" - ").flat_map(|part| {
        part.split(|c: char| {
            matches!(
                c,
                '.' | ',' | ';' | ':' | '!' | '?' | '(' | ')' | '[' | ']' | '|' | '\n' | '–' | '—'
            )
        })
    })
}

fn clause_refers_to_self(clause: &str) -> bool {
    let lower = clause
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    let mut rest = lower.as_str();
    while let Some(stripped) = SELF_PREFIXES.iter().find_map(|p| rest.strip_prefix(p)) {
        rest = stripped.trim_start();
    }
    DESCRIPTION_PHRASES.iter().any(|phrase| {
        let Some(after) = rest.strip_prefix(phrase) else {
            return false;
        };
        if after.is_empty() {
            return true;
        }
        let Some(after) = after.strip_prefix(' ') else {
            return false;
        };
        let after = after.trim_start();
        CONTINUATIONS.iter().any(|word| {
            after
                .strip_prefix(word)
                .is_some_and(|tail| tail.is_empty() || tail.starts_with(' '))
        })
    })
}

#[cfg(test)]
mod tests {
    use crate::deprecation::{Field, scan};

    fn description(text: &str) -> Option<String> {
        scan(Some(text), None).map(|notice| {
            assert_eq!(notice.field, Field::Description);
            notice.phrase
        })
    }

    #[test]
    fn description_phrase_alone_matches() {
        for text in [
            "DEPRECATED",
            "Deprecated.",
            "Unmaintained",
            "No longer maintained!",
            "Unfinished",
            "Do not use",
            "⚠️ deprecated ⚠️",
        ] {
            assert!(description(text).is_some(), "{text}");
        }
    }

    #[test]
    fn description_clause_after_other_text_matches() {
        assert_eq!(
            description("Fast JSON parser. Deprecated, use serde_json").as_deref(),
            Some("Deprecated")
        );
        assert_eq!(
            description("A parser (unmaintained)").as_deref(),
            Some("unmaintained")
        );
        assert_eq!(
            description("Old bindings - no longer maintained").as_deref(),
            Some("no longer maintained")
        );
    }

    #[test]
    fn description_self_reference_matches() {
        for text in [
            "This crate is deprecated",
            "This project is no longer maintained, see foo",
            "This library is now unmaintained",
            "It is officially deprecated",
            "An unfinished fork of foo",
            "A temporary fork of foo",
        ] {
            assert!(description(text).is_some(), "{text}");
        }
    }

    #[test]
    fn description_continuations_match() {
        for text in [
            "Deprecated in favor of bar",
            "Deprecated in favour of bar",
            "Deprecated since 0.3",
            "Deprecated as of 2024",
            "Unmaintained fork of foo",
            "Temporary fork of foo until upstream merges",
            "Do not use this crate",
            "Do not use it",
            "Do not use yet",
            "Unfinished for now",
        ] {
            assert!(description(text).is_some(), "{text}");
        }
    }

    #[test]
    fn description_phrase_about_something_else_does_not_match() {
        for text in [
            "deprecated APIs are supported",
            "replaces the deprecated foo crate",
            "Migrates code away from deprecated functions",
            "Detects unmaintained dependencies",
            "Do not use unsafe code in your parser",
            "Finds no longer maintained crates in a lock file",
            "A non-deprecated builder",
            "Lists unfinished tasks",
        ] {
            assert_eq!(description(text), None, "{text}");
        }
    }
}
