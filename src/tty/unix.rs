extern crate nix;
extern crate libc;

use std;
use nix::sys::termios;
use nix::errno::Errno;
use super::Result;
use super::tty_common;
use super::error;

/// Unsupported Terminals that don't support RAW mode
static UNSUPPORTED_TERM: [&'static str; 3] = ["dumb", "cons25", "emacs"];

/// Check to see if the current `TERM` is unsupported in unix
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

/// Enable raw mode for the TERM
pub fn enable_raw_mode() -> Result<termios::Termios> {
    use nix::sys::termios::{BRKINT, CS8, ECHO, ICANON, ICRNL, IEXTEN, INPCK, ISIG, ISTRIP, IXON,
                            OPOST, VMIN, VTIME};
    if !tty_common::is_a_tty(libc::STDIN_FILENO) {
        return Err(error::ReadlineError::from_errno(Errno::ENOTTY));
    }
    let original_term = try!(termios::tcgetattr(libc::STDIN_FILENO));
    let mut raw = original_term;
    raw.c_iflag = raw.c_iflag & !(BRKINT | ICRNL | INPCK | ISTRIP | IXON); // disable BREAK interrupt, CR to NL conversion on input, input parity check, strip high bit (bit 8), output flow control
    raw.c_oflag = raw.c_oflag & !(OPOST); // disable all output processing
    raw.c_cflag = raw.c_cflag | (CS8); // character-size mark (8 bits)
    raw.c_lflag = raw.c_lflag & !(ECHO | ICANON | IEXTEN | ISIG); // disable echoing, canonical mode, extended input processing and signals
    raw.c_cc[VMIN] = 1; // One character-at-a-time input
    raw.c_cc[VTIME] = 0; // with blocking read
    try!(termios::tcsetattr(libc::STDIN_FILENO, termios::TCSAFLUSH, &raw));
    Ok(original_term)
}

/// Disable Raw mode for the term
pub fn disable_raw_mode(original_termios: termios::Termios) -> Result<()> {
    try!(termios::tcsetattr(libc::STDIN_FILENO, termios::TCSAFLUSH, &original_termios));
    Ok(())
}
