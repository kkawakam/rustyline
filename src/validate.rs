//! Input buffer validation API (Multi-line editing)

/// This trait provides an extension interface for determining whether
/// the current input buffer is valid. Rustyline uses the method
/// provided by this trait to decide whether hitting the enter key
/// will end the current editing session and return the current line
/// buffer to the caller of `Editor::readline` or variants.
pub trait Validator {
    /// Takes the currently edited `line` and returns a bool
    /// indicating whether it is valid or not. The most common
    /// validity check to implement is probably whether the input is
    /// complete or not, for instance ensuring that all delimiters are
    /// fully balanced.
    ///
    /// If you implement more complex validation checks it's probably
    /// a good idea to also implement a `Hinter` to provide feedback
    /// about what is invalid.
    #[allow(unused_variables)]
    fn is_valid(&self, line: &str) -> bool {
        true
    }
}

impl Validator for () {}
