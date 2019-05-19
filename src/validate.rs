//! Input validation API (Multi-line editing)

use crate::line_buffer::LineBuffer;

/// Input validation result
pub enum ValidationResult {
    /// Incomplete input
    Incomplete,
    /// Validation fails with an optional error message. User must fix the
    /// input.
    Invalid(Option<String>),
    /// Validation succeeds with an optional message (instead of https://github.com/kkawakam/rustyline/pull/169)
    Valid(Option<String>),
}

/// This trait provides an extension interface for determining whether
/// the current input buffer is valid. Rustyline uses the method
/// provided by this trait to decide whether hitting the enter key
/// will end the current editing session and return the current line
/// buffer to the caller of `Editor::readline` or variants.
pub trait Validator {
    /// Takes the currently edited `line` and returns a
    /// `ValidationResult` indicating whether it is valid or not along
    /// with an option message to display about the result. The most
    /// common validity check to implement is probably whether the
    /// input is complete or not, for instance ensuring that all
    /// delimiters are fully balanced.
    ///
    /// If you implement more complex validation checks it's probably
    /// a good idea to also implement a `Hinter` to provide feedback
    /// about what is invalid.
    ///
    /// For auto-correction like a missing closing quote or to reject invalid
    /// char while typing, the `line` is mutable.
    fn validate(&self, line: &mut LineBuffer) -> ValidationResult {
        let _ = line;
        ValidationResult::Valid(None)
    }

    /// Configure whether validation is performed while typing or only
    /// when user presses the Enter key.
    ///
    /// Default is `false`.
    // TODO we can implement this later.
    fn validate_while_typing(&self) -> bool {
        false
    }
}

impl Validator for () {}

impl<'v, V: ?Sized + Validator> Validator for &'v V {
    fn validate(&self, line: &mut LineBuffer) -> ValidationResult {
        (**self).validate(line)
    }

    fn validate_while_typing(&self) -> bool {
        (**self).validate_while_typing()
    }
}
