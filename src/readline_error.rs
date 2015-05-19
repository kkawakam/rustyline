use std::io;
use std::error;
use std::fmt;
use nix;

#[derive(Debug)]
pub enum ReadlineError {
    Io(io::Error),
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
