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
        History {
            entries: VecDeque::new(),
            max_len: DEFAULT_HISTORY_MAX_LEN,
        }
    }

    /// Return the history entry at position `index`, starting from 0.
    pub fn get(&self, index: usize) -> Option<&String> {
        self.entries.get(index)
    }

    /// Add a new entry in the history.
    pub fn add(&mut self, line: &str) -> bool {
        if self.max_len == 0 {
            return false;
        }
        if line.is_empty() || line.chars().next().map_or(true, |c| c.is_whitespace()) {
            // ignorespace
            return false;
        }
        if let Some(s) = self.entries.back() {
            if s == line {
                // ignoredups
                return false;
            }
        }
        if self.entries.len() == self.max_len {
            self.entries.pop_front();
        }
        self.entries.push_back(String::from(line));
        true
    }

    /// Returns the number of entries in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Returns true if the history has no entry.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
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
    pub fn save<P: AsRef<Path> + ?Sized>(&self, path: &P) -> Result<()> {
        use std::io::{BufWriter, Write};

        if self.is_empty() {
            return Ok(());
        }
        let file = try!(File::create(path));
        let mut wtr = BufWriter::new(file);
        for entry in &self.entries {
            try!(wtr.write_all(&entry.as_bytes()));
            try!(wtr.write_all(b"\n"));
        }
        Ok(())
    }

    /// Load the history from the specified file.
    pub fn load<P: AsRef<Path> + ?Sized>(&mut self, path: &P) -> Result<()> {
        use std::io::{BufRead, BufReader};

        let file = try!(File::open(&path));
        let rdr = BufReader::new(file);
        for line in rdr.lines() {
            self.add(try!(line).as_ref()); // TODO truncate to MAX_LINE
        }
        Ok(())
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.entries.clear()
    }

    /// Search history (start position inclusive [0, len-1])
    pub fn search(&self, term: &str, start: usize, reverse: bool) -> Option<usize> {
        if term.is_empty() || start >= self.len() {
            return None;
        }
        if reverse {
            let index = self.entries
                .iter()
                .rev()
                .skip(self.entries.len() - 1 - start)
                .position(|entry| entry.contains(term));
            index.and_then(|index| Some(start - index))
        } else {
            let index = self.entries.iter().skip(start).position(|entry| entry.contains(term));
            index.and_then(|index| Some(index + start))
        }
    }
}

impl Default for History {
    fn default() -> History {
        History::new()
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
        history
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
        assert!(!history.add("line2"));
        assert!(!history.add(""));
        assert!(!history.add(" line3"));
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

    #[test]
    fn search() {
        let history = init();
        assert_eq!(None, history.search("", 0, false));
        assert_eq!(None, history.search("none", 0, false));
        assert_eq!(None, history.search("line", 3, false));

        assert_eq!(Some(0), history.search("line", 0, false));
        assert_eq!(Some(1), history.search("line", 1, false));
        assert_eq!(Some(2), history.search("line3", 1, false));
    }

    #[test]
    fn reverse_search() {
        let history = init();
        assert_eq!(None, history.search("", 2, true));
        assert_eq!(None, history.search("none", 2, true));
        assert_eq!(None, history.search("line", 3, true));

        assert_eq!(Some(2), history.search("line", 2, true));
        assert_eq!(Some(1), history.search("line", 1, true));
        assert_eq!(Some(0), history.search("line1", 1, true));
    }
}
