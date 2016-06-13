extern crate nix;
extern crate libc;

use std;
use nix::sys::termios;
use nix::errno::Errno;
use super::Terminal;
use super::StandardStream;
use ::Result;
use ::error;

/// Unsupported Terminals that don't support RAW mode
static UNSUPPORTED_TERM: [&'static str; 3] = ["dumb", "cons25", "emacs"];

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
const TIOCGWINSZ: libc::c_ulong = 0x40087468;

#[cfg(any(all(target_os = "linux", target_env = "gnu"), target_os = "android"))]
const TIOCGWINSZ: libc::c_ulong = 0x5413;

#[cfg(all(target_os = "linux", target_env = "musl"))]
const TIOCGWINSZ: libc::c_int = 0x5413;

/// Try to get the number of columns in the current terminal,
/// or assume 80 if it fails.
pub fn get_columns() -> usize {
    use std::mem::zeroed;
    use libc::c_ushort;
    use libc;

    unsafe {
        #[repr(C)]
        struct winsize {
            ws_row: c_ushort,
            ws_col: c_ushort,
            ws_xpixel: c_ushort,
            ws_ypixel: c_ushort,
        }

        let mut size: winsize = zeroed();
        match libc::ioctl(libc::STDOUT_FILENO, TIOCGWINSZ, &mut size) {
            0 => size.ws_col as usize, // TODO getCursorPosition
            _ => 80,
        }
    }
}

/// Get UnixTerminal struct
pub fn get_terminal() -> UnixTerminal {
    UnixTerminal{ original_termios: None }
}

/// Check TERM environment variable to see if current term is in our
/// unsupported list
pub fn is_unsupported_term() -> bool {
    use std::ascii::AsciiExt;
    match std::env::var("TERM") {
        Ok(term) => {
            let mut unsupported = false;
            for iter in &UNSUPPORTED_TERM {
                unsupported = (*iter).eq_ignore_ascii_case(&term)
            }
            unsupported
        }
        Err(_) => false,
    }
}


/// Return whether or not STDIN, STDOUT or STDERR is a TTY
pub fn is_a_tty(stream: StandardStream) -> bool {
    extern crate libc;

    let fd = match stream {
            StandardStream::StdIn => libc::STDIN_FILENO,
            StandardStream::StdOut => libc::STDOUT_FILENO,
        };

    unsafe { libc::isatty(fd) != 0 }
}

/// Structure that will contain the original termios before enabling RAW mode
pub struct UnixTerminal {
    original_termios: Option<termios::Termios>
}

impl Terminal for UnixTerminal {
    /// Enable raw mode for the TERM
    fn enable_raw_mode(&mut self) -> Result<()> {
        use nix::sys::termios::{BRKINT, CS8, ECHO, ICANON, ICRNL, IEXTEN, INPCK, ISIG, ISTRIP, IXON,
                                OPOST, VMIN, VTIME};
        if !is_a_tty(StandardStream::StdIn) {
            return Err(error::ReadlineError::from_errno(Errno::ENOTTY));
        }
        let original_termios = try!(termios::tcgetattr(libc::STDIN_FILENO));
        let mut raw = original_termios; 
        // disable BREAK interrupt, CR to NL conversion on input, 
        // input parity check, strip high bit (bit 8), output flow control
        raw.c_iflag = raw.c_iflag & !(BRKINT | ICRNL | INPCK | ISTRIP | IXON);
        raw.c_oflag = raw.c_oflag & !(OPOST); // disable all output processing
        raw.c_cflag = raw.c_cflag | (CS8); // character-size mark (8 bits)
        // disable echoing, canonical mode, extended input processing and signals
        raw.c_lflag = raw.c_lflag & !(ECHO | ICANON | IEXTEN | ISIG);
        raw.c_cc[VMIN] = 1; // One character-at-a-time input
        raw.c_cc[VTIME] = 0; // with blocking read
        try!(termios::tcsetattr(libc::STDIN_FILENO, termios::TCSAFLUSH, &raw));

        // Set the original terminal to the struct field
        self.original_termios = Some(original_termios);
        Ok(())
    }

    /// Disable Raw mode for the term
    fn disable_raw_mode(&self) -> Result<()> {
        try!(termios::tcsetattr(libc::STDIN_FILENO,
                                termios::TCSAFLUSH,
                                &self.original_termios.expect("RAW MODE was not enabled previously")));
        Ok(())
    }
}

/// Ensure that RAW mode is disabled even in the case of a panic!
#[allow(unused_must_use)]
impl Drop for UnixTerminal {
    fn drop(&mut self) {
        self.disable_raw_mode();
    }
}
