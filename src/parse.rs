//! Parser API
use core::fmt;

/// This trait provides an extension interface for tokenizer/parser.
///
/// Rustyline will call [Parser::parse] when the line is modified.
/// You can put the tokenizer/parser here, store the parsed results into
/// `Helper` (`&mut self` field in [Parser::parse]), so that other
/// Rustyline functions access the parsed results.
///
/// TODO: Make possible to do incremental parsing by providing the change.
pub trait Parser {
    /// Parse and update helper itself when line has been modified.
    ///
    /// This is the first-called function
    /// (that is, before all other functions within [Completer](crate::completion::Completer),
    /// [Hinter](crate::hint::Hinter), [Highlighter](crate::highlight::Highlighter), and [Validator](crate::validate::Validator))
    /// just after the line is modified.
    ///
    /// You can put the tokenizer/parser here, store the parsed results into
    /// `Helper` (`&mut self` field), so that other
    /// Rustyline functions access the parsed results.
    ///
    /// TODO: Rightnow the provided `change: InputEdit` is invalid/empty
    fn parse(&mut self, line: &str, change: InputEdit) {
        _ = (line, change);
    }
}

impl Parser for () {}

impl<'p, P: ?Sized + Parser> Parser for &'p mut P {
    fn parse(&mut self, line: &str, change: InputEdit) {
        (**self).parse(line, change);
    }
}

/// A position in a multi-line text document, in terms of rows and columns.
///
/// Rows and columns are zero-based.
///
/// reference to [tree-sitter](https://docs.rs/crate/tree-sitter/0.24.1/source/binding_rust/lib.rs#69-76)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Point {
    /// row
    pub row: usize,
    /// column
    pub column: usize,
}

/// A summary of a change to a text document.
/// reference to [tree-sitter](https://docs.rs/crate/tree-sitter/0.24.1/source/binding_rust/lib.rs#88-97)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputEdit {
    /// start_byte
    pub start_byte: usize,
    /// old_end_byte
    pub old_end_byte: usize,
    /// new_end_byte
    pub new_end_byte: usize,
    /// start_position
    pub start_position: Point,
    /// old_end_position
    pub old_end_position: Point,
    /// new_end_position
    pub new_end_position: Point,
}

impl Point {
    /// Create a new Point
    #[must_use]
    pub const fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.row, self.column)
    }
}
