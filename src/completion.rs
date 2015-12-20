//! Completion API
use std::collections::BTreeSet;
use std::fs;
use std::path::{self, Path};

use super::Result;

// TODO: let the implementers choose/find word boudaries ???
// (line, pos) is like (rl_line_buffer, rl_point) to make contextual completion ("select t.na| from tbl as t")
// TOOD: make &self &mut self ???
// TODO: change update signature: _line: Into<String>

/// To be called for tab-completion.
pub trait Completer {
    /// Takes the currently edited `line` with the cursor `pos`ition and
    /// returns the start position and the completion candidates for the partial word to be completed.
    /// "ls /usr/loc" => Ok((3, vec!["/usr/local/"]))
    fn complete(&self, line: &str, pos: usize) -> Result<(usize, Vec<String>)>;
    /// Takes the currently edited `line` with the cursor `pos`ition and
    /// the `elected` candidate.
    /// Returns the new line content and cursor position.
    fn update(&self, line: &str, pos: usize, start: usize, elected: &str) -> (String, usize) {
        let mut buf = String::with_capacity(start + elected.len() + line.len() - pos);
        buf.push_str(&line[..start]);
        buf.push_str(elected);
        // buf.push(' ');
        let new_pos = buf.len();
        buf.push_str(&line[pos..]);
        (buf, new_pos)
    }
}

pub struct FilenameCompleter {
    break_chars: BTreeSet<char>,
}

static DEFAULT_BREAK_CHARS: [char; 18] = [' ', '\t', '\n', '"', '\\', '\'', '`', '@', '$', '>',
                                          '<', '=', ';', '|', '&', '{', '(', '\0'];

impl FilenameCompleter {
    pub fn new() -> FilenameCompleter {
        FilenameCompleter { break_chars: DEFAULT_BREAK_CHARS.iter().cloned().collect() }
    }
}

impl Completer for FilenameCompleter {
    fn complete(&self, line: &str, pos: usize) -> Result<(usize, Vec<String>)> {
        let (start, path) = extract_word(line, pos, &self.break_chars);
        let matches = try!(filename_complete(path));
        Ok((start, matches))
    }
}

fn filename_complete(path: &str) -> Result<Vec<String>> {
    use std::env::{current_dir, home_dir};

    let sep = path::MAIN_SEPARATOR;
    let (dir_name, file_name) = match path.rfind(sep) {
        Some(idx) => path.split_at(idx + sep.len_utf8()),
        None => ("", path),
    };

    let dir_path = Path::new(dir_name);
    let dir = if dir_path.starts_with("~") {
        // ~[/...]
        if let Some(home) = home_dir() {
            match dir_path.relative_from("~") {
                Some(rel_path) => home.join(rel_path),
                None => home,
            }
        } else {
            dir_path.to_path_buf()
        }
    } else if dir_path.is_relative() {
        // TODO ~user[/...] (https://crates.io/crates/users)
        if let Ok(cwd) = current_dir() {
            cwd.join(dir_path)
        } else {
            dir_path.to_path_buf()
        }
    } else {
        dir_path.to_path_buf()
    };

    let mut entries: Vec<String> = Vec::new();
    for entry in try!(fs::read_dir(dir)) {
        let entry = try!(entry);
        if let Some(s) = entry.file_name().to_str() {
            if s.starts_with(file_name) {
                let mut path = String::from(dir_name) + s;
                if try!(fs::metadata(entry.path())).is_dir() {
                    path.push(sep);
                }
                entries.push(path);
            }
        }
    }
    Ok(entries)
}

pub fn extract_word<'l>(line: &'l str,
                        pos: usize,
                        break_chars: &BTreeSet<char>)
                        -> (usize, &'l str) {
    let line = &line[..pos];
    if line.is_empty() {
        return (0, line);
    }
    match line.char_indices().rev().find(|&(_, c)| break_chars.contains(&c)) {
        Some((i, c)) => {
            let start = i + c.len_utf8();
            (start, &line[start..])
        }
        None => (0, line),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    #[test]
    pub fn extract_word() {
        let break_chars: BTreeSet<char> = super::DEFAULT_BREAK_CHARS.iter().cloned().collect();
        let line = "ls '/usr/local/b";
        assert_eq!((4, "/usr/local/b"),
                   super::extract_word(line, line.len(), &break_chars));
    }
}
