//! This module implements and describes common TTY methods & traits
use std::io::Write;
use Result;
use config::Config;
use consts::KeyPress;

/// Terminal state
pub trait RawMode: Copy + Sized {
    /// Disable RAW mode for the terminal.
    fn disable_raw_mode(&self) -> Result<()>;
}

/// Translate bytes read from stdin to keys.
pub trait RawReader: Sized {
    /// Blocking read of key pressed.
    fn next_key(&mut self) -> Result<KeyPress>;
    /// For CTRL-V support
    #[cfg(unix)]
    fn next_char(&mut self) -> Result<char>;
}

/// Terminal contract
pub trait Term: Clone {
    type Reader: RawReader;
    type Writer: Write;
    type Mode: RawMode;

    fn new() -> Self;
    /// Check if current terminal can provide a rich line-editing user interface.
    fn is_unsupported(&self) -> bool;
    /// check if stdin is connected to a terminal.
    fn is_stdin_tty(&self) -> bool;
    /// Get the number of columns in the current terminal.
    fn get_columns(&self) -> usize;
    /// Get the number of rows in the current terminal.
    fn get_rows(&self) -> usize;
    /// Check if a SIGWINCH signal has been received
    fn sigwinch(&self) -> bool;
    /// Enable RAW mode for the terminal.
    fn enable_raw_mode(&self) -> Result<Self::Mode>;
    /// Create a RAW reader
    fn create_reader(&self, config: &Config) -> Result<Self::Reader>;
    /// Create a writer
    fn create_writer(&self) -> Self::Writer;
    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self, w: &mut Write) -> Result<()>;
}

// If on Windows platform import Windows TTY module
// and re-export into mod.rs scope
#[cfg(all(windows, not(test)))]
mod windows;
#[cfg(all(windows, not(test)))]
pub use self::windows::*;

// If on Unix platform import Unix TTY module
// and re-export into mod.rs scope
#[cfg(all(unix, not(any(test, target_os = "fuchsia"))))]
mod unix;
#[cfg(all(unix, not(any(test, target_os = "fuchsia"))))]
pub use self::unix::*;

// If on a Fuchsia platform import Fuchsia TTY module
// and re-export into mod.rs scope
#[cfg(target_os = "fuchsia")]
mod fuchsia;
#[cfg(target_os = "fuchsia")]
pub use self::fuchsia::*;

#[cfg(test)]
mod test;
#[cfg(test)]
pub use self::test::*;
