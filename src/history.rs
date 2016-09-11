//! History API

use std::collections::VecDeque;
use std::path::Path;
use std::fs::File;

use super::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Reverse,
}

pub struct History {
    entries: VecDeque<String>,
    max_len: usize,
    ignore_space: bool,
    ignore_dups: bool,
}

const DEFAULT_HISTORY_MAX_LEN: usize = 100;

impl History {
    pub fn new() -> History {
        History {
            entries: VecDeque::new(),
            max_len: DEFAULT_HISTORY_MAX_LEN,
            ignore_space: false,
            ignore_dups: true,
        }
    }

    /// Tell if lines which begin with a space character are saved or not in the history list.
    /// By default, they are saved.
    pub fn ignore_space(&mut self, yes: bool) {
        self.ignore_space = yes;
    }

    /// Tell if lines which match the previous history entry are saved or not in the history list.
    /// By default, they are ignored.
    pub fn ignore_dups(&mut self, yes: bool) {
        self.ignore_dups = yes;
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
        if line.is_empty() ||
           (self.ignore_space && line.chars().next().map_or(true, |c| c.is_whitespace())) {
            return false;
        }
        if self.ignore_dups {
            if let Some(s) = self.entries.back() {
                if s == line {
                    return false;
                }
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
    /// Return the absolute index of the nearest history entry that matches `term`.
    /// Return None if no entry contains `term` between [start, len -1] for forward search
    /// or between [0, start] for reverse search.
    pub fn search(&self, term: &str, start: usize, dir: Direction) -> Option<usize> {
        if term.is_empty() || start >= self.len() {
            return None;
        }
        match dir {
            Direction::Reverse => {
                let index = self.entries
                    .iter()
                    .rev()
                    .skip(self.entries.len() - 1 - start)
                    .position(|entry| entry.contains(term));
                index.and_then(|index| Some(start - index))
            }
            Direction::Forward => {
                let index = self.entries.iter().skip(start).position(|entry| entry.contains(term));
                index.and_then(|index| Some(index + start))
            }
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
    use super::{Direction, History};

    fn init() -> History {
        let mut history = History::new();
        assert!(history.add("line1"));
        assert!(history.add("line2"));
        assert!(history.add("line3"));
        history
    }

    #[test]
    fn new() {
        let history = History::new();
        assert_eq!(super::DEFAULT_HISTORY_MAX_LEN, history.max_len);
        assert_eq!(0, history.entries.len());
    }

    #[test]
    fn add() {
        let mut history = History::new();
        history.ignore_space(true);
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
        assert_eq!(None, history.search("", 0, Direction::Forward));
        assert_eq!(None, history.search("none", 0, Direction::Forward));
        assert_eq!(None, history.search("line", 3, Direction::Forward));

        assert_eq!(Some(0), history.search("line", 0, Direction::Forward));
        assert_eq!(Some(1), history.search("line", 1, Direction::Forward));
        assert_eq!(Some(2), history.search("line3", 1, Direction::Forward));
    }

    #[test]
    fn reverse_search() {
        let history = init();
        assert_eq!(None, history.search("", 2, Direction::Reverse));
        assert_eq!(None, history.search("none", 2, Direction::Reverse));
        assert_eq!(None, history.search("line", 3, Direction::Reverse));

        assert_eq!(Some(2), history.search("line", 2, Direction::Reverse));
        assert_eq!(Some(1), history.search("line", 1, Direction::Reverse));
        assert_eq!(Some(0), history.search("line1", 1, Direction::Reverse));
    }
}
