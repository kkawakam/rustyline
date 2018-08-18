///! Syntax highlighting
use std::borrow::Cow::{self, Borrowed};

/// Syntax highlighter with [ansi color](https://en.wikipedia.org/wiki/ANSI_escape_code#SGR_(Select_Graphic_Rendition)_parameters).
/// Rustyline will try to handle escape sequence for ansi color on windows
/// when not supported natively (windows <10). TODO to be used
pub trait Highlighter {
    /// Takes the currently edited `line` with the cursor `_pos`ition and
    /// returns the highlighted version (with ANSI color).
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        Borrowed(line)
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
    fn highlight_canidate<'c>(&self, candidate: &'c str) -> Cow<'c, str> {
        Borrowed(candidate)
    }
}

impl Highlighter for () {}
