/// Provide two versions of the prompt:
/// - the `raw` version used when `stdout` is not a tty, or when the terminal is
///   not supported or in `NO_COLOR` mode
/// - the `styled` version
pub trait Prompt {
    /// No style, no ANSI escape sequence
    fn raw(&self) -> &str;
    /// With style(s), ANSI escape sequences
    ///
    /// Currently, the styled version *must* have the same display width as
    /// the raw version.
    ///
    /// By default, returns the raw string.
    fn styled(&self) -> &str {
        self.raw()
    }
}

impl Prompt for str {
    fn raw(&self) -> &str {
        self
    }
}
impl Prompt for String {
    fn raw(&self) -> &str {
        self.as_str()
    }
}
impl<Raw: AsRef<str>, Styled: AsRef<str>> Prompt for (Raw, Styled) {
    fn raw(&self) -> &str {
        self.0.as_ref()
    }

    fn styled(&self) -> &str {
        self.1.as_ref()
    }
}
