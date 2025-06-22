//! Contains error type for handling I/O and Errno errors
#[cfg(windows)]
use std::char;
use std::error::Error;
use std::fmt;
use std::io;

/// The error type for Rustyline errors that can arise from
/// I/O related errors or Errno when using the nix-rust library
// #[non_exhaustive]
#[expect(clippy::module_name_repetitions)]
#[derive(Debug)]
#[non_exhaustive]
pub enum ReadlineError {
    /// I/O Error
    Io(io::Error),
    /// EOF (VEOF / Ctrl-D)
    Eof,
    /// Interrupt signal (VINTR / VQUIT / Ctrl-C)
    Interrupted,
    /// Unix Error from syscall
    #[cfg(unix)]
    Errno(nix::Error),
    /// Error generated on `WINDOW_BUFFER_SIZE_EVENT` / `SIGWINCH` signal
    Signal(Signal),
    /// Like `Utf8Error` on unix
    #[cfg(windows)]
    Decode(char::DecodeUtf16Error),
    /// Something went wrong calling a Windows API
    #[cfg(windows)]
    SystemError(clipboard_win::ErrorCode),
    /// Error related to SQLite history backend
    #[cfg(feature = "with-sqlite-history")]
    SQLiteError(rusqlite::Error),
}

impl fmt::Display for ReadlineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Io(ref err) => err.fmt(f),
            Self::Eof => write!(f, "EOF"),
            Self::Interrupted => write!(f, "Interrupted"),
            #[cfg(unix)]
            Self::Errno(ref err) => err.fmt(f),
            Self::Signal(ref sig) => write!(f, "Signal({sig:?})"),
            #[cfg(windows)]
            Self::Decode(ref err) => err.fmt(f),
            #[cfg(windows)]
            Self::SystemError(ref err) => err.fmt(f),
            #[cfg(feature = "with-sqlite-history")]
            Self::SQLiteError(ref err) => err.fmt(f),
        }
    }
}

impl Error for ReadlineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            Self::Io(ref err) => Some(err),
            Self::Eof => None,
            Self::Interrupted => None,
            #[cfg(unix)]
            Self::Errno(ref err) => Some(err),
            Self::Signal(_) => None,
            #[cfg(windows)]
            Self::Decode(ref err) => Some(err),
            #[cfg(windows)]
            Self::SystemError(_) => None,
            #[cfg(feature = "with-sqlite-history")]
            Self::SQLiteError(ref err) => Some(err),
        }
    }
}

/// Signal received from terminal
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Signal {
    /// SIGINT
    #[cfg(unix)]
    Interrupt,
    /// SIGWINCH / `WINDOW_BUFFER_SIZE_EVENT`
    Resize,
}

#[cfg(unix)]
impl Signal {
    #[cfg(not(feature = "signal-hook"))]
    pub(crate) fn from(b: u8) -> Self {
        match b {
            b'I' => Self::Interrupt,
            b'W' => Self::Resize,
            _ => unreachable!(),
        }
    }

    #[cfg(feature = "signal-hook")]
    pub(crate) fn from(_: u8) -> Self {
        Self::Resize
    }

    #[cfg(not(feature = "signal-hook"))]
    pub(crate) fn to_byte(sig: libc::c_int) -> u8 {
        match sig {
            libc::SIGINT => b'I',
            libc::SIGWINCH => b'W',
            _ => unreachable!(),
        }
    }
}

#[cfg(unix)]
#[derive(Debug)]
pub(crate) struct SignalError(pub Signal);
#[cfg(unix)]
impl fmt::Display for SignalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Signal({:?})", self.0)
    }
}
#[cfg(unix)]
impl Error for SignalError {}

impl From<io::Error> for ReadlineError {
    fn from(err: io::Error) -> Self {
        #[cfg(unix)]
        if err.kind() == io::ErrorKind::Interrupted {
            if let Some(e) = err.get_ref() {
                if let Some(se) = e.downcast_ref::<SignalError>() {
                    return Self::Signal(se.0);
                }
            }
        }
        Self::Io(err)
    }
}

impl From<io::ErrorKind> for ReadlineError {
    fn from(kind: io::ErrorKind) -> Self {
        Self::Io(io::Error::from(kind))
    }
}

#[cfg(unix)]
impl From<nix::Error> for ReadlineError {
    fn from(err: nix::Error) -> Self {
        Self::Errno(err)
    }
}

#[cfg(windows)]
impl From<char::DecodeUtf16Error> for ReadlineError {
    fn from(err: char::DecodeUtf16Error) -> Self {
        Self::Io(io::Error::new(io::ErrorKind::InvalidData, err))
    }
}

#[cfg(windows)]
impl From<std::string::FromUtf8Error> for ReadlineError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self::Io(io::Error::new(io::ErrorKind::InvalidData, err))
    }
}

#[cfg(unix)]
impl From<fmt::Error> for ReadlineError {
    fn from(err: fmt::Error) -> Self {
        Self::Io(io::Error::other(err))
    }
}

#[cfg(windows)]
impl From<clipboard_win::ErrorCode> for ReadlineError {
    fn from(err: clipboard_win::ErrorCode) -> Self {
        Self::SystemError(err)
    }
}

#[cfg(feature = "with-sqlite-history")]
impl From<rusqlite::Error> for ReadlineError {
    fn from(err: rusqlite::Error) -> Self {
        Self::SQLiteError(err)
    }
}
