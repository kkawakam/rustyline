//! Completion API
use std::collections::BTreeSet;
use std::fs;
use std::path::{self, Path};

use super::Result;
use line_buffer::LineBuffer;

// TODO: let the implementers choose/find word boudaries ???
// (line, pos) is like (rl_line_buffer, rl_point) to make contextual completion ("select t.na| from tbl as t")
// TOOD: make &self &mut self ???

/// To be called for tab-completion.
pub trait Completer {
    /// Takes the currently edited `line` with the cursor `pos`ition and
    /// returns the start position and the completion candidates for the partial word to be completed.
    /// "ls /usr/loc" => Ok((3, vec!["/usr/local/"]))
    fn complete(&self, line: &str, pos: usize) -> Result<(usize, Vec<String>)>;
    /// Updates the edited `line` with the `elected` candidate.
    fn update(&self, line: &mut LineBuffer, start: usize, elected: &str) {
        let end = line.pos();
        line.replace(start, end, elected)
    }
}

impl Completer for () {
    fn complete(&self, _line: &str, _pos: usize) -> Result<(usize, Vec<String>)> {
        Ok((0, Vec::new()))
    }
    fn update(&self, _line: &mut LineBuffer, _start: usize, _elected: &str) {
        unreachable!()
    }
}

impl<'c, C: ?Sized + Completer> Completer for &'c C {
    fn complete(&self, line: &str, pos: usize) -> Result<(usize, Vec<String>)> {
        (**self).complete(line, pos)
    }
    fn update(&self, line: &mut LineBuffer, start: usize, elected: &str) {
        (**self).update(line, start, elected)
    }
}
macro_rules! box_completer {
    ($($id: ident)*) => {
        $(
            impl<C: ?Sized + Completer> Completer for $id<C> {
                fn complete(&self, line: &str, pos: usize) -> Result<(usize, Vec<String>)> {
                    (**self).complete(line, pos)
                }
                fn update(&self, line: &mut LineBuffer, start: usize, elected: &str) {
                    (**self).update(line, start, elected)
                }
            }
        )*
    }
}

use std::rc::Rc;
use std::sync::Arc;
box_completer! { Box Rc Arc }

pub struct FilenameCompleter {
    break_chars: BTreeSet<char>,
}

#[cfg(unix)]
static DEFAULT_BREAK_CHARS: [char; 18] = [' ', '\t', '\n', '"', '\\', '\'', '`', '@', '$', '>',
                                          '<', '=', ';', '|', '&', '{', '(', '\0'];
// Remove \ to make file completion works on windows
#[cfg(windows)]
static DEFAULT_BREAK_CHARS: [char; 17] = [' ', '\t', '\n', '"', '\'', '`', '@', '$', '>',
                                          '<', '=', ';', '|', '&', '{', '(', '\0'];

impl FilenameCompleter {
    pub fn new() -> FilenameCompleter {
        FilenameCompleter { break_chars: DEFAULT_BREAK_CHARS.iter().cloned().collect() }
    }
}

impl Default for FilenameCompleter {
    fn default() -> FilenameCompleter {
        FilenameCompleter::new()
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
            match dir_path.strip_prefix("~") {
                Ok(rel_path) => home.join(rel_path),
                _ => home,
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

/// Given a `line` and a cursor `pos`ition,
/// try to find backward the start of a word.
/// Return (0, `line[..pos]`) if no break char has been found.
/// Return the word and its start position (idx, `line[idx..pos]`) otherwise.
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
