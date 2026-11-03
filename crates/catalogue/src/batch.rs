//! The per-item loop shared by the commands that look up several names.

use catalogue_signals::http::{Headers, Pacer};

/// Looks up every item in order and returns the results in the same order.
///
/// The pacer waits before every item except the first. The loop stops at the
/// first error and returns it.
pub(crate) fn each<I, T, E>(
    items: &[I],
    pacer: &mut dyn Pacer,
    mut lookup: impl FnMut(&I, &mut dyn Pacer) -> Result<T, E>,
) -> Result<Vec<T>, E> {
    let mut results = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            pacer.wait(&Headers::default());
        }
        results.push(lookup(item, pacer)?);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Counts the waits.
    #[derive(Default)]
    struct Counting {
        waits: usize,
    }

    impl Pacer for Counting {
        fn wait(&mut self, _response_headers: &Headers) {
            self.waits += 1;
        }
    }

    #[test]
    fn waits_between_items_only() {
        let mut pacer = Counting::default();
        let mut seen_waits = Vec::new();
        let results = each(&[1, 2, 3], &mut pacer, |item, _| {
            seen_waits.push(*item);
            Ok::<_, ()>(item * 2)
        })
        .expect("no error");
        assert_eq!(results, [2, 4, 6]);
        assert_eq!(pacer.waits, 2);
        assert_eq!(seen_waits, [1, 2, 3]);
    }

    #[test]
    fn an_empty_list_never_waits() {
        let mut pacer = Counting::default();
        let results = each(&[] as &[u8], &mut pacer, |_, _| Ok::<u8, ()>(0)).expect("no error");
        assert!(results.is_empty());
        assert_eq!(pacer.waits, 0);
    }

    #[test]
    fn stops_at_the_first_error() {
        let mut pacer = Counting::default();
        let mut calls = 0;
        let error = each(&[1, 2, 3], &mut pacer, |item, _| {
            calls += 1;
            if *item == 2 { Err("two") } else { Ok(*item) }
        })
        .expect_err("second item fails");
        assert_eq!(error, "two");
        assert_eq!(calls, 2);
        assert_eq!(pacer.waits, 1);
    }
}
