//! History impl. based on SQLite
use std::borrow::Cow;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, DatabaseName, OptionalExtension};

use crate::history::SearchResult;
use crate::{Config, History, HistoryDuplicates, ReadlineError, Result, SearchDirection};

// FIXME history_index (src/lib.rs:686)

/// History stored in an SQLite database.
pub struct SQLiteHistory {
    max_len: usize,
    ignore_space: bool,
    ignore_dups: bool,
    path: Option<PathBuf>, // None => memory
    conn: Connection,      /* we need to keep a connection opened at least for in memory
                            * database and also for cached statement(s) */
    session_id: usize, // 0 means no new entry added
    row_id: usize,     // max entry id
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
    /// Transient in-memory database
    pub fn with_config(config: Config) -> Result<Self>
    where
        Self: Sized,
    {
        Self::new(config, None)
    }

    /// Open specifed database
    pub fn open<P: AsRef<Path> + ?Sized>(config: Config, path: &P) -> Result<Self> {
        let mut sh = Self::new(config, normalize(path.as_ref()))?;
        sh.check_schema()?;
        Ok(sh)
    }

    fn new(config: Config, path: Option<PathBuf>) -> Result<Self> {
        let conn = conn(path.as_ref())?;
        Ok(SQLiteHistory {
            max_len: config.max_history_size(),
            ignore_space: config.history_ignore_space(),
            // not strictly consecutive...
            ignore_dups: config.history_duplicates() == HistoryDuplicates::IgnoreConsecutive,
            path,
            conn,
            session_id: 0,
            row_id: 0,
        })
    }

    fn is_mem_or_temp(&self) -> bool {
        match self.path {
            None => true,
            Some(ref p) => is_mem_or_temp(p),
        }
    }

    fn reset(&mut self, path: &Path) -> Result<Connection> {
        self.path = normalize(path);
        self.session_id = 0;
        self.row_id = 0;
        Ok(std::mem::replace(&mut self.conn, conn(self.path.as_ref())?))
    }

    fn update_row_id(&mut self) -> Result<()> {
        self.row_id =
            self.conn
                .query_row("SELECT ifnull(max(rowid), 0) FROM history;", [], |r| {
                    r.get(0)
                })?;
        Ok(())
    }

    fn check_schema(&mut self) -> Result<()> {
        let user_version: i32 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        if user_version <= 0 {
            self.conn.execute_batch(
                "
PRAGMA auto_vacuum = INCREMENTAL;
CREATE TABLE session (
    id INTEGER PRIMARY KEY NOT NULL,
    timestamp REAL NOT NULL DEFAULT (julianday('now'))
) STRICT; -- user, host, pid
CREATE TABLE history (
    --id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    session_id INTEGER NOT NULL,
    entry TEXT NOT NULL,
    timestamp REAL NOT NULL DEFAULT (julianday('now')),
    FOREIGN KEY (session_id) REFERENCES session(id) ON DELETE CASCADE
) STRICT;
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
        self.conn.pragma_update(None, "foreign_keys", 1)?;
        if self.ignore_dups || user_version > 0 {
            self.ignore_dups(self.ignore_dups)?;
        }
        if self.row_id == 0 && user_version > 0 {
            self.update_row_id()?;
        }
        Ok(())
    }

    fn create_session(&mut self) -> Result<()> {
        if self.session_id == 0 {
            self.check_schema()?;
            self.session_id = self.conn.query_row(
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

    fn add_entry(&mut self, line: &str) -> Result<bool> {
        // ignore SQLITE_CONSTRAINT_UNIQUE
        let mut stmt = self.conn.prepare_cached(
            "INSERT OR IGNORE INTO history (session_id, entry) VALUES (?, ?) RETURNING rowid;",
        )?;
        if let Some(row_id) = stmt
            .query_row((self.session_id, line), |r| r.get(0))
            .optional()?
        {
            self.row_id = row_id;
            Ok(true)
        } else {
            Ok(false)
        }
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
        let offsets = if start_with { "" } else { ", offsets(entry)" };
        let prefix = if start_with { "'^' || " } else { "" };
        let op = match dir {
            SearchDirection::Reverse => '<',
            SearchDirection::Forward => '>',
        };
        self.conn
            .query_row(
                &format!(
                    "SELECT docid, entry {} FROM fts WHERE entry MATCH {} ? || '*'  AND docid {} \
                     ?;",
                    offsets, prefix, op
                ),
                (term, start),
                |r| {
                    Ok(SearchResult {
                        entry: Cow::Owned(r.get(1)?),
                        idx: r.get(0)?,
                        pos: if start_with { 0 } else { r.get(2)? }, // FIXME extract from offsets
                    })
                },
            )
            .optional()
            .map_err(ReadlineError::from)
    }
}

impl History for SQLiteHistory {
    fn get(&self, index: usize) -> Result<Option<Cow<str>>> {
        // TODO: rowid may not be sequential
        let mut stmt = self
            .conn
            .prepare_cached("SELECT entry FROM history WHERE rowid = ?;")?;
        stmt.query_row([index], |r| r.get(0).map(Cow::Owned))
            .optional()
            .map_err(ReadlineError::from)
    }

    fn add(&mut self, line: &str) -> Result<bool> {
        if self.ignore(line) {
            return Ok(false);
        }
        // Do not create a session until the first entry is added.
        self.create_session()?;
        self.add_entry(line)
    }

    fn add_owned(&mut self, line: String) -> Result<bool> {
        self.add(line.as_str())
    }

    fn len(&self) -> usize {
        self.row_id + 1
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

    fn ignore_dups(&mut self, yes: bool) -> Result<()> {
        if self.ignore_dups {
            // TODO Validate ignore dups only in the same session_id ?
            self.conn.execute_batch(
                "CREATE UNIQUE INDEX IF NOT EXISTS ignore_dups ON history(entry, session_id);",
            )?;
        } else {
            self.conn
                .execute_batch("DROP INDEX IF EXISTS ignore_dups;")?;
        }
        Ok(())
    }

    fn ignore_space(&mut self, yes: bool) {
        self.ignore_space = yes;
    }

    fn save(&mut self, path: &Path) -> Result<()> {
        if self.session_id == 0 {
            // nothing to save
            return Ok(());
        } else if is_same(self.path.as_ref(), path) {
            if !self.is_mem_or_temp() {
                self.conn.execute_batch(
                    "
PRAGMA optimize;
PRAGMA incremental_vacuum;
         ",
                )?;
            }
        } else {
            // TODO Validate backup whole history
            self.conn.backup(DatabaseName::Main, path, None)?;
            // TODO Validate keep using original path
        }
        Ok(())
    }

    fn append(&mut self, path: &Path) -> Result<()> {
        if is_same(self.path.as_ref(), path) {
            return Ok(()); // no entry in memory
        } else if self.session_id == 0 {
            self.reset(path)?;
            self.check_schema()?;
            return Ok(()); // no entry to append
        }
        let old_id = self.session_id;
        {
            let old = self.reset(path)?; // keep connection alive in case of in-memory database
            self.create_session()?; // TODO preserve session.timestamp
            old.execute("ATTACH DATABASE ? AS new;", [path.to_string_lossy()])?; // TODO empty path / temporary database
            old.execute(
                "INSERT OR IGNORE INTO new.history (session_id, entry) SELECT ?, entry FROM \
                 history WHERE session_id = ?;",
                [self.session_id, old_id],
            )?; // TODO Validate only current session entries
            old.execute("DETACH DATABASE new;", [])?;

            let _ = old.close(); // FIXME busy
        }
        self.update_row_id()?;
        Ok(())
    }

    fn load(&mut self, path: &Path) -> Result<()> {
        #[allow(clippy::if_same_then_else)]
        if is_same(self.path.as_ref(), path) {
            return Ok(());
        } else if self.path.is_none() {
            // TODO check that there is no memory entries (session_id == 0) ?
            self.reset(path)?;
            self.check_schema()?;
        } else if self.path.as_ref().map_or(true, |p| p != path) {
            self.reset(path)?;
            self.check_schema()?;
        }
        // Keep all on disk
        Ok(())
    }

    fn clear(&mut self) -> Result<()> {
        if self.session_id == 0 {
            return Ok(());
        } else if self.is_mem_or_temp() {
            // ON DELETE CASCADE...
            self.conn
                .execute("DELETE FROM session WHERE id = ?;", [self.session_id])?;
            self.session_id = 0;
            self.update_row_id()?;
        } // else nothing in memory, TODO Validate: no delete ?
        Ok(())
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

fn conn(path: Option<&PathBuf>) -> rusqlite::Result<Connection> {
    if let Some(ref path) = path {
        Connection::open(path)
    } else {
        Connection::open_in_memory()
    }
}

const MEMORY: &str = ":memory:";

fn normalize(path: &Path) -> Option<PathBuf> {
    if path.as_os_str() == MEMORY {
        None
    } else {
        Some(path.to_path_buf())
    }
}
fn is_mem_or_temp(path: &Path) -> bool {
    let os_str = path.as_os_str();
    os_str.is_empty() || os_str == MEMORY
}
fn is_same(old: Option<&PathBuf>, new: &Path) -> bool {
    if let Some(old) = old {
        old == new // TODO canonicalize ?
    } else {
        new.as_os_str() == MEMORY
    }
}

#[cfg(test)]
mod tests {
    use super::SQLiteHistory;
    use crate::config::Config;
    use crate::history::History;
    use crate::Result;
    use std::path::Path;

    #[test]
    fn save() -> Result<()> {
        let db1 = "file:db1?mode=memory&cache=shared";
        let db2 = "file:db2?mode=memory";
        let mut h = SQLiteHistory::open(Config::default(), db1)?;
        h.save(Path::new(db1))?;
        h.save(Path::new(db2))?;
        h.add("line")?;
        h.save(Path::new(db1))?;
        assert_eq!(db1, h.path.unwrap().as_os_str());
        assert_eq!(1, h.session_id);
        assert_eq!(1, h.row_id);
        Ok(())
    }

    #[test]
    #[ignore] // FIXME panic
    fn append() -> Result<()> {
        let db1 = "file:db1?mode=memory&cache=shared";
        let db2 = "file:db2?mode=memory";
        let mut h = SQLiteHistory::open(Config::default(), db1)?;
        h.append(Path::new(db1))?;
        h.append(Path::new(db2))?;
        h.add("line")?;
        h.append(Path::new(db1))?;
        assert_eq!(db1, h.path.unwrap().as_os_str());
        assert_eq!(1, h.session_id);
        assert_eq!(1, h.row_id);
        Ok(())
    }

    #[test]
    fn load() -> Result<()> {
        let db1 = "file:db1?mode=memory";
        let db2 = "file:db2?mode=memory";
        let mut h = SQLiteHistory::open(Config::default(), db1)?;
        h.load(Path::new(db1))?;
        h.add("line")?;
        h.load(Path::new(db2))?;
        assert_eq!(db2, h.path.unwrap().as_os_str());
        assert_eq!(0, h.session_id);
        assert_eq!(0, h.row_id);
        Ok(())
    }

    #[test]
    fn clear() -> Result<()> {
        let mut h = SQLiteHistory::with_config(Config::default())?;
        h.clear()?;
        h.add("line")?;
        h.clear()?;
        assert_eq!(0, h.session_id);
        assert_eq!(0, h.row_id);
        assert_eq!(
            0,
            h.conn
                .query_row("SELECT count(1) FROM session;", [], |r| r.get::<_, i32>(0))?
        );
        assert_eq!(
            0,
            h.conn
                .query_row("SELECT count(1) FROM history;", [], |r| r.get::<_, i32>(0))?
        );
        Ok(())
    }
}
