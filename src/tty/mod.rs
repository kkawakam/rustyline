//! This module implements and describes common TTY methods & traits
use std::io::Write;
use ::Result;
use consts::KeyPress;

pub trait RawMode: Copy + Sized {
    /// Disable RAW mode for the terminal.
    fn disable_raw_mode(&self) -> Result<()>;
}

pub trait RawReader: Sized {
    /// Blocking read of key pressed.
    fn next_key(&mut self, timeout_ms: i32) -> Result<KeyPress>;
    /// For CTRL-V support
    #[cfg(unix)]
    fn next_char(&mut self) -> Result<char>;
}

/// Terminal contract
pub trait Term: Clone {
    type Reader: RawReader;
    type Mode;

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
    fn create_reader(&self) -> Result<Self::Reader>;
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
#[cfg(all(unix, not(test)))]
mod unix;
#[cfg(all(unix, not(test)))]
pub use self::unix::*;

#[cfg(test)]
mod test;
#[cfg(test)]
pub use self::test::*;
