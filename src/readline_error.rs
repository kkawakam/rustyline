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
    Errno(nix::Error)
}

impl fmt::Display for ReadlineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ReadlineError::Io(ref err) => write!(f, "IO error: {}", err),
            ReadlineError::Errno(ref err) => write!(f, "Errno: {}", err.errno().desc())
        }
    }
}

impl error::Error for ReadlineError {
    fn description(&self) -> &str {
        match *self {
            ReadlineError::Io(ref err) => err.description(),
            ReadlineError::Errno(ref err) => err.errno().desc()
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
