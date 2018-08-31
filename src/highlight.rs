//! Syntax highlighting

use config::CompletionType;
use std::borrow::Cow::{self, Borrowed};

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
    #[deprecated(
        since = "2.0.1",
        note = "please use `highlight_prompt` instead"
    )]
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
