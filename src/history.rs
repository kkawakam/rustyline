//! History API

use std::collections::vec_deque;
use std::collections::VecDeque;
use std::fs::File;
use std::iter::DoubleEndedIterator;
use std::ops::Index;
use std::path::Path;

use super::Result;
use crate::config::{Config, HistoryDuplicates};

/// Search direction
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Reverse,
}

/// Current state of the history.
#[derive(Default)]
pub struct History {
    entries: VecDeque<String>,
    max_len: usize,
    pub(crate) ignore_space: bool,
    pub(crate) ignore_dups: bool,
}

impl History {
    // New multiline-aware history files start with `#V2\n` and have newlines
    // and backslashes escaped in them.
    const FILE_VERSION_V2: &'static str = "#V2";

    pub fn new() -> Self {
        Self::with_config(Config::default())
    }

    pub fn with_config(config: Config) -> Self {
        Self {
            entries: VecDeque::new(),
            max_len: config.max_history_size(),
            ignore_space: config.history_ignore_space(),
            ignore_dups: config.history_duplicates() == HistoryDuplicates::IgnoreConsecutive,
        }
    }

    /// Return the history entry at position `index`, starting from 0.
    pub fn get(&self, index: usize) -> Option<&String> {
        self.entries.get(index)
    }

    /// Return the last history entry (i.e. previous command)
    pub fn last(&self) -> Option<&String> {
        self.entries.back()
    }

    /// Add a new entry in the history.
    pub fn add<S: AsRef<str> + Into<String>>(&mut self, line: S) -> bool {
        if self.max_len == 0 {
            return false;
        }
        if line.as_ref().is_empty()
            || (self.ignore_space
                && line
                    .as_ref()
                    .chars()
                    .next()
                    .map_or(true, char::is_whitespace))
        {
            return false;
        }
        if self.ignore_dups {
            if let Some(s) = self.entries.back() {
                if s == line.as_ref() {
                    return false;
                }
            }
        }
        if self.entries.len() == self.max_len {
            self.entries.pop_front();
        }
        self.entries.push_back(line.into());
        true
    }

    /// Return the number of entries in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return true if the history has no entry.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Set the maximum length for the history. This function can be called even
    /// if there is already some history, the function will make sure to retain
    /// just the latest `len` elements if the new history length value is
    /// smaller than the amount of items already inside the history.
    ///
    /// Like [stifle_history](http://cnswww.cns.cwru.
    /// edu/php/chet/readline/history.html#IDX11).
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
    // TODO append_history
    // http://cnswww.cns.cwru.edu/php/chet/readline/history.html#IDX30
    // TODO history_truncate_file
    // http://cnswww.cns.cwru.edu/php/chet/readline/history.html#IDX31
    pub fn save<P: AsRef<Path> + ?Sized>(&self, path: &P) -> Result<()> {
        use std::io::{BufWriter, Write};

        if self.is_empty() {
            return Ok(());
        }
        let old_umask = umask();
        let f = File::create(path);
        restore_umask(old_umask);
        let file = f?;
        fix_perm(&file);
        let mut wtr = BufWriter::new(file);
        wtr.write_all(Self::FILE_VERSION_V2.as_bytes())?;
        for entry in &self.entries {
            wtr.write_all(b"\n")?;
            wtr.write_all(entry.replace('\\', "\\\\").replace('\n', "\\n").as_bytes())?;
        }
        wtr.write_all(b"\n")?;
        // https://github.com/rust-lang/rust/issues/32677#issuecomment-204833485
        wtr.flush()?;
        Ok(())
    }

    /// Load the history from the specified file.
    ///
    /// # Errors
    /// Will return `Err` if path does not already exist or could not be read.
    pub fn load<P: AsRef<Path> + ?Sized>(&mut self, path: &P) -> Result<()> {
        use std::io::{BufRead, BufReader};

        let file = File::open(&path)?;
        let rdr = BufReader::new(file);
        let mut lines = rdr.lines();
        let mut v2 = false;
        if let Some(first) = lines.next() {
            let line = first?;
            if line == Self::FILE_VERSION_V2 {
                v2 = true;
            } else {
                self.add(line);
            }
        }
        for line in lines {
            let line = if v2 {
                line?.replace("\\n", "\n").replace("\\\\", "\\")
            } else {
                line?
            };
            if line.is_empty() {
                continue;
            }
            self.add(line); // TODO truncate to MAX_LINE
        }
        Ok(())
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.entries.clear()
    }

    /// Search history (start position inclusive [0, len-1]).
    ///
    /// Return the absolute index of the nearest history entry that matches
    /// `term`.
    ///
    /// Return None if no entry contains `term` between [start, len -1] for
    /// forward search
    /// or between [0, start] for reverse search.
    pub fn search(&self, term: &str, start: usize, dir: Direction) -> Option<usize> {
        let test = |entry: &String| entry.contains(term);
        self.search_match(term, start, dir, test)
    }

    /// Anchored search
    pub fn starts_with(&self, term: &str, start: usize, dir: Direction) -> Option<usize> {
        let test = |entry: &String| entry.starts_with(term);
        self.search_match(term, start, dir, test)
    }

    fn search_match<F>(&self, term: &str, start: usize, dir: Direction, test: F) -> Option<usize>
    where
        F: Fn(&String) -> bool,
    {
        if term.is_empty() || start >= self.len() {
            return None;
        }
        match dir {
            Direction::Reverse => {
                let index = self
                    .entries
                    .iter()
                    .rev()
                    .skip(self.entries.len() - 1 - start)
                    .position(test);
                index.map(|index| start - index)
            }
            Direction::Forward => {
                let index = self.entries.iter().skip(start).position(test);
                index.map(|index| index + start)
            }
        }
    }

    /// Return a forward iterator.
    pub fn iter(&self) -> Iter<'_> {
        Iter(self.entries.iter())
    }
}

impl Index<usize> for History {
    type Output = String;

    fn index(&self, index: usize) -> &String {
        &self.entries[index]
    }
}

impl<'a> IntoIterator for &'a History {
    type IntoIter = Iter<'a>;
    type Item = &'a String;

    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

/// History iterator.
pub struct Iter<'a>(vec_deque::Iter<'a, String>);

impl<'a> Iterator for Iter<'a> {
    type Item = &'a String;

    fn next(&mut self) -> Option<&'a String> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<&'a String> {
        self.0.next_back()
    }
}

cfg_if::cfg_if! {
    if #[cfg(any(windows, target_arch = "wasm32"))] {
        fn umask() -> u16 {
            0
        }

        fn restore_umask(_: u16) {}

        fn fix_perm(_: &File) {}
    } else if #[cfg(unix)] {
        fn umask() -> libc::mode_t {
            unsafe { libc::umask(libc::S_IXUSR | libc::S_IRWXG | libc::S_IRWXO) }
        }

        fn restore_umask(old_umask: libc::mode_t) {
            unsafe {
                libc::umask(old_umask);
            }
        }

        fn fix_perm(file: &File) {
            use std::os::unix::io::AsRawFd;
            unsafe {
                libc::fchmod(file.as_raw_fd(), libc::S_IRUSR | libc::S_IWUSR);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Direction, History};
    use crate::config::Config;
    use std::path::Path;

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
        assert_eq!(0, history.entries.len());
    }

    #[test]
    fn add() {
        let config = Config::builder().history_ignore_space(true).build();
        let mut history = History::with_config(config);
        assert_eq!(config.max_history_size(), history.max_len);
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
        assert_eq!(Some(&"line3".to_owned()), history.last());
    }

    #[test]
    #[cfg_attr(miri, ignore)] // unsupported operation: `getcwd` not available when isolation is enabled
    fn save() {
        let mut history = init();
        assert!(history.add("line\nfour \\ abc"));
        let td = tempdir::TempDir::new_in(&Path::new("."), "histo").unwrap();
        let history_path = td.path().join(".history");

        history.save(&history_path).unwrap();
        let mut history2 = History::new();
        history2.load(&history_path).unwrap();
        for (a, b) in history.entries.iter().zip(history2.entries.iter()) {
            assert_eq!(a, b);
        }

        td.close().unwrap();
    }

    #[test]
    #[cfg_attr(miri, ignore)] // unsupported operation: `getcwd` not available when isolation is enabled
    fn load_legacy() {
        use std::io::Write;
        let td = tempdir::TempDir::new_in(&Path::new("."), "histo").unwrap();
        let history_path = td.path().join(".history_v1");
        {
            let mut legacy = std::fs::File::create(&history_path).unwrap();
            // Some data we'd accidentally corrupt if we got the version wrong
            let data = b"\
                test\\n \\abc \\123\n\
                123\\n\\\\n\n\
                abcde
            ";
            legacy.write_all(data).unwrap();
            legacy.flush().unwrap();
        }
        let mut history = History::new();
        history.load(&history_path).unwrap();
        assert_eq!(history.entries[0], "test\\n \\abc \\123");
        assert_eq!(history.entries[1], "123\\n\\\\n");
        assert_eq!(history.entries[2], "abcde");
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
