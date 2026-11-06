//! Movement as a plain word on a row, and the line pages show without enough history.

use catalogue_core::{Movement, MovementKind, Slug};

use crate::inputs::SiteInputs;

/// The line a page shows when the history cannot measure movement.
const NEEDS_HISTORY: &str = "Movement needs more history.";

/// The word of `kind`.
fn word(kind: MovementKind) -> &'static str {
    match kind {
        MovementKind::New => "Recently added",
        MovementKind::Rising => "Rising",
        MovementKind::Falling => "Falling",
    }
}

/// The word for entry `slug`. `None` without a report, or when the entry did not move.
pub(crate) fn movement_word(inputs: &SiteInputs<'_>, slug: &Slug) -> Option<&'static str> {
    inputs.report?;
    inputs.movement.kind_of(slug).map(word)
}

/// The line a page shows when tiers are published and the history cannot measure movement.
pub(crate) fn movement_note(inputs: &SiteInputs<'_>) -> Option<&'static str> {
    (inputs.report.is_some() && matches!(inputs.movement, Movement::NeedsHistory))
        .then_some(NEEDS_HISTORY)
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Date, FirstSeen, MeasuredMovement, Tier};

    use super::*;
    use crate::testing::{catalogue, entry, record, report, slug};

    fn fresh() -> Movement {
        let date = Date::new(2026, 11, 3).expect("date");
        Movement::Measured(MeasuredMovement {
            newest: date,
            baseline: date,
            new: vec![FirstSeen {
                slug: slug("alpha"),
                date,
            }],
            rising: Vec::new(),
            falling: Vec::new(),
        })
    }

    #[test]
    fn a_row_gets_a_word_only_with_a_report() {
        let data = catalogue(vec![entry("alpha", "")]);
        let tiers = report(vec![("alpha", record(Tier::Listed, None))]);
        let movement = fresh();
        let inputs = |report| SiteInputs {
            catalogue: &data,
            report,
            changes: &[],
            first_check: None,
            movement: &movement,
            state: None,
        };
        assert_eq!(
            movement_word(&inputs(Some(&tiers)), &slug("alpha")),
            Some("Recently added")
        );
        assert_eq!(movement_word(&inputs(None), &slug("alpha")), None);
        assert_eq!(movement_note(&inputs(Some(&tiers))), None);
    }

    #[test]
    fn the_note_shows_only_with_tiers_and_too_little_history() {
        let data = catalogue(vec![entry("alpha", "")]);
        let tiers = report(vec![("alpha", record(Tier::Listed, None))]);
        let movement = Movement::NeedsHistory;
        let inputs = |report| SiteInputs {
            catalogue: &data,
            report,
            changes: &[],
            first_check: None,
            movement: &movement,
            state: None,
        };
        assert_eq!(movement_note(&inputs(Some(&tiers))), Some(NEEDS_HISTORY));
        assert_eq!(movement_note(&inputs(None)), None);
        assert_eq!(movement_word(&inputs(Some(&tiers)), &slug("alpha")), None);
    }
}
