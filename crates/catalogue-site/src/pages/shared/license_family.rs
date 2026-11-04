//! The license family of an SPDX expression, for a filter that groups licenses.

use catalogue_core::{NONE_STATED, NOT_MEASURED};

/// The family of a license that places no copyleft duty.
const PERMISSIVE: &str = "Permissive";
/// The family of a license whose copyleft covers the licensed files or library.
const WEAK_COPYLEFT: &str = "Weak copyleft";
/// The family of a license whose copyleft covers the whole combined work.
const STRONG_COPYLEFT: &str = "Strong copyleft";
/// The family of a license this table does not place. The page never guesses.
const OTHER: &str = "Other";

/// Exact ids of permissive licenses.
const PERMISSIVE_IDS: [&str; 15] = [
    "0BSD",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSL-1.0",
    "BlueOak-1.0.0",
    "CC0-1.0",
    "ISC",
    "MIT",
    "MIT-0",
    "PostgreSQL",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "Unlicense",
    "Zlib",
];

/// Id prefixes of weak copyleft licenses.
const WEAK_PREFIXES: [&str; 4] = ["CDDL-", "EPL-", "LGPL-", "MPL-"];

/// Id prefixes of strong copyleft licenses.
const STRONG_PREFIXES: [&str; 3] = ["AGPL-", "EUPL-", "GPL-"];

/// The place of a family from the least duty to the most. `None` for [`OTHER`].
fn duty(family: &str) -> Option<u8> {
    match family {
        PERMISSIVE => Some(0),
        WEAK_COPYLEFT => Some(1),
        STRONG_COPYLEFT => Some(2),
        _ => None,
    }
}

/// The family of one license id. An exception after `WITH` keeps the family of the license.
fn id_family(id: &str) -> &'static str {
    let name = id.split(" WITH ").next().unwrap_or(id).trim();
    if PERMISSIVE_IDS.contains(&name) {
        PERMISSIVE
    } else if WEAK_PREFIXES.iter().any(|prefix| name.starts_with(prefix)) {
        WEAK_COPYLEFT
    } else if STRONG_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        STRONG_COPYLEFT
    } else {
        OTHER
    }
}

/// The family of ids joined by `AND`: the reader takes every duty, so the heaviest decides.
fn all_of(part: &str) -> &'static str {
    let mut heaviest = PERMISSIVE;
    for id in part.split(" AND ") {
        let family = id_family(id);
        match (duty(family), duty(heaviest)) {
            (None, _) => return OTHER,
            (Some(found), Some(most)) if found > most => heaviest = family,
            _ => {}
        }
    }
    heaviest
}

/// The family of the license fact `value`.
///
/// `OR` gives the reader the choice, so the lightest choice decides. A value with parentheses, or
/// with an id the table does not place in every choice, reads as `Other`. A missing value and a
/// project that states no license each stay their own answer.
pub(crate) fn license_family(value: Option<&str>) -> &'static str {
    let Some(text) = value else {
        return NOT_MEASURED;
    };
    if text == NONE_STATED {
        return NONE_STATED;
    }
    if text.contains('(') {
        return OTHER;
    }
    text.split(" OR ")
        .map(all_of)
        .filter(|family| duty(family).is_some())
        .min_by_key(|family| duty(family))
        .unwrap_or(OTHER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_value_of_the_data_finds_its_family() {
        for (value, family) in [
            ("MIT", PERMISSIVE),
            ("Apache-2.0", PERMISSIVE),
            ("Apache-2.0 OR MIT", PERMISSIVE),
            ("Apache-2.0 WITH LLVM-exception", PERMISSIVE),
            ("MIT AND Apache-2.0", PERMISSIVE),
            ("Apache-2.0 OR EUPL-1.2 OR MIT", PERMISSIVE),
            ("Apache-2.0 OR BlueOak-1.0.0", PERMISSIVE),
            ("BSD-3-Clause OR MIT", PERMISSIVE),
            ("Unlicense", PERMISSIVE),
            ("PostgreSQL", PERMISSIVE),
            ("MPL-2.0", WEAK_COPYLEFT),
            ("LGPL-3.0", WEAK_COPYLEFT),
            ("LGPL-3.0-or-later", WEAK_COPYLEFT),
            ("EPL-2.0", WEAK_COPYLEFT),
            ("GPL-3.0", STRONG_COPYLEFT),
            ("GPL-3.0+", STRONG_COPYLEFT),
            ("AGPL-3.0", STRONG_COPYLEFT),
            ("AGPL-3.0-or-later", STRONG_COPYLEFT),
            ("EUPL-1.2", STRONG_COPYLEFT),
        ] {
            assert_eq!(license_family(Some(value)), family, "{value}");
        }
    }

    #[test]
    fn the_lightest_choice_and_the_heaviest_duty_decide() {
        assert_eq!(license_family(Some("GPL-3.0 OR MIT")), PERMISSIVE);
        assert_eq!(license_family(Some("GPL-3.0 AND MIT")), STRONG_COPYLEFT);
        assert_eq!(
            license_family(Some("MIT AND MPL-2.0 OR GPL-3.0")),
            WEAK_COPYLEFT
        );
    }

    #[test]
    fn a_value_the_table_cannot_place_reads_as_other() {
        for value in [
            "Custom",
            "BUSL-1.1",
            "MIT AND Custom",
            "(MIT OR Apache-2.0) AND GPL-3.0",
        ] {
            assert_eq!(license_family(Some(value)), OTHER, "{value}");
        }
    }

    #[test]
    fn a_missing_value_and_no_stated_license_are_their_own_answers() {
        assert_eq!(license_family(None), NOT_MEASURED);
        assert_eq!(license_family(Some(NONE_STATED)), NONE_STATED);
    }
}
