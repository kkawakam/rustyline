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

/// Check to see if `fd` is a TTY
pub fn is_a_tty(fd: libc::c_int) -> bool {
    unsafe { libc::isatty(fd) != 0 }
}

