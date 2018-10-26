//! Syntax highlighting

use config::CompletionType;
use memchr::memchr;
use std::borrow::Cow::{self, Borrowed, Owned};

/// Syntax highlighter with [ansi color](https://en.wikipedia.org/wiki/ANSI_escape_code#SGR_(Select_Graphic_Rendition)_parameters).
/// Rustyline will try to handle escape sequence for ansi color on windows
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
    fn highlight_prompt<'p>(&self, prompt: &'p str) -> Cow<'p, str> {
        Borrowed(prompt)
    }
    /// Takes the dynamic `prompt` and
    /// returns the highlighted version (with ANSI color).
    #[deprecated(since = "2.0.1", note = "please use `highlight_prompt` instead")]
    fn highlight_dynamic_prompt<'p>(&self, prompt: &'p str) -> Cow<'p, str> {
        Borrowed(prompt)
    }
    /// Takes the `hint` and
    /// returns the highlighted version (with ANSI color).
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Borrowed(hint)
    }
    /// Takes the completion `canditate` and
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
    /// Tells if the `ch`ar needs to be highlighted when typed or when cursor
    /// is moved under.
    ///
    /// Used to optimize refresh when a character is inserted or the cursor is
    /// moved.
    fn highlight_char(&self, grapheme: &str) -> bool {
        let _ = grapheme;
        false
    }
}

impl Highlighter for () {}

static OPENS: &'static [u8; 3] = b"{[(";
static CLOSES: &'static [u8; 3] = b"}])";

pub struct MatchingBracketHighlihter {}

impl Highlighter for MatchingBracketHighlihter {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        if line.len() <= 1 {
            return Borrowed(line);
        }
        // highlight matching brace/bracket/parenthese if it exists
        if let Some((bracket, pos)) = check_bracket(line, pos) {
            if let Some((matching, idx)) = find_matching_bracket(line, pos, bracket) {
                let mut copy = line.to_owned();
                copy.replace_range(idx..=idx, &format!("\x1b[1;34m{}\x1b[0m", matching as char));
                return Owned(copy);
            }
        }
        Borrowed(line)
    }

    fn highlight_char(&self, grapheme: &str) -> bool {
        // will highlight matching brace/bracket/parenthese if it exists
        if grapheme.len() != 1 {
            return false;
        }
        // TODO we can't memorize the character to search...
        let b = grapheme.as_bytes()[0];
        is_open_bracket(b) || is_close_bracket(b)
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
