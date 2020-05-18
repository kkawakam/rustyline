//! Contains error type for handling I/O and Errno errors
#[cfg(windows)]
use std::char;
use std::error;
use std::fmt;
use std::io;

/// The error type for Rustyline errors that can arise from
/// I/O related errors or Errno when using the nix-rust library
// #[non_exhaustive]
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
#[non_exhaustive]
pub enum ReadlineError {
    /// I/O Error
    Io(io::Error),
    /// EOF (Ctrl-D)
    Eof,
    /// Ctrl-C
    Interrupted,
    /// Unix Error from syscall
    #[cfg(unix)]
    Errno(nix::Error),
    /// Error generated on WINDOW_BUFFER_SIZE_EVENT to mimic unix SIGWINCH signal
    #[cfg(windows)]
    WindowResize,
}

impl fmt::Display for ReadlineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ReadlineError::Io(ref err) => err.fmt(f),
            ReadlineError::Eof => write!(f, "EOF"),
            ReadlineError::Interrupted => write!(f, "Interrupted"),
            #[cfg(unix)]
            ReadlineError::Errno(ref err) => err.fmt(f),
            #[cfg(windows)]
            ReadlineError::WindowResize => write!(f, "WindowResize"),
        }
    }
}

impl error::Error for ReadlineError {}

impl From<io::Error> for ReadlineError {
    fn from(err: io::Error) -> Self {
        ReadlineError::Io(err)
    }
}

impl From<io::ErrorKind> for ReadlineError {
    fn from(kind: io::ErrorKind) -> Self {
        ReadlineError::Io(io::Error::from(kind))
    }
}

#[cfg(unix)]
impl From<nix::Error> for ReadlineError {
    fn from(err: nix::Error) -> Self {
        ReadlineError::Errno(err)
    }
}

#[cfg(windows)]
impl From<char::DecodeUtf16Error> for ReadlineError {
    fn from(err: char::DecodeUtf16Error) -> Self {
        ReadlineError::Io(io::Error::new(io::ErrorKind::InvalidData, err))
    }
}

#[cfg(windows)]
impl From<std::string::FromUtf8Error> for ReadlineError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        ReadlineError::Io(io::Error::new(io::ErrorKind::InvalidData, err))
    }
}

#[cfg(unix)]
impl From<fmt::Error> for ReadlineError {
    fn from(err: fmt::Error) -> Self {
        ReadlineError::Io(io::Error::new(io::ErrorKind::Other, err))
    }
}
