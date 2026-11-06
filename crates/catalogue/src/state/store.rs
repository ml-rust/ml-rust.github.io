//! The SQLite file that keeps the AI summaries, one row per project and release.
//!
//! The file moves as it is, so a fill on one machine can be copied to another.

use std::path::Path;

use catalogue_core::{Date, Slug};
use catalogue_state::SummaryStore;
use rusqlite::{Connection, OptionalExtension, params};

/// The table. Old releases stay, so a project keeps the history of its summaries.
const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS summary (
    slug TEXT NOT NULL,
    release TEXT NOT NULL,
    text TEXT NOT NULL,
    model TEXT NOT NULL,
    written TEXT NOT NULL,
    PRIMARY KEY (slug, release)
)";

/// The summaries in one SQLite file.
pub(super) struct SqliteStore {
    connection: Connection,
    today: Date,
}

impl SqliteStore {
    /// Opens the file at `path`, creating it when absent. `today` dates each new row.
    pub(super) fn open(path: &Path, today: Date) -> Result<Self, String> {
        let fail = |error: rusqlite::Error| {
            format!("cannot open the summaries at {}: {error}", path.display())
        };
        let connection = Connection::open(path).map_err(fail)?;
        connection.execute(SCHEMA, []).map_err(fail)?;
        Ok(Self { connection, today })
    }
}

impl SummaryStore for SqliteStore {
    fn get(&self, slug: &Slug, release: &str) -> Result<Option<String>, String> {
        self.connection
            .query_row(
                "SELECT text FROM summary WHERE slug = ?1 AND release = ?2",
                params![slug.as_str(), release],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("cannot read the summary of {slug}: {error}"))
    }

    fn put(&mut self, slug: &Slug, release: &str, text: &str, model: &str) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT OR REPLACE INTO summary (slug, release, text, model, written)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![slug.as_str(), release, text, model, self.today.to_string()],
            )
            .map(|_| ())
            .map_err(|error| format!("cannot keep the summary of {slug}: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_summary_is_kept_per_release_and_survives_a_reopen() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
        std::fs::create_dir_all(&base).expect("target dir");
        let dir = tempfile::tempdir_in(base).expect("scratch directory");
        let path = dir.path().join("summaries.db");
        let today = Date::new(2026, 11, 6).expect("date");
        let slug = Slug::new("tool".to_owned()).expect("slug");
        {
            let mut store = SqliteStore::open(&path, today).expect("open");
            assert_eq!(store.get(&slug, "v1").expect("get"), None);
            store.put(&slug, "v1", "Old card.", "model").expect("put");
            store.put(&slug, "v2", "New card.", "model").expect("put");
        }
        let store = SqliteStore::open(&path, today).expect("reopen");
        assert_eq!(
            store.get(&slug, "v1").expect("get").as_deref(),
            Some("Old card.")
        );
        assert_eq!(
            store.get(&slug, "v2").expect("get").as_deref(),
            Some("New card.")
        );
        assert_eq!(store.get(&slug, "v3").expect("get"), None);
    }
}
