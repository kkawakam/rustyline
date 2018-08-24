///! Syntax highlighting
use std::borrow::Cow::{self, Borrowed};

/// Syntax highlighter with [ansi color](https://en.wikipedia.org/wiki/ANSI_escape_code#SGR_(Select_Graphic_Rendition)_parameters).
/// Rustyline will try to handle escape sequence for ansi color on windows
/// when not supported natively (windows <10).
///
/// TODO to be used
pub trait Highlighter {
    /// Takes the currently edited `line` with the cursor `pos`ition and
    /// returns the highlighted version (with ANSI color)
    /// and new cursor position which may have been shifted by highlighting.
    ///
    /// For example, you can implement
    /// [blink-matching-paren](https://www.gnu.org/software/bash/manual/html_node/Readline-Init-File-Syntax.html).
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> (Cow<'l, str>, usize) {
        (Borrowed(line), pos)
    }
    /// Takes the `prompt` and
    /// returns the highlighted version (with ANSI color).
    fn highlight_prompt<'p>(&self, prompt: &'p str) -> Cow<'p, str> {
        Borrowed(prompt)
    }
    /// Takes the `hint` and
    /// returns the highlighted version (with ANSI color).
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Borrowed(hint)
    }
    /// Takes the completion `canditate` and
    /// returns the highlighted version (with ANSI color).
    fn highlight_candidate<'c>(&self, candidate: &'c str) -> Cow<'c, str> {
        Borrowed(candidate)
    }
}

impl Highlighter for () {}
