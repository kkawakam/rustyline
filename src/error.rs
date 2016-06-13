//! Contains error type for handling I/O and Errno errors
use std::io;
use std::error;
use std::fmt;
use nix;

use char_iter;

/// The error type for Rustyline errors that can arise from
/// I/O related errors or Errno when using the nix-rust library
#[derive(Debug)]
pub enum ReadlineError {
    /// I/O Error
    Io(io::Error),
    /// Chars Error
    Char(char_iter::CharsError),
    /// EOF (Ctrl-d)
    Eof,
    /// Ctrl-C
    Interrupted,
    /// Unix Error from syscall
    #[cfg(unix)]
    Errno(nix::Error),
}

impl fmt::Display for ReadlineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ReadlineError::Io(ref err) => err.fmt(f),
            ReadlineError::Char(ref err) => err.fmt(f),
            ReadlineError::Eof => write!(f, "EOF"),
            ReadlineError::Interrupted => write!(f, "Interrupted"),
            #[cfg(unix)]
            ReadlineError::Errno(ref err) => write!(f, "Errno: {}", err.errno().desc()),
        }
    }
}

impl error::Error for ReadlineError {
    fn description(&self) -> &str {
        match *self {
            ReadlineError::Io(ref err) => err.description(),
            ReadlineError::Char(ref err) => err.description(),
            ReadlineError::Eof => "EOF",
            ReadlineError::Interrupted => "Interrupted",
            #[cfg(unix)]
            ReadlineError::Errno(ref err) => err.errno().desc(),
        }
    }
}

impl From<io::Error> for ReadlineError {
    fn from(err: io::Error) -> ReadlineError {
        ReadlineError::Io(err)
    }
}

#[cfg(unix)]
impl From<nix::Error> for ReadlineError {
    fn from(err: nix::Error) -> ReadlineError {
        ReadlineError::Errno(err)
    }
}

impl From<char_iter::CharsError> for ReadlineError {
    fn from(err: char_iter::CharsError) -> ReadlineError {
        ReadlineError::Char(err)
    }
}

impl ReadlineError {
    #[cfg(unix)]
    pub fn from_errno(errno: nix::errno::Errno) -> ReadlineError {
        ReadlineError::from(nix::Error::from_errno(errno))
    }
}
