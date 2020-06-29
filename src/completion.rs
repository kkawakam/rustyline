//! Completion API
use std::borrow::Cow::{self, Borrowed, Owned};
use std::fs;
use std::path::{self, Path};

use crate::line_buffer::LineBuffer;
use crate::{Context, Result};
use memchr::memchr;

// TODO: let the implementers choose/find word boundaries ???
// (line, pos) is like (rl_line_buffer, rl_point) to make contextual completion
// ("select t.na| from tbl as t")
// TODO: make &self &mut self ???

/// A completion candidate.
pub trait Candidate {
    /// Text to display when listing alternatives.
    fn display(&self) -> &str;
    /// Text to insert in line.
    fn replacement(&self) -> &str;
}

impl Candidate for String {
    fn display(&self) -> &str {
        self.as_str()
    }

    fn replacement(&self) -> &str {
        self.as_str()
    }
}

/// Completion candidate pair
pub struct Pair {
    /// Text to display when listing alternatives.
    pub display: String,
    /// Text to insert in line.
    pub replacement: String,
}

impl Candidate for Pair {
    fn display(&self) -> &str {
        self.display.as_str()
    }

    fn replacement(&self) -> &str {
        self.replacement.as_str()
    }
}

/// To be called for tab-completion.
pub trait Completer {
    /// Specific completion candidate.
    type Candidate: Candidate;

    /// Takes the currently edited `line` with the cursor `pos`ition and
    /// returns the start position and the completion candidates for the
    /// partial word to be completed.
    ///
    /// ("ls /usr/loc", 11) => Ok((3, vec!["/usr/local/"]))
    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Self::Candidate>)> {
        let _ = (line, pos, ctx);
        Ok((0, Vec::with_capacity(0)))
    }
    /// Updates the edited `line` with the `elected` candidate.
    fn update(&self, line: &mut LineBuffer, start: usize, elected: &str) {
        let end = line.pos();
        line.replace(start..end, elected)
    }
}

impl Completer for () {
    type Candidate = String;

    fn update(&self, _line: &mut LineBuffer, _start: usize, _elected: &str) {
        unreachable!()
    }
}

impl<'c, C: ?Sized + Completer> Completer for &'c C {
    type Candidate = C::Candidate;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Self::Candidate>)> {
        (**self).complete(line, pos, ctx)
    }

    fn update(&self, line: &mut LineBuffer, start: usize, elected: &str) {
        (**self).update(line, start, elected)
    }
}
macro_rules! box_completer {
    ($($id: ident)*) => {
        $(
            impl<C: ?Sized + Completer> Completer for $id<C> {
                type Candidate = C::Candidate;

                fn complete(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Result<(usize, Vec<Self::Candidate>)> {
                    (**self).complete(line, pos, ctx)
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
    break_chars: &'static [u8],
    double_quotes_special_chars: &'static [u8],
}

const DOUBLE_QUOTES_ESCAPE_CHAR: Option<char> = Some('\\');

cfg_if::cfg_if! {
    if #[cfg(unix)] {
        // rl_basic_word_break_characters, rl_completer_word_break_characters
        const DEFAULT_BREAK_CHARS: [u8; 18] = [
            b' ', b'\t', b'\n', b'"', b'\\', b'\'', b'`', b'@', b'$', b'>', b'<', b'=', b';', b'|', b'&',
            b'{', b'(', b'\0',
        ];
        const ESCAPE_CHAR: Option<char> = Some('\\');
        // In double quotes, not all break_chars need to be escaped
        // https://www.gnu.org/software/bash/manual/html_node/Double-Quotes.html
        const DOUBLE_QUOTES_SPECIAL_CHARS: [u8; 4] = [b'"', b'$', b'\\', b'`'];
    } else if #[cfg(windows)] {
        // Remove \ to make file completion works on windows
        const DEFAULT_BREAK_CHARS: [u8; 17] = [
            b' ', b'\t', b'\n', b'"', b'\'', b'`', b'@', b'$', b'>', b'<', b'=', b';', b'|', b'&', b'{',
            b'(', b'\0',
        ];
        const ESCAPE_CHAR: Option<char> = None;
        const DOUBLE_QUOTES_SPECIAL_CHARS: [u8; 1] = [b'"']; // TODO Validate: only '"' ?
    } else if #[cfg(target_arch = "wasm32")] {
        const DEFAULT_BREAK_CHARS: [u8; 0] = [];
        const ESCAPE_CHAR: Option<char> = None;
        const DOUBLE_QUOTES_SPECIAL_CHARS: [u8; 0] = [];
    }
}

/// Kind of quote.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Quote {
    /// Double quote: `"`
    Double,
    /// Single quote: `'`
    Single,
    /// No quote
    None,
}

impl FilenameCompleter {
    /// Constructor
    pub fn new() -> Self {
        Self {
            break_chars: &DEFAULT_BREAK_CHARS,
            double_quotes_special_chars: &DOUBLE_QUOTES_SPECIAL_CHARS,
        }
    }

    /// Takes the currently edited `line` with the cursor `pos`ition and
    /// returns the start position and the completion candidates for the
    /// partial path to be completed.
    pub fn complete_path(&self, line: &str, pos: usize) -> Result<(usize, Vec<Pair>)> {
        let (start, path, esc_char, break_chars, quote) =
            if let Some((idx, quote)) = find_unclosed_quote(&line[..pos]) {
                let start = idx + 1;
                if quote == Quote::Double {
                    (
                        start,
                        unescape(&line[start..pos], DOUBLE_QUOTES_ESCAPE_CHAR),
                        DOUBLE_QUOTES_ESCAPE_CHAR,
                        &self.double_quotes_special_chars,
                        quote,
                    )
                } else {
                    (
                        start,
                        Borrowed(&line[start..pos]),
                        None,
                        &self.break_chars,
                        quote,
                    )
                }
            } else {
                let (start, path) = extract_word(line, pos, ESCAPE_CHAR, &self.break_chars);
                let path = unescape(path, ESCAPE_CHAR);
                (start, path, ESCAPE_CHAR, &self.break_chars, Quote::None)
            };
        let mut matches = filename_complete(&path, esc_char, break_chars, quote)?;
        #[allow(clippy::unnecessary_sort_by)]
        matches.sort_by(|a, b| a.display().cmp(b.display()));
        Ok((start, matches))
    }
}

impl Default for FilenameCompleter {
    fn default() -> Self {
        Self::new()
    }
}

impl Completer for FilenameCompleter {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Result<(usize, Vec<Pair>)> {
        self.complete_path(line, pos)
    }
}

/// Remove escape char
pub fn unescape(input: &str, esc_char: Option<char>) -> Cow<'_, str> {
    let esc_char = if let Some(c) = esc_char {
        c
    } else {
        return Borrowed(input);
    };
    if !input.chars().any(|c| c == esc_char) {
        return Borrowed(input);
    }
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == esc_char {
            if let Some(ch) = chars.next() {
                if cfg!(windows) && ch != '"' {
                    // TODO Validate: only '"' ?
                    result.push(esc_char);
                }
                result.push(ch);
            } else if cfg!(windows) {
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
pub fn escape(
    mut input: String,
    esc_char: Option<char>,
    break_chars: &[u8],
    quote: Quote,
) -> String {
    if quote == Quote::Single {
        return input; // no escape in single quotes
    }
    let n = input
        .bytes()
        .filter(|b| memchr(*b, break_chars).is_some())
        .count();
    if n == 0 {
        return input; // no need to escape
    }
    let esc_char = if let Some(c) = esc_char {
        c
    } else {
        if cfg!(windows) && quote == Quote::None {
            input.insert(0, '"'); // force double quote
            return input;
        }
        return input;
    };
    let mut result = String::with_capacity(input.len() + n);

    for c in input.chars() {
        if c.is_ascii() && memchr(c as u8, break_chars).is_some() {
            result.push(esc_char);
        }
        result.push(c);
    }
    result
}

fn filename_complete(
    path: &str,
    esc_char: Option<char>,
    break_chars: &[u8],
    quote: Quote,
) -> Result<Vec<Pair>> {
    #[cfg(feature = "with-dirs")]
    use dirs_next::home_dir;
    use std::env::current_dir;

    let sep = path::MAIN_SEPARATOR;
    let (dir_name, file_name) = match path.rfind(sep) {
        Some(idx) => path.split_at(idx + sep.len_utf8()),
        None => ("", path),
    };

    let dir_path = Path::new(dir_name);
    let dir = if dir_path.starts_with("~") {
        // ~[/...]
        #[cfg(feature = "with-dirs")]
        {
            if let Some(home) = home_dir() {
                match dir_path.strip_prefix("~") {
                    Ok(rel_path) => home.join(rel_path),
                    _ => home,
                }
            } else {
                dir_path.to_path_buf()
            }
        }
        #[cfg(not(feature = "with-dirs"))]
        {
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

    let mut entries: Vec<Pair> = Vec::new();

    // if dir doesn't exist, then don't offer any completions
    if !dir.exists() {
        return Ok(entries);
    }

    // if any of the below IO operations have errors, just ignore them
    if let Ok(read_dir) = dir.read_dir() {
        let file_name = normalize(file_name);
        for entry in read_dir {
            if let Ok(entry) = entry {
                if let Some(s) = entry.file_name().to_str() {
                    let ns = normalize(s);
                    if ns.starts_with(file_name.as_ref()) {
                        if let Ok(metadata) = fs::metadata(entry.path()) {
                            let mut path = String::from(dir_name) + s;
                            if metadata.is_dir() {
                                path.push(sep);
                            }
                            entries.push(Pair {
                                display: String::from(s),
                                replacement: escape(path, esc_char, break_chars, quote),
                            });
                        } // else ignore PermissionDenied
                    }
                }
            }
        }
    }
    Ok(entries)
}

#[cfg(any(windows, target_os = "macos"))]
fn normalize(s: &str) -> Cow<str> {
    // case insensitive
    Cow::Owned(s.to_lowercase())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn normalize(s: &str) -> Cow<str> {
    Cow::Borrowed(s)
}

/// Given a `line` and a cursor `pos`ition,
/// try to find backward the start of a word.
/// Return (0, `line[..pos]`) if no break char has been found.
/// Return the word and its start position (idx, `line[idx..pos]`) otherwise.
pub fn extract_word<'l>(
    line: &'l str,
    pos: usize,
    esc_char: Option<char>,
    break_chars: &[u8],
) -> (usize, &'l str) {
    let line = &line[..pos];
    if line.is_empty() {
        return (0, line);
    }
    let mut start = None;
    for (i, c) in line.char_indices().rev() {
        if let (Some(esc_char), true) = (esc_char, start.is_some()) {
            if esc_char == c {
                // escaped break char
                start = None;
                continue;
            } else {
                break;
            }
        }
        if c.is_ascii() && memchr(c as u8, break_chars).is_some() {
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

/// Returns the longest common prefix among all `Candidate::replacement()`s.
pub fn longest_common_prefix<C: Candidate>(candidates: &[C]) -> Option<&str> {
    if candidates.is_empty() {
        return None;
    } else if candidates.len() == 1 {
        return Some(&candidates[0].replacement());
    }
    let mut longest_common_prefix = 0;
    'o: loop {
        for (i, c1) in candidates.iter().enumerate().take(candidates.len() - 1) {
            let b1 = c1.replacement().as_bytes();
            let b2 = candidates[i + 1].replacement().as_bytes();
            if b1.len() <= longest_common_prefix
                || b2.len() <= longest_common_prefix
                || b1[longest_common_prefix] != b2[longest_common_prefix]
            {
                break 'o;
            }
        }
        longest_common_prefix += 1;
    }
    let candidate = candidates[0].replacement();
    while !candidate.is_char_boundary(longest_common_prefix) {
        longest_common_prefix -= 1;
    }
    if longest_common_prefix == 0 {
        return None;
    }
    Some(&candidate[0..longest_common_prefix])
}

#[derive(PartialEq)]
enum ScanMode {
    DoubleQuote,
    Escape,
    EscapeInDoubleQuote,
    Normal,
    SingleQuote,
}

/// try to find an unclosed single/double quote in `s`.
/// Return `None` if no unclosed quote is found.
/// Return the unclosed quote position and if it is a double quote.
fn find_unclosed_quote(s: &str) -> Option<(usize, Quote)> {
    let char_indices = s.char_indices();
    let mut mode = ScanMode::Normal;
    let mut quote_index = 0;
    for (index, char) in char_indices {
        match mode {
            ScanMode::DoubleQuote => {
                if char == '"' {
                    mode = ScanMode::Normal;
                } else if char == '\\' {
                    // both windows and unix support escape in double quote
                    mode = ScanMode::EscapeInDoubleQuote;
                }
            }
            ScanMode::Escape => {
                mode = ScanMode::Normal;
            }
            ScanMode::EscapeInDoubleQuote => {
                mode = ScanMode::DoubleQuote;
            }
            ScanMode::Normal => {
                if char == '"' {
                    mode = ScanMode::DoubleQuote;
                    quote_index = index;
                } else if char == '\\' && cfg!(not(windows)) {
                    mode = ScanMode::Escape;
                } else if char == '\'' && cfg!(not(windows)) {
                    mode = ScanMode::SingleQuote;
                    quote_index = index;
                }
            }
            ScanMode::SingleQuote => {
                if char == '\'' {
                    mode = ScanMode::Normal;
                } // no escape in single quotes
            }
        };
    }
    if ScanMode::DoubleQuote == mode || ScanMode::EscapeInDoubleQuote == mode {
        return Some((quote_index, Quote::Double));
    } else if ScanMode::SingleQuote == mode {
        return Some((quote_index, Quote::Single));
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    pub fn extract_word() {
        let break_chars: &[u8] = &super::DEFAULT_BREAK_CHARS;
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
        if cfg!(windows) {
            let input = "c:\\users\\All Users\\";
            let result: Cow<'_, str> = Borrowed(input);
            assert_eq!(result, super::unescape(input, Some('\\')));
        } else {
            let input = "/User\\ Information";
            let result: Cow<'_, str> = Owned(String::from("/User Information"));
            assert_eq!(result, super::unescape(input, Some('\\')));
        }
    }

    #[test]
    pub fn escape() {
        let break_chars: &[u8] = &super::DEFAULT_BREAK_CHARS;
        let input = String::from("/usr/local/b");
        assert_eq!(
            input.clone(),
            super::escape(input, Some('\\'), &break_chars, super::Quote::None)
        );
        let input = String::from("/User Information");
        let result = String::from("/User\\ Information");
        assert_eq!(
            result,
            super::escape(input, Some('\\'), &break_chars, super::Quote::None)
        );
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
        candidates.push(c1);
        {
            let lcp = super::longest_common_prefix(&candidates);
            assert_eq!(Some(s), lcp);
        }

        let c2 = String::from("Users");
        candidates.push(c2);
        {
            let lcp = super::longest_common_prefix(&candidates);
            assert_eq!(Some(s), lcp);
        }

        let c3 = String::from("");
        candidates.push(c3);
        {
            let lcp = super::longest_common_prefix(&candidates);
            assert!(lcp.is_none());
        }

        let candidates = vec![String::from("fée"), String::from("fête")];
        let lcp = super::longest_common_prefix(&candidates);
        assert_eq!(Some("f"), lcp);
    }

    #[test]
    pub fn find_unclosed_quote() {
        assert_eq!(None, super::find_unclosed_quote("ls /etc"));
        assert_eq!(
            Some((3, super::Quote::Double)),
            super::find_unclosed_quote("ls \"User Information")
        );
        assert_eq!(
            None,
            super::find_unclosed_quote("ls \"/User Information\" /etc")
        );
        assert_eq!(
            Some((0, super::Quote::Double)),
            super::find_unclosed_quote("\"c:\\users\\All Users\\")
        )
    }

    #[cfg(windows)]
    #[test]
    pub fn normalize() {
        assert_eq!(super::normalize("Windows"), "windows")
    }
}
