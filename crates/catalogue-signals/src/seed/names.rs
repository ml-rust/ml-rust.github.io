//! The description of a family draft.

use catalogue_core::description_text;

use crate::dump::DumpSnapshot;

/// The first usable description among `members` and the name of its crate.
///
/// Errors with the primary's reason when no member has one.
pub(super) fn description_of<'a>(
    snapshot: &'a DumpSnapshot,
    members: &[(u32, &str)],
) -> Result<(String, &'a str), String> {
    let mut first_error = None;
    for (id, _) in members {
        let Some(row) = snapshot.crates.get(id) else {
            continue;
        };
        match description_text(row.description.as_deref()) {
            Ok(text) => return Ok((text, row.name.as_str())),
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    Err(first_error.unwrap_or_else(|| "no description".to_owned()))
}
