//! Tokenizer/parser used for both completion, suggestion, highlighting.
//! (parse current line once)

use crate::line_buffer::{ChangeListener, DeleteListener};

/// Input parser
pub trait Parser: ChangeListener {
    /// Parsed user input line(s)
    type Document;
    /// Update document after user change(s)
    fn update(&mut self, line: &str);
    /// Parsed user input line(s)
    fn document(&self) -> &Self::Document;
}
impl Parser for () {
    type Document = ();

    fn update(&mut self, _: &str) {}

    fn document(&self) -> &Self::Document {
        &()
    }
}
impl ChangeListener for () {}
impl DeleteListener for () {}
