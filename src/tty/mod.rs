//! This module implements and describes common TTY methods & traits
extern crate libc;
use super::Result;

// If on Windows platform import Windows TTY module 
// and re-export into mod.rs scope
#[cfg(windows)] mod windows;
#[cfg(windows)] pub use self::windows::*;

// If on Unix platform import Unix TTY module 
// and re-export into mod.rs scope
#[cfg(unix)] mod unix;
#[cfg(unix)] pub use self::unix::*;

/// Trait that should be for each TTY/Terminal on various platforms
/// (e.g. unix & windows)
pub trait Terminal {
    /// Enable RAW mode for the terminal
    fn enable_raw_mode(&mut self) -> Result<()>;

    /// Disable RAW mode for the terminal
    fn disable_raw_mode(&self) -> Result<()>;
}

/// Enum for Standard Streams 
///
/// libc::STDIN_FILENO/STDOUT_FILENO/STDERR_FILENO is not defined for the
/// Windows platform.  We will use this enum instead when calling isatty
/// function
pub enum StandardStream {
    StdIn,
    StdOut,
}
