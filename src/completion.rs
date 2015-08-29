//! Completion API

/// To be called for tab-completion.
pub trait Completer {
    /// Takes the currently edited `line` with the cursor `pos`ition and
    /// returns the completion candidates for the partial word to be completed.
    fn complete(&self, line: &str, pos: usize) -> Vec<String>;
    /// Takes the currently edited `line` with the cursor `pos`ition and
    /// the `elected` candidate.
    /// Returns the new line content and cursor position.
    fn update(&self, _line: &str, _pos: usize, elected: &str) -> (String, usize) {
        // line completion (vs word completion)
        (String::from(elected), elected.len())
    }
}
