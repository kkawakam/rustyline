//! This module implements and describes common TTY methods & traits
use ::Result;
use consts::KeyPress;

pub trait RawReader: Sized {
    fn next_key(&mut self) -> Result<KeyPress>;
    #[cfg(unix)]
    fn next_char(&mut self) -> Result<char>;
}

// If on Windows platform import Windows TTY module
// and re-export into mod.rs scope
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

// If on Unix platform import Unix TTY module
// and re-export into mod.rs scope
#[cfg(unix)]mod unix;
#[cfg(unix)]
pub use self::unix::*;
