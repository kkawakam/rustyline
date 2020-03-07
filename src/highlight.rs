//! Syntax highlighting

use crate::config::CompletionType;
use memchr::memchr;
use std::borrow::Cow::{self, Borrowed, Owned};
use std::cell::Cell;

pub struct PromptInfo<'a> {
    pub(crate) default: bool,
    pub(crate) offset: usize,
    pub(crate) cursor: Option<usize>,
    pub(crate) input: &'a str,
    pub(crate) line: &'a str,
    pub(crate) line_no: usize,
}

/// Syntax highlighter with [ANSI color](https://en.wikipedia.org/wiki/ANSI_escape_code#SGR_(Select_Graphic_Rendition)_parameters).
/// Rustyline will try to handle escape sequence for ANSI color on windows
/// when not supported natively (windows <10).
///
/// Currently, the highlighted version *must* have the same display width as
/// the original input.
pub trait Highlighter {
    /// Takes the currently edited `line` with the cursor `pos`ition and
    /// returns the highlighted version (with ANSI color).
    ///
    /// For example, you can implement
    /// [blink-matching-paren](https://www.gnu.org/software/bash/manual/html_node/Readline-Init-File-Syntax.html).
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        let _ = pos;
        Borrowed(line)
    }
    /// Takes the `prompt` and
    /// returns the highlighted version (with ANSI color).
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        info: PromptInfo<'_>,
    ) -> Cow<'b, str> {
        let _ = info;
        Borrowed(prompt)
    }

    /// Returns `true` if prompt is rectangular rather than being present only
    /// on the first line of input
    fn has_continuation_prompt(&self) -> bool {
        false
    }

    /// Takes the `hint` and
    /// returns the highlighted version (with ANSI color).
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Borrowed(hint)
    }
    /// Takes the completion `candidate` and
    /// returns the highlighted version (with ANSI color).
    ///
    /// Currently, used only with `CompletionType::List`.
    fn highlight_candidate<'c>(
        &self,
        candidate: &'c str,
        completion: CompletionType,
    ) -> Cow<'c, str> {
        let _ = completion;
        Borrowed(candidate)
    }
    /// Tells if `line` needs to be highlighted when a specific char is typed or
    /// when cursor is moved under a specific char.
    ///
    /// Used to optimize refresh when a character is inserted or the cursor is
    /// moved.
    fn highlight_char(&self, line: &str, pos: usize) -> bool {
        let _ = (line, pos);
        false
    }
}

impl Highlighter for () {}

impl<'r, H: ?Sized + Highlighter> Highlighter for &'r H {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        (**self).highlight(line, pos)
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        info: PromptInfo<'_>,
    ) -> Cow<'b, str> {
        (**self).highlight_prompt(prompt, info)
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        (**self).highlight_hint(hint)
    }

    fn highlight_candidate<'c>(
        &self,
        candidate: &'c str,
        completion: CompletionType,
    ) -> Cow<'c, str> {
        (**self).highlight_candidate(candidate, completion)
    }

    fn highlight_char(&self, line: &str, pos: usize) -> bool {
        (**self).highlight_char(line, pos)
    }
}

const OPENS: &[u8; 3] = b"{[(";
const CLOSES: &[u8; 3] = b"}])";

#[derive(Default)]
pub struct MatchingBracketHighlighter {
    bracket: Cell<Option<(u8, usize)>>, // memorize the character to search...
}

impl MatchingBracketHighlighter {
    pub fn new() -> Self {
        Self {
            bracket: Cell::new(None),
        }
    }
}

impl Highlighter for MatchingBracketHighlighter {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        if line.len() <= 1 {
            return Borrowed(line);
        }
        // highlight matching brace/bracket/parenthesis if it exists
        if let Some((bracket, pos)) = self.bracket.get() {
            if let Some((matching, idx)) = find_matching_bracket(line, pos, bracket) {
                let mut copy = line.to_owned();
                copy.replace_range(idx..=idx, &format!("\x1b[1;34m{}\x1b[0m", matching as char));
                return Owned(copy);
            }
        }
        Borrowed(line)
    }

    fn highlight_char(&self, line: &str, pos: usize) -> bool {
        // will highlight matching brace/bracket/parenthesis if it exists
        self.bracket.set(check_bracket(line, pos));
        self.bracket.get().is_some()
    }
}

fn find_matching_bracket(line: &str, pos: usize, bracket: u8) -> Option<(u8, usize)> {
    let matching = matching_bracket(bracket);
    let mut idx;
    let mut unmatched = 1;
    if is_open_bracket(bracket) {
        // forward search
        idx = pos + 1;
        let bytes = &line.as_bytes()[idx..];
        for b in bytes {
            if *b == matching {
                unmatched -= 1;
                if unmatched == 0 {
                    debug_assert_eq!(matching, line.as_bytes()[idx]);
                    return Some((matching, idx));
                }
            } else if *b == bracket {
                unmatched += 1;
            }
            idx += 1;
        }
        debug_assert_eq!(idx, line.len());
    } else {
        // backward search
        idx = pos;
        let bytes = &line.as_bytes()[..idx];
        for b in bytes.iter().rev() {
            if *b == matching {
                unmatched -= 1;
                if unmatched == 0 {
                    debug_assert_eq!(matching, line.as_bytes()[idx - 1]);
                    return Some((matching, idx - 1));
                }
            } else if *b == bracket {
                unmatched += 1;
            }
            idx -= 1;
        }
        debug_assert_eq!(idx, 0);
    }
    None
}

// check under or before the cursor
fn check_bracket(line: &str, pos: usize) -> Option<(u8, usize)> {
    if line.is_empty() {
        return None;
    }
    let mut pos = pos;
    if pos >= line.len() {
        pos = line.len() - 1; // before cursor
        let b = line.as_bytes()[pos]; // previous byte
        if is_close_bracket(b) {
            Some((b, pos))
        } else {
            None
        }
    } else {
        let mut under_cursor = true;
        loop {
            let b = line.as_bytes()[pos];
            if is_close_bracket(b) {
                if pos == 0 {
                    return None;
                } else {
                    return Some((b, pos));
                }
            } else if is_open_bracket(b) {
                if pos + 1 == line.len() {
                    return None;
                } else {
                    return Some((b, pos));
                }
            } else if under_cursor && pos > 0 {
                under_cursor = false;
                pos -= 1; // or before cursor
            } else {
                return None;
            }
        }
    }
}

fn matching_bracket(bracket: u8) -> u8 {
    match bracket {
        b'{' => b'}',
        b'}' => b'{',
        b'[' => b']',
        b']' => b'[',
        b'(' => b')',
        b')' => b'(',
        b => b,
    }
}
fn is_open_bracket(bracket: u8) -> bool {
    memchr(bracket, OPENS).is_some()
}
fn is_close_bracket(bracket: u8) -> bool {
    memchr(bracket, CLOSES).is_some()
}

pub(crate) fn split_highlight<'a>(src: &'a str, offset: usize) -> (Cow<'a, str>, Cow<'a, str>) {
    let mut style_buffer = String::with_capacity(32);
    let mut iter = src.char_indices();
    let mut non_escape_idx = 0;
    while let Some((idx, c)) = iter.next() {
        if c == '\x1b' {
            match iter.next() {
                Some((_, '[')) => {}
                _ => continue, // unknown escape, skip
            }
            while let Some((end_idx, c)) = iter.next() {
                match c {
                    'm' => {
                        let slice = &src[idx..end_idx + 1];
                        if slice == "\x1b[0m" {
                            style_buffer.clear();
                        } else {
                            style_buffer.push_str(slice);
                        }
                        break;
                    }
                    ';' | '0'..='9' => continue,
                    _ => break, // unknown escape, skip
                }
            }
            continue;
        }
        if non_escape_idx >= offset {
            if style_buffer.is_empty() {
                return (src[..idx].into(), src[idx..].into());
            } else {
                let mut left = String::with_capacity(idx + 4);
                left.push_str(&src[..idx]);
                left.push_str("\x1b[0m");
                let mut right = String::with_capacity(src.len() - idx + style_buffer.len());
                right.push_str(&style_buffer);
                right.push_str(&src[idx..]);
                return (left.into(), right.into());
            }
        }
        non_escape_idx += c.len_utf8();
    }
    return (src.into(), "".into());
}

impl PromptInfo<'_> {
    /// Returns true if this is the default prompt
    pub fn default(&self) -> bool {
        self.default
    }

    /// Returns the byte offset where prompt is shown in the initial text
    ///
    /// This is a position right after the newline of the previous line
    pub fn line_offset(&self) -> usize {
        self.offset
    }

    /// Returns the byte position of the cursor relative to `line_offset` if
    /// the cursor is in the current line
    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    /// Returns the zero-based line number of the current prompt line
    pub fn line_no(&self) -> usize {
        self.line_no
    }

    /// Returns the line contents shown after the prompt
    pub fn line(&self) -> &str {
        self.line
    }

    /// Returns the whole input (equal to `line` if input is the single line)
    pub fn input(&self) -> &str {
        self.input
    }
}

#[cfg(test)]
mod tests {
    #[test]
    pub fn find_matching_bracket() {
        use super::find_matching_bracket;
        assert_eq!(find_matching_bracket("(...", 0, b'('), None);
        assert_eq!(find_matching_bracket("...)", 3, b')'), None);

        assert_eq!(find_matching_bracket("()..", 0, b'('), Some((b')', 1)));
        assert_eq!(find_matching_bracket("(..)", 0, b'('), Some((b')', 3)));

        assert_eq!(find_matching_bracket("..()", 3, b')'), Some((b'(', 2)));
        assert_eq!(find_matching_bracket("(..)", 3, b')'), Some((b'(', 0)));

        assert_eq!(find_matching_bracket("(())", 0, b'('), Some((b')', 3)));
        assert_eq!(find_matching_bracket("(())", 3, b')'), Some((b'(', 0)));
    }
    #[test]
    pub fn check_bracket() {
        use super::check_bracket;
        assert_eq!(check_bracket(")...", 0), None);
        assert_eq!(check_bracket("(...", 2), None);
        assert_eq!(check_bracket("...(", 3), None);
        assert_eq!(check_bracket("...(", 4), None);
        assert_eq!(check_bracket("..).", 4), None);

        assert_eq!(check_bracket("(...", 0), Some((b'(', 0)));
        assert_eq!(check_bracket("(...", 1), Some((b'(', 0)));
        assert_eq!(check_bracket("...)", 3), Some((b')', 3)));
        assert_eq!(check_bracket("...)", 4), Some((b')', 3)));
    }
    #[test]
    pub fn matching_bracket() {
        use super::matching_bracket;
        assert_eq!(matching_bracket(b'('), b')');
        assert_eq!(matching_bracket(b')'), b'(');
    }

    #[test]
    pub fn is_open_bracket() {
        use super::is_close_bracket;
        use super::is_open_bracket;
        assert!(is_open_bracket(b'('));
        assert!(is_close_bracket(b')'));
    }
}
