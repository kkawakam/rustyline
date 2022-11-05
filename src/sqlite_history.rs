//! History impl. based on SQLite
use std::borrow::Cow;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

use crate::history::SearchResult;
use crate::{Config, History, HistoryDuplicates, ReadlineError, Result, SearchDirection};

// FIXME history_index (src/lib.rs:686)

/// History stored in an SQLite database.
pub struct SQLiteHistory {
    max_len: usize,
    ignore_space: bool,
    ignore_dups: bool,
    path: Option<PathBuf>, // None => memory
    session_id: usize,     // 0 means no new entry added
    row_id: usize,         // max entry id
}

/*
https://sqlite.org/autoinc.html
If no ROWID is specified on the insert, or if the specified ROWID has a value of NULL, then an appropriate ROWID is created automatically.
The usual algorithm is to give the newly created row a ROWID that is one larger than the largest ROWID in the table prior to the insert.
If the table is initially empty, then a ROWID of 1 is used.
If the largest ROWID is equal to the largest possible integer (9223372036854775807) then the database engine starts picking positive candidate ROWIDs
at random until it finds one that is not previously used.
https://sqlite.org/lang_vacuum.html
The VACUUM command may change the ROWIDs of entries in any tables that do not have an explicit INTEGER PRIMARY KEY.
 */

impl SQLiteHistory {
    /// Open specifed
    pub fn open<P: AsRef<Path> + ?Sized>(config: Config, path: &P) -> Result<Self> {
        let mut sh = Self::new(config, Some(path.as_ref().to_path_buf()));
        sh.check_schema()?;
        Ok(sh)
    }

    fn new(config: Config, path: Option<PathBuf>) -> Self {
        SQLiteHistory {
            max_len: config.max_history_size(),
            ignore_space: config.history_ignore_space(),
            // not strictly consecutive...
            ignore_dups: config.history_duplicates() == HistoryDuplicates::IgnoreConsecutive,
            path,
            session_id: 0,
            row_id: 0,
        }
    }

    fn conn(&self) -> Result<Connection> {
        if let Some(ref path) = self.path {
            Connection::open(path)
        } else {
            Connection::open_in_memory()
        }
        .map_err(ReadlineError::from)
    }

    fn check_schema(&mut self) -> Result<()> {
        let conn = self.conn()?;
        let user_version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if user_version <= 0 {
            conn.execute_batch(
                "
PRAGMA auto_vacuum = INCREMENTAL;
CREATE TABLE session (
    id INTEGER PRIMARY KEY NOT NULL,
    timestamp REAL NOT NULL DEFAULT julianday('now')
); -- user, host, pid
CREATE TABLE history (
    --id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    session_id INTEGER NOT NULL,
    entry TEXT NOT NULL,
    timestamp REAL NOT NULL DEFAULT julianday('now'),
    FOREIGN KEY (session_id) REFERENCES session(id) ON DELETE CASCADE
);
--TODO fts enabled
CREATE VIRTUAL TABLE fts USING fts4(content=history, entry);
CREATE TRIGGER history_bu BEFORE UPDATE ON history BEGIN
    DELETE FROM fts WHERE docid=old.rowid;
END;
CREATE TRIGGER history_bd BEFORE DELETE ON history BEGIN
    DELETE FROM fts WHERE docid=old.rowid;
END;
CREATE TRIGGER history_au AFTER UPDATE ON history BEGIN
    INSERT INTO fts (docid, entry) VALUES (new.rowid, new.entry);
END;
CREATE TRIGGER history_ai AFTER INSERT ON history BEGIN
    INSERT INTO fts (docid, entry) VALUES(new.rowid, new.entry);
END;
PRAGMA user_version = 1;
                 ",
            )?
        }
        conn.pragma_update(None, "foreign_keys", 1)?;
        if self.ignore_dups {
            // TODO Validate ignore dups only in the same session_id ?
            conn.execute_batch(
                "CREATE UNIQUE INDEX IF NOT EXISTS ignore_dups ON history(entry, session_id);",
            )?;
        } else {
            conn.execute_batch("DROP INDEX IF EXISTS ignore_dups;")?;
        }
        Ok(())
    }

    fn create_session(&mut self) -> Result<()> {
        if self.session_id == 0 {
            self.session_id = self.conn()?.query_row(
                "INSERT INTO session (id) VALUES (NULL) RETURNING id;",
                [],
                |r| r.get(0),
            )?;
        }
        Ok(())
    }

    fn ignore(&self, line: &str) -> bool {
        if self.max_len == 0 {
            return true;
        }
        if line.is_empty()
            || (self.ignore_space && line.chars().next().map_or(true, char::is_whitespace))
        {
            return true;
        }
        // ignore_dups => SQLITE_CONSTRAINT_UNIQUE
        false
    }

    fn search_match(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
        start_with: bool,
    ) -> Result<Option<SearchResult>> {
        if term.is_empty() || start >= self.len() {
            return Ok(None);
        }
        let start_with = if start_with { "'^' || " } else { "" };
        let op = match dir {
            SearchDirection::Reverse => '<',
            SearchDirection::Forward => '>',
        };
        self.conn()?
            .query_row(
                &format!(
                    "SELECT docid, entry, offsets(entry) FROM fts WHERE entry MATCH {} ? || '*'  AND docid {} ?;",
                    start_with, op
                ),
                [],
                |r| {
                    Ok(SearchResult {
                        entry: Cow::Owned(r.get(1)?),
                        idx: r.get(0)?,
                        pos: r.get(2)?, // FIXME extract from offsets
                    })
                },
            )
            .optional()
            .map_err(ReadlineError::from)
    }
}

impl History for SQLiteHistory {
    /// Transient in-memory database
    fn with_config(config: Config) -> Self
    where
        Self: Sized,
    {
        Self::new(config, None)
    }

    fn get(&self, index: usize) -> Option<&String> {
        // TODO: rowid may not be sequential
        // SELECT entry FROM history WHERE rowid = :index;
        todo!()
    }

    fn add(&mut self, line: &str) -> Result<bool> {
        if self.ignore(line) {
            return Ok(false);
        }
        // Do not create a session until the first entry is added.
        self.create_session()?;
        //self.row_id
        // INSERT OR IGNORE INTO history (session_id, entry) VALUES (?, ?) RETURNING
        // rowid; ignore SQLITE_CONSTRAINT_UNIQUE
        todo!()
    }

    fn add_owned(&mut self, line: String) -> Result<bool> {
        self.add(line.as_str())
    }

    fn len(&self) -> usize {
        // max(rowid) vs count()
        todo!()
    }

    fn is_empty(&self) -> bool {
        self.row_id == 0
    }

    fn set_max_len(&mut self, len: usize) {
        // SELECT count(1) FROM history;
        // DELETE FROM history WHERE rowid IN (SELECT rowid FROM history ORDER BY rowid
        // ASC LIMIT ?);
        self.max_len = len;
        todo!()
    }

    fn ignore_dups(&mut self, yes: bool) {
        todo!()
    }

    fn ignore_space(&mut self, yes: bool) {
        self.ignore_space = yes;
    }

    fn save(&mut self, path: &Path) -> Result<()> {
        // TODO check self.path == path
        if self.session_id == 0 {
            // nothing to save
            return Ok(());
        }
        self.conn()?.execute_batch(
            "
PRAGMA optimize;
PRAGMA incremental_vacuum;
         ",
        )?;
        Ok(())
    }

    fn append(&mut self, path: &Path) -> Result<()> {
        // TODO check self.path == path
        Ok(())
    }

    fn load(&mut self, path: &Path) -> Result<()> {
        if self.path.is_none() {
            // TODO check that there is no memory entries (session_id == 0) ?
            self.path = Some(path.to_path_buf());
            self.check_schema()?;
        } else if self.path.as_ref().map_or(true, |p| p != path) {
            self.path = Some(path.to_path_buf());
            self.check_schema()?;
        }
        // Keep all on disk
        Ok(())
    }

    fn clear(&mut self) {
        // nothing in memory
    }

    fn search(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
    ) -> Result<Option<SearchResult>> {
        self.search_match(term, start, dir, false)
    }

    fn starts_with(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
    ) -> Result<Option<SearchResult>> {
        self.search_match(term, start, dir, true)
    }
}
