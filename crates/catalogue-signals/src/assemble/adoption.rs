//! The adoption signals of a library family.

use catalogue_core::Signals;

use crate::dump::IdMap;
use crate::usage::Usage;

/// Sets the adoption signals from the family crate with the highest current users.
///
/// `family` holds crate ids in entry order. A tie goes to the earlier crate.
pub(super) fn adoption(family: &[u32], usage: &IdMap<Usage>, signals: &mut Signals) {
    let mut best: Option<&Usage> = None;
    for found in family.iter().filter_map(|id| usage.get(id)) {
        if best.is_none_or(|current| found.current > current.current) {
            best = Some(found);
        }
    }
    if let Some(best) = best {
        signals.active_users = Some(best.current);
        signals.new_adopters = Some(best.new_adopters);
        signals.peak_users = Some(best.peak);
        signals.former_glory = Some(best.former_glory);
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::EntryKind;

    use super::*;

    fn usage(current: f64, peak: f64) -> Usage {
        Usage {
            current,
            peak,
            former_glory: 0.25,
            growth: 1.0,
            new_adopters: current / 2.0,
            removed: 0,
            expired: 0,
            history: Vec::new(),
        }
    }

    fn adopted(family: &[u32], usage: &IdMap<Usage>) -> Signals {
        let mut signals = Signals::unknown(EntryKind::Library);
        adoption(family, usage, &mut signals);
        signals
    }

    #[test]
    fn the_most_used_family_crate_supplies_every_adoption_signal() {
        let found: IdMap<Usage> = [(1, usage(3.0, 4.0)), (2, usage(5.0, 9.0))]
            .into_iter()
            .collect();
        let signals = adopted(&[1, 2], &found);
        assert_eq!(signals.active_users, Some(5.0));
        assert_eq!(signals.peak_users, Some(9.0));
        assert_eq!(signals.new_adopters, Some(2.5));
        assert_eq!(signals.former_glory, Some(0.25));
    }

    #[test]
    fn a_tie_goes_to_the_crate_listed_first() {
        let found: IdMap<Usage> = [(1, usage(3.0, 4.0)), (2, usage(3.0, 6.0))]
            .into_iter()
            .collect();
        assert_eq!(adopted(&[1, 2], &found).peak_users, Some(4.0));
        assert_eq!(adopted(&[2, 1], &found).peak_users, Some(6.0));
    }

    #[test]
    fn no_usage_leaves_adoption_unknown() {
        let signals = adopted(&[1], &IdMap::default());
        assert_eq!(signals.active_users, None);
        assert_eq!(signals.former_glory, None);
    }
}
