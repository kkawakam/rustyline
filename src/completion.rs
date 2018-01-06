//! Completion API
use std::borrow::Cow::{self, Borrowed, Owned};
use std::collections::BTreeSet;
use std::fs;
use std::path::{self, Path};

use super::Result;
use line_buffer::LineBuffer;

// TODO: let the implementers choose/find word boudaries ???
// (line, pos) is like (rl_line_buffer, rl_point) to make contextual completion
// ("select t.na| from tbl as t")
// TODO: make &self &mut self ???

/// To be called for tab-completion.
pub trait Completer {
    /// Takes the currently edited `line` with the cursor `pos`ition and
    /// returns the start position and the completion candidates for the
    /// partial word to be completed.
    ///
    /// ("ls /usr/loc", 11) => Ok((3, vec!["/usr/local/"]))
    fn complete(&self, line: &str, pos: usize) -> Result<(usize, Vec<String>)>;
    /// Updates the edited `line` with the `elected` candidate.
    fn update(&self, line: &mut LineBuffer, start: usize, elected: &str) {
        let end = line.pos();
        line.replace(start..end, elected)
    }
}

impl Completer for () {
    fn complete(&self, _line: &str, _pos: usize) -> Result<(usize, Vec<String>)> {
        Ok((0, Vec::with_capacity(0)))
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

/// A `Completer` for file and folder names.
pub struct FilenameCompleter {
    break_chars: BTreeSet<char>,
}

#[cfg(unix)]
static DEFAULT_BREAK_CHARS: [char; 18] = [
    ' ', '\t', '\n', '"', '\\', '\'', '`', '@', '$', '>', '<', '=', ';', '|', '&', '{', '(', '\0'
];
#[cfg(unix)]
static ESCAPE_CHAR: Option<char> = Some('\\');
// Remove \ to make file completion works on windows
#[cfg(windows)]
static DEFAULT_BREAK_CHARS: [char; 17] = [
    ' ', '\t', '\n', '"', '\'', '`', '@', '$', '>', '<', '=', ';', '|', '&', '{', '(', '\0'
];
#[cfg(windows)]
static ESCAPE_CHAR: Option<char> = None;

impl FilenameCompleter {
    pub fn new() -> FilenameCompleter {
        FilenameCompleter {
            break_chars: DEFAULT_BREAK_CHARS.iter().cloned().collect(),
        }
    }
}

impl Default for FilenameCompleter {
    fn default() -> FilenameCompleter {
        FilenameCompleter::new()
    }
}

impl Completer for FilenameCompleter {
    fn complete(&self, line: &str, pos: usize) -> Result<(usize, Vec<String>)> {
        let (start, path) = extract_word(line, pos, ESCAPE_CHAR, &self.break_chars);
        let path = unescape(path, ESCAPE_CHAR);
        let matches = try!(filename_complete(&path, ESCAPE_CHAR, &self.break_chars));
        Ok((start, matches))
    }
}

/// Remove escape char
pub fn unescape(input: &str, esc_char: Option<char>) -> Cow<str> {
    if esc_char.is_none() {
        return Borrowed(input);
    }
    let esc_char = esc_char.unwrap();
    let n = input.chars().filter(|&c| c == esc_char).count();
    if n == 0 {
        return Borrowed(input);
    }
    let mut result = String::with_capacity(input.len() - n);
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == esc_char {
            if let Some(ch) = chars.next() {
                result.push(ch);
            }
        } else {
            result.push(ch);
        }
    }
    Owned(result)
}

/// Escape any `break_chars` in `input` string with `esc_char`.
/// For example, '/User Information' becomes '/User\ Information'
/// when space is a breaking char and '\\' the escape char.
pub fn escape(input: String, esc_char: Option<char>, break_chars: &BTreeSet<char>) -> String {
    if esc_char.is_none() {
        return input;
    }
    let esc_char = esc_char.unwrap();
    let n = input.chars().filter(|c| break_chars.contains(c)).count();
    if n == 0 {
        return input;
    }
    let mut result = String::with_capacity(input.len() + n);

    for c in input.chars() {
        if break_chars.contains(&c) {
            result.push(esc_char);
        }
        result.push(c);
    }
    result
}

fn filename_complete(
    path: &str,
    esc_char: Option<char>,
    break_chars: &BTreeSet<char>,
) -> Result<Vec<String>> {
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
    for entry in try!(dir.read_dir()) {
        let entry = try!(entry);
        if let Some(s) = entry.file_name().to_str() {
            if s.starts_with(file_name) {
                let mut path = String::from(dir_name) + s;
                if try!(fs::metadata(entry.path())).is_dir() {
                    path.push(sep);
                }
                entries.push(escape(path, esc_char, break_chars));
            }
        }
    }
    Ok(entries)
}

/// Given a `line` and a cursor `pos`ition,
/// try to find backward the start of a word.
/// Return (0, `line[..pos]`) if no break char has been found.
/// Return the word and its start position (idx, `line[idx..pos]`) otherwise.
pub fn extract_word<'l>(
    line: &'l str,
    pos: usize,
    esc_char: Option<char>,
    break_chars: &BTreeSet<char>,
) -> (usize, &'l str) {
    let line = &line[..pos];
    if line.is_empty() {
        return (0, line);
    }
    let mut start = None;
    for (i, c) in line.char_indices().rev() {
        if esc_char.is_some() && start.is_some() {
            if esc_char.unwrap() == c {
                // escaped break char
                start = None;
                continue;
            } else {
                break;
            }
        }
        if break_chars.contains(&c) {
            start = Some(i + c.len_utf8());
            if esc_char.is_none() {
                break;
            } // else maybe escaped...
        }
    }

    match start {
        Some(start) => (start, &line[start..]),
        None => (0, line),
    }
}

pub fn longest_common_prefix(candidates: &[String]) -> Option<&str> {
    if candidates.is_empty() {
        return None;
    } else if candidates.len() == 1 {
        return Some(&candidates[0]);
    }
    let mut longest_common_prefix = 0;
    'o: loop {
        for (i, c1) in candidates.iter().enumerate().take(candidates.len() - 1) {
            let b1 = c1.as_bytes();
            let b2 = candidates[i + 1].as_bytes();
            if b1.len() <= longest_common_prefix || b2.len() <= longest_common_prefix
                || b1[longest_common_prefix] != b2[longest_common_prefix]
            {
                break 'o;
            }
        }
        longest_common_prefix += 1;
    }
    while !candidates[0].is_char_boundary(longest_common_prefix) {
        longest_common_prefix -= 1;
    }
    if longest_common_prefix == 0 {
        return None;
    }
    Some(&candidates[0][0..longest_common_prefix])
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    #[test]
    pub fn extract_word() {
        let break_chars: BTreeSet<char> = super::DEFAULT_BREAK_CHARS.iter().cloned().collect();
        let line = "ls '/usr/local/b";
        assert_eq!(
            (4, "/usr/local/b"),
            super::extract_word(line, line.len(), Some('\\'), &break_chars)
        );
        let line = "ls /User\\ Information";
        assert_eq!(
            (3, "/User\\ Information"),
            super::extract_word(line, line.len(), Some('\\'), &break_chars)
        );
    }

    #[test]
    pub fn unescape() {
        use std::borrow::Cow::{self, Borrowed, Owned};
        let input = "/usr/local/b";
        assert_eq!(Borrowed(input), super::unescape(input, Some('\\')));
        let input = "/User\\ Information";
        let result: Cow<str> = Owned(String::from("/User Information"));
        assert_eq!(result, super::unescape(input, Some('\\')));
    }

    #[test]
    pub fn escape() {
        let break_chars: BTreeSet<char> = super::DEFAULT_BREAK_CHARS.iter().cloned().collect();
        let input = String::from("/usr/local/b");
        assert_eq!(
            input.clone(),
            super::escape(input, Some('\\'), &break_chars)
        );
        let input = String::from("/User Information");
        let result = String::from("/User\\ Information");
        assert_eq!(result, super::escape(input, Some('\\'), &break_chars));
    }

    #[test]
    pub fn longest_common_prefix() {
        let mut candidates = vec![];
        {
            let lcp = super::longest_common_prefix(&candidates);
            assert!(lcp.is_none());
        }

        let s = "User";
        let c1 = String::from(s);
        candidates.push(c1.clone());
        {
            let lcp = super::longest_common_prefix(&candidates);
            assert_eq!(Some(s), lcp);
        }

        let c2 = String::from("Users");
        candidates.push(c2.clone());
        {
            let lcp = super::longest_common_prefix(&candidates);
            assert_eq!(Some(s), lcp);
        }

        let c3 = String::from("");
        candidates.push(c3.clone());
        {
            let lcp = super::longest_common_prefix(&candidates);
            assert!(lcp.is_none());
        }

        let candidates = vec![String::from("fée"), String::from("fête")];
        let lcp = super::longest_common_prefix(&candidates);
        assert_eq!(Some("f"), lcp);
    }
}
