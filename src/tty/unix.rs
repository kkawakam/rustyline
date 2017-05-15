//! Unix specific definitions
use std;
use std::io::{self, Read, Write};
use std::sync;
use std::sync::atomic;
use libc;
use nix;
use nix::poll;
use nix::sys::signal;
use nix::sys::termios;

use char_iter;
use consts::{self, KeyPress};
use ::Result;
use ::error;
use super::{RawMode, RawReader, Term};

const STDIN_FILENO: libc::c_int = libc::STDIN_FILENO;
const STDOUT_FILENO: libc::c_int = libc::STDOUT_FILENO;

/// Unsupported Terminals that don't support RAW mode
static UNSUPPORTED_TERM: [&'static str; 3] = ["dumb", "cons25", "emacs"];

fn get_win_size() -> (usize, usize) {
    use std::mem::zeroed;

    unsafe {
        let mut size: libc::winsize = zeroed();
        match libc::ioctl(STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) {
            0 => (size.ws_col as usize, size.ws_row as usize), // TODO getCursorPosition
            _ => (80, 24),
        }
    }
}

/// Check TERM environment variable to see if current term is in our
/// unsupported list
fn is_unsupported_term() -> bool {
    use std::ascii::AsciiExt;
    match std::env::var("TERM") {
        Ok(term) => {
            for iter in &UNSUPPORTED_TERM {
                if (*iter).eq_ignore_ascii_case(&term) {
                    return true;
                }
            }
            false
        }
        Err(_) => false,
    }
}


/// Return whether or not STDIN, STDOUT or STDERR is a TTY
fn is_a_tty(fd: libc::c_int) -> bool {
    unsafe { libc::isatty(fd) != 0 }
}

pub type Mode = termios::Termios;

impl RawMode for Mode {
    /// Disable RAW mode for the terminal.
    fn disable_raw_mode(&self) -> Result<()> {
        try!(termios::tcsetattr(STDIN_FILENO, termios::TCSADRAIN, self));
        Ok(())
    }
}

// Rust std::io::Stdin is buffered with no way to know if bytes are available.
// So we use low-level stuff instead...
struct StdinRaw {}

impl Read for StdinRaw {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let res = unsafe {
                libc::read(STDIN_FILENO,
                           buf.as_mut_ptr() as *mut libc::c_void,
                           buf.len() as libc::size_t)
            };
            if res == -1 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error);
                }
            } else {
                return Ok(res as usize);
            }
        }
    }
}

/// Console input reader
pub struct PosixRawReader {
    chars: char_iter::Chars<StdinRaw>,
}

impl PosixRawReader {
    pub fn new() -> Result<PosixRawReader> {
        let stdin = StdinRaw {};
        Ok(PosixRawReader { chars: char_iter::chars(stdin) })
    }

    fn escape_sequence(&mut self) -> Result<KeyPress> {
        // Read the next two bytes representing the escape sequence.
        let seq1 = try!(self.next_char());
        if seq1 == '[' {
            // ESC [ sequences.
            let seq2 = try!(self.next_char());
            if seq2.is_digit(10) {
                // Extended escape, read additional byte.
                let seq3 = try!(self.next_char());
                if seq3 == '~' {
                    match seq2 {
                        '1' => Ok(KeyPress::Home), // xterm
                        '3' => Ok(KeyPress::Delete),
                        '4' => Ok(KeyPress::End), // xterm
                        '5' => Ok(KeyPress::PageUp),
                        '6' => Ok(KeyPress::PageDown),
                        '7' => Ok(KeyPress::Home),
                        '8' => Ok(KeyPress::End),
                        _ => Ok(KeyPress::UnknownEscSeq),
                    }
                } else if seq2 == '1' && seq3 == ';' {
                    let seq4 = try!(self.next_char());
                    if seq4 == '5' {
                        let seq5 = try!(self.next_char());
                        match seq5 {
                            // 'A' => Ok(KeyPress::CtrlUp),
                            // 'B' => Ok(KeyPress::CtrlDown),
                            'C' => Ok(KeyPress::CtrlRight),
                            'D' => Ok(KeyPress::CtrlLeft),
                            // 'F' => Ok(KeyPress::CtrlEnd),
                            // 'H' => Ok(KeyPress::CtrlHome),
                            _ => Ok(KeyPress::UnknownEscSeq),
                        }
                    } else if seq4 == '3' {
                        let seq5 = try!(self.next_char());
                        match seq5 {
                            // 'A' => Ok(KeyPress::AltUp),
                            // 'B' => Ok(KeyPress::AltDown),
                            // 'C' => Ok(KeyPress::AltRight),
                            // 'D' => Ok(KeyPress::AltLeft),
                            // 'F' => Ok(KeyPress::AltEnd),
                            // 'H' => Ok(KeyPress::AltHome),
                            _ => Ok(KeyPress::UnknownEscSeq),
                        }
                    } else {
                        Ok(KeyPress::UnknownEscSeq)
                    }
                } else {
                    Ok(KeyPress::UnknownEscSeq)
                }
            } else {
                match seq2 {
                    'A' => Ok(KeyPress::Up), // ANSI
                    'B' => Ok(KeyPress::Down),
                    'C' => Ok(KeyPress::Right),
                    'D' => Ok(KeyPress::Left),
                    'F' => Ok(KeyPress::End),
                    'H' => Ok(KeyPress::Home),
                    _ => Ok(KeyPress::UnknownEscSeq),
                }
            }
        } else if seq1 == 'O' {
            // ESC O sequences.
            let seq2 = try!(self.next_char());
            match seq2 {
                'A' => Ok(KeyPress::Up),
                'B' => Ok(KeyPress::Down),
                'C' => Ok(KeyPress::Right),
                'D' => Ok(KeyPress::Left),
                'F' => Ok(KeyPress::End),
                'H' => Ok(KeyPress::Home),
                _ => Ok(KeyPress::UnknownEscSeq),
            }
        } else {
            // TODO ESC-N (n): search history forward not interactively
            // TODO ESC-P (p): search history backward not interactively
            // TODO ESC-R (r): Undo all changes made to this line.
            match seq1 {
                '\x08' => Ok(KeyPress::Meta('\x08')), // Backspace
                '<' => Ok(KeyPress::Meta('<')),
                '>' => Ok(KeyPress::Meta('>')),
                'b' | 'B' => Ok(KeyPress::Meta('B')),
                'c' | 'C' => Ok(KeyPress::Meta('C')),
                'd' | 'D' => Ok(KeyPress::Meta('D')),
                'f' | 'F' => Ok(KeyPress::Meta('F')),
                'l' | 'L' => Ok(KeyPress::Meta('L')),
                't' | 'T' => Ok(KeyPress::Meta('T')),
                'u' | 'U' => Ok(KeyPress::Meta('U')),
                'y' | 'Y' => Ok(KeyPress::Meta('Y')),
                '\x7f' => Ok(KeyPress::Meta('\x7f')), // Delete
                _ => {
                    // writeln!(io::stderr(), "key: {:?}, seq1: {:?}", KeyPress::Esc, seq1).unwrap();
                    Ok(KeyPress::UnknownEscSeq)
                }
            }
        }
    }
}

impl RawReader for PosixRawReader {
    fn next_key(&mut self, timeout_ms: i32) -> Result<KeyPress> {
        let c = try!(self.next_char());

        let mut key = consts::char_to_key_press(c);
        if key == KeyPress::Esc {
            let mut fds =
                [poll::PollFd::new(STDIN_FILENO, poll::POLLIN, poll::EventFlags::empty())];
            match poll::poll(&mut fds, timeout_ms) {
                Ok(n) if n == 0 => {
                    // single escape
                }
                Ok(_) => {
                    // escape sequence
                    key = try!(self.escape_sequence())
                }
                // Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(key)
    }

    fn next_char(&mut self) -> Result<char> {
        match self.chars.next() {
            Some(c) => Ok(try!(c)),
            None => Err(error::ReadlineError::Eof),
        }
    }
}


static SIGWINCH_ONCE: sync::Once = sync::ONCE_INIT;
static SIGWINCH: atomic::AtomicBool = atomic::ATOMIC_BOOL_INIT;

fn install_sigwinch_handler() {
    SIGWINCH_ONCE.call_once(|| unsafe {
        let sigwinch = signal::SigAction::new(signal::SigHandler::Handler(sigwinch_handler),
                                              signal::SaFlags::empty(),
                                              signal::SigSet::empty());
        let _ = signal::sigaction(signal::SIGWINCH, &sigwinch);
    });
}

extern "C" fn sigwinch_handler(_: libc::c_int) {
    SIGWINCH.store(true, atomic::Ordering::SeqCst);
}

pub type Terminal = PosixTerminal;

#[derive(Clone,Debug)]
pub struct PosixTerminal {
    unsupported: bool,
    stdin_isatty: bool,
}

impl Term for PosixTerminal {
    type Reader = PosixRawReader;
    type Mode = Mode;

    fn new() -> PosixTerminal {
        let term = PosixTerminal {
            unsupported: is_unsupported_term(),
            stdin_isatty: is_a_tty(STDIN_FILENO),
        };
        if !term.unsupported && term.stdin_isatty && is_a_tty(STDOUT_FILENO) {
            install_sigwinch_handler();
        }
        term
    }

    // Init checks:

    /// Check if current terminal can provide a rich line-editing user interface.
    fn is_unsupported(&self) -> bool {
        self.unsupported
    }

    /// check if stdin is connected to a terminal.
    fn is_stdin_tty(&self) -> bool {
        self.stdin_isatty
    }

    // Interactive loop:

    /// Try to get the number of columns in the current terminal,
    /// or assume 80 if it fails.
    fn get_columns(&self) -> usize {
        let (cols, _) = get_win_size();
        cols
    }

    /// Try to get the number of rows in the current terminal,
    /// or assume 24 if it fails.
    fn get_rows(&self) -> usize {
        let (_, rows) = get_win_size();
        rows
    }

    fn enable_raw_mode(&self) -> Result<Mode> {
        use nix::errno::Errno::ENOTTY;
        use nix::sys::termios::{BRKINT, CS8, ECHO, ICANON, ICRNL, IEXTEN, INPCK, ISIG, ISTRIP,
                                IXON, /* OPOST, */ VMIN, VTIME};
        if !self.stdin_isatty {
            try!(Err(nix::Error::from_errno(ENOTTY)));
        }
        let original_mode = try!(termios::tcgetattr(STDIN_FILENO));
        let mut raw = original_mode;
        // disable BREAK interrupt, CR to NL conversion on input,
        // input parity check, strip high bit (bit 8), output flow control
        raw.c_iflag = raw.c_iflag & !(BRKINT | ICRNL | INPCK | ISTRIP | IXON);
        // we don't want raw output, it turns newlines into straight linefeeds
        // raw.c_oflag = raw.c_oflag & !(OPOST); // disable all output processing
        raw.c_cflag = raw.c_cflag | (CS8); // character-size mark (8 bits)
        // disable echoing, canonical mode, extended input processing and signals
        raw.c_lflag = raw.c_lflag & !(ECHO | ICANON | IEXTEN | ISIG);
        raw.c_cc[VMIN] = 1; // One character-at-a-time input
        raw.c_cc[VTIME] = 0; // with blocking read
        try!(termios::tcsetattr(STDIN_FILENO, termios::TCSADRAIN, &raw));
        Ok(original_mode)
    }

    /// Create a RAW reader
    fn create_reader(&self) -> Result<PosixRawReader> {
        PosixRawReader::new()
    }

    /// Check if a SIGWINCH signal has been received
    fn sigwinch(&self) -> bool {
        SIGWINCH.compare_and_swap(true, false, atomic::Ordering::SeqCst)
    }

    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self, w: &mut Write) -> Result<()> {
        try!(w.write_all(b"\x1b[H\x1b[2J"));
        try!(w.flush());
        Ok(())
    }
}

#[cfg(unix)]
pub fn suspend() -> Result<()> {
    // For macos:
    try!(signal::kill(nix::unistd::getppid(), signal::SIGTSTP));
    try!(signal::kill(nix::unistd::getpid(), signal::SIGTSTP));
    Ok(())
}

#[cfg(all(unix,test))]
mod test {
    #[test]
    fn test_unsupported_term() {
        ::std::env::set_var("TERM", "xterm");
        assert_eq!(false, super::is_unsupported_term());

        ::std::env::set_var("TERM", "dumb");
        assert_eq!(true, super::is_unsupported_term());
    }
}
