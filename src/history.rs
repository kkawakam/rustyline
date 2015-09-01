//! History API

use std::collections::VecDeque;
use std::path::Path;
use std::fs::File;

use super::Result;

pub struct History {
    entries: VecDeque<String>,
    max_len: usize,
}

const DEFAULT_HISTORY_MAX_LEN: usize = 100;

impl History {
    pub fn new() -> History {
        History { entries: VecDeque::new(), max_len: DEFAULT_HISTORY_MAX_LEN }
    }

    /// Return the history entry at position `index`, starting from 0.
    pub fn get(& self, index: usize) -> Option<&String> {
        return self.entries.get(index)
    }

    /// Add a new entry in the history.
    pub fn add(&mut self, line: &str) -> bool {
        if self.max_len == 0 {
            return false;
        }
        if line.len() == 0 || line.chars().next().map_or(true, |c| c.is_whitespace()) { // ignorespace
            return false;
        }
        let s = String::from(line); // TODO try to allocate only on push_back
        if self.entries.back() == Some(&s) { // ignoredups
            return false;
        }
        if self.entries.len() == self.max_len {
            self.entries.pop_front();
        }
        self.entries.push_back(s);
        return true;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Set the maximum length for the history. This function can be called even
    /// if there is already some history, the function will make sure to retain
    /// just the latest `len` elements if the new history length value is smaller
    /// than the amount of items already inside the history.
    pub fn set_max_len(&mut self, len: usize) {
        self.max_len = len;
        if len == 0 {
            self.entries.clear();
            return;
        }
        loop {
            if self.entries.len() <= len {
                break;
            }
            self.entries.pop_front();
        }
    }

    /// Save the history in the specified file.
    pub fn save<P: AsRef<Path>+?Sized>(&self, path: &P) -> Result<()> {
        use std::io::{BufWriter, Write};

        if self.entries.len() == 0 {
            return Ok(());
        }
        let file = try!(File::create(path));
        let mut wtr = BufWriter::new(file);
        for entry in self.entries.iter() {
            try!(wtr.write_all(&entry.as_bytes()));
            try!(wtr.write_all(b"\n"));
        }
        return Ok(());
    }

    /// Load the history from the specified file.
    pub fn load<P: AsRef<Path>+?Sized>(&mut self, path: &P) -> Result<()> {
        use std::io::{BufRead, BufReader};

        let file = try!(File::open(&path));
        let rdr = BufReader::new(file);
        for line in rdr.lines() {
            self.add(try!(line).as_ref()); // TODO truncate to MAX_LINE
        }
        return Ok(());
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.entries.clear()
    }
}

#[cfg(test)]
mod tests {
    extern crate tempdir;
    use std::path::Path;

    fn init() -> super::History {
        let mut history = super::History::new();
        assert!(history.add("line1"));
        assert!(history.add("line2"));
        assert!(history.add("line3"));
        return history;
    }

    #[test]
    fn new() {
        let history = super::History::new();
        assert_eq!(super::DEFAULT_HISTORY_MAX_LEN, history.max_len);
        assert_eq!(0, history.entries.len());
    }

    #[test]
    fn add() {
        let mut history = super::History::new();
        assert!(history.add("line1"));
        assert!(history.add("line2"));
        assert!(! history.add("line2"));
        assert!(! history.add(""));
        assert!(! history.add(" line3"));
    }

    #[test]
    fn set_max_len() {
        let mut history = init();
        history.set_max_len(1);
        assert_eq!(1, history.entries.len());
        assert_eq!(Some(&"line3".to_string()), history.entries.back());
    }

    #[test]
    fn save() {
        let mut history = init();
        let td = tempdir::TempDir::new_in(&Path::new("."), "histo").unwrap();
        let history_path = td.path().join(".history");

        history.save(&history_path).unwrap();
        history.load(&history_path).unwrap();
        td.close().unwrap();
    }
}