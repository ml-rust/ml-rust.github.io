//! Which entries moved over a window of checks.
//!
//! The history measures it and the site shows it. No tier rule reads it.

use crate::date::Date;
use crate::ids::Slug;

/// How an entry moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementKind {
    /// First seen inside the window.
    New,
    /// Active dependents rose by at least the set share and number.
    Rising,
    /// Active dependents fell by at least the set share and number.
    Falling,
}

/// An entry first seen inside the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstSeen {
    /// The entry slug.
    pub slug: Slug,
    /// The date of the oldest snapshot that holds the entry.
    pub date: Date,
}

/// An entry whose active dependents changed enough to count.
#[derive(Debug, Clone, PartialEq)]
pub struct UsersChange {
    /// The entry slug.
    pub slug: Slug,
    /// The active dependents in the baseline snapshot.
    pub from: f64,
    /// The active dependents in the newest snapshot.
    pub to: f64,
}

/// Movement between two snapshots. Each list sorts by slug.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasuredMovement {
    /// The date of the newest snapshot.
    pub newest: Date,
    /// The date of the snapshot nearest the window start.
    pub baseline: Date,
    /// The entries first seen inside the window.
    pub new: Vec<FirstSeen>,
    /// The entries whose active dependents rose.
    pub rising: Vec<UsersChange>,
    /// The entries whose active dependents fell.
    pub falling: Vec<UsersChange>,
}

/// What a history says about movement.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Movement {
    /// The history cannot measure the window, so no entry has movement.
    #[default]
    NeedsHistory,
    /// The history measures the window.
    Measured(MeasuredMovement),
}

impl Movement {
    /// How entry `slug` moved. `None` when it did not, or when the history cannot say.
    ///
    /// An entry both new and changed reads as new.
    pub fn kind_of(&self, slug: &Slug) -> Option<MovementKind> {
        let Self::Measured(measured) = self else {
            return None;
        };
        if measured.new.iter().any(|found| &found.slug == slug) {
            return Some(MovementKind::New);
        }
        if measured.rising.iter().any(|change| &change.slug == slug) {
            return Some(MovementKind::Rising);
        }
        if measured.falling.iter().any(|change| &change.slug == slug) {
            return Some(MovementKind::Falling);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slug(text: &str) -> Slug {
        Slug::new(text).expect("slug")
    }

    fn change(name: &str) -> UsersChange {
        UsersChange {
            slug: slug(name),
            from: 1.0,
            to: 2.0,
        }
    }

    #[test]
    fn kind_of_names_each_list_and_new_comes_first() {
        let date = Date::new(2026, 11, 3).expect("date");
        let movement = Movement::Measured(MeasuredMovement {
            newest: date,
            baseline: date,
            new: vec![FirstSeen {
                slug: slug("fresh"),
                date,
            }],
            rising: vec![change("up"), change("fresh")],
            falling: vec![change("down")],
        });
        assert_eq!(movement.kind_of(&slug("fresh")), Some(MovementKind::New));
        assert_eq!(movement.kind_of(&slug("up")), Some(MovementKind::Rising));
        assert_eq!(movement.kind_of(&slug("down")), Some(MovementKind::Falling));
        assert_eq!(movement.kind_of(&slug("still")), None);
    }

    #[test]
    fn a_history_that_cannot_measure_gives_no_kind() {
        assert_eq!(Movement::default().kind_of(&slug("any")), None);
    }
}
