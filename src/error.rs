//! Contains error type for handling I/O and Errno errors
use std::io;
use std::error;
use std::fmt;
use nix;

/// The error type for Rustyline errors that can arise from
/// I/O related errors or Errno when using the nix-rust library
#[derive(Debug)]
pub enum ReadlineError {
    /// I/O Error
    Io(io::Error),
    /// Error from syscall
    Errno(nix::Error),
    /// Chars Error
    Char(io::CharsError),
    /// EOF (Ctrl-d)
    Eof,
    /// Ctrl-C
    Interrupted,
}

impl fmt::Display for ReadlineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ReadlineError::Io(ref err) => err.fmt(f),
            ReadlineError::Errno(ref err) => write!(f, "Errno: {}", err.errno().desc()),
            ReadlineError::Char(ref err) => err.fmt(f),
            ReadlineError::Eof => write!(f, "EOF"),
            ReadlineError::Interrupted => write!(f, "Interrupted"),
        }
    }
}

impl error::Error for ReadlineError {
    fn description(&self) -> &str {
        match *self {
            ReadlineError::Io(ref err) => err.description(),
            ReadlineError::Errno(ref err) => err.errno().desc(),
            ReadlineError::Char(ref err) => err.description(),
            ReadlineError::Eof => "EOF",
            ReadlineError::Interrupted => "Interrupted",
        }
    }
}

impl From<io::Error> for ReadlineError {
    fn from(err: io::Error) -> ReadlineError {
        ReadlineError::Io(err)
    }
}

impl From<nix::Error> for ReadlineError {
    fn from(err: nix::Error) -> ReadlineError {
        ReadlineError::Errno(err)
    }
}

impl From<io::CharsError> for ReadlineError {
    fn from(err: io::CharsError) -> ReadlineError {
        ReadlineError::Char(err)
    }
}
