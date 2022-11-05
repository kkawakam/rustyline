//! History impl. based on SQLite
use crate::history::SearchResult;
use crate::{Config, History, SearchDirection};
use std::path::{Path, PathBuf};

// FIXME history_index (src/lib.rs:686)

struct SQLiteHistory {
    // TODO db: Connection,
    path: Option<PathBuf>,
    session_id: usize,
    row_id: usize,
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

impl History for SQLiteHistory {
    /// Transient in-memory database
    fn with_config(config: Config) -> Self
    where
        Self: Sized,
    {
        // -- if PRAGMA user_version == 0:
        // PRAGMA auto_vacuum = INCREMENTAL;
        // CREATE TABLE session (
        // id INTEGER PRIMARY KEY NOT NULL,
        // timestamp REAL NOT NULL DEFAULT julianday('now')
        // ); -- user, host, pid
        // CREATE TABLE history (
        // --id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
        // session_id INTEGER NOT NULL,
        // entry TEXT NOT NULL,
        // timestamp REAL NOT NULL DEFAULT julianday('now'),
        // FOREIGN KEY (session_id) REFERENCES session(id) ON DELETE CASCADE
        // );
        // -- only if ignore_dups is activated:
        // CREATE UNIQUE INDEX ignore_dups ON history(entry);
        // TODO fts enabled
        // CREATE VIRTUAL TABLE fts USING fts4(content=history, entry);
        // CREATE TRIGGER history_bu BEFORE UPDATE ON history BEGIN
        //     DELETE FROM fts WHERE docid=old.rowid;
        // END;
        // CREATE TRIGGER history_bd BEFORE DELETE ON history BEGIN
        //     DELETE FROM fts WHERE docid=old.rowid;
        // END;
        // CREATE TRIGGER history_au AFTER UPDATE ON history BEGIN
        //     INSERT INTO fts (docid, entry) VALUES (new.rowid, new.entry);
        // END;
        // CREATE TRIGGER history_ai AFTER INSERT ON history BEGIN
        //     INSERT INTO fts (docid, entry) VALUES(new.rowid, new.entry);
        // END;
        // PRAGMA user_version = 1;
        todo!()
        // PRAGMA foreign_keys = 1;
    }

    fn get(&self, index: usize) -> Option<&String> {
        // SELECT entry FROM history WHERE rowid = :index;
        todo!()
    }

    fn add(&mut self, line: &str) -> bool {
        // TODO ignore_space
        // INSERT INTO session (id) VALUES (NULL) RETURNING id; -- if session_id = 0
        // INSERT OR IGNORE INTO history (session_id, entry) VALUES (?, ?) RETURNING
        // rowid; ignore SQLITE_CONSTRAINT_UNIQUE
        todo!()
    }

    fn add_owned(&mut self, line: String) -> bool {
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
        todo!()
    }

    fn ignore_dups(&mut self, yes: bool) {
        todo!()
    }

    fn ignore_space(&mut self, yes: bool) {
        todo!()
    }

    fn save(&mut self, path: &Path) -> crate::Result<()> {
        // TODO check self.path == path
        // PRAGMA optimize;
        // PRAGMA incremental_vacuum;
        Ok(())
    }

    fn append(&mut self, path: &Path) -> crate::Result<()> {
        // TODO check self.path == path
        Ok(())
    }

    fn load(&mut self, path: &Path) -> crate::Result<()> {
        if self.path.is_none() {
            self.path = Some(path.to_path_buf());
        } else if self.path.map_or(true, |p| p != path) {
            self.path = Some(path.to_path_buf());
        }
        // Keep all on disk
        Ok(())
    }

    fn clear(&mut self) {
        // nothing in memory
    }

    fn search(&self, term: &str, start: usize, dir: SearchDirection) -> Option<SearchResult> {
        // SELECT * FROM fts WHERE entry MATCH ? || '*'  AND docid < ?;
        todo!()
    }

    fn starts_with(&self, term: &str, start: usize, dir: SearchDirection) -> Option<SearchResult> {
        // SELECT * FROM fts WHERE entry MATCH '^' || x || '*'  AND docid < ?;
        todo!()
    }
}
