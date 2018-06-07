//! Unix specific definitions
use std;
use std::io::{self, Read, Stdout, Write};
use std::sync;
use std::sync::atomic;
use libc;
use nix;
use nix::poll;
use nix::sys::signal;
use nix::sys::termios;

use char_iter;
use config::Config;
use consts::{self, KeyPress};
use Result;
use error;
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
        try!(termios::tcsetattr(STDIN_FILENO, termios::SetArg::TCSADRAIN, self));
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
    timeout_ms: i32,
}

impl PosixRawReader {
    pub fn new(config: &Config) -> Result<PosixRawReader> {
        let stdin = StdinRaw {};
        Ok(PosixRawReader {
               chars: char_iter::chars(stdin),
               timeout_ms: config.keyseq_timeout(),
           })
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
                    Ok(match seq2 {
                           '1' | '7' => KeyPress::Home, // '1': xterm
                           '3' => KeyPress::Delete,
                           '4' | '8' => KeyPress::End, // '4': xterm
                           '5' => KeyPress::PageUp,
                           '6' => KeyPress::PageDown,
                           _ => {
                        debug!(target: "rustyline", "unsupported esc sequence: ESC{:?}{:?}{:?}", seq1, seq2, seq3);
                        KeyPress::UnknownEscSeq
                    }
                       })
                } else {
                    debug!(target: "rustyline", "unsupported esc sequence: ESC{:?}{:?}{:?}", seq1, seq2, seq3);
                    Ok(KeyPress::UnknownEscSeq)
                }
            } else {
                Ok(match seq2 {
                       'A' => KeyPress::Up, // ANSI
                       'B' => KeyPress::Down,
                       'C' => KeyPress::Right,
                       'D' => KeyPress::Left,
                       'F' => KeyPress::End,
                       'H' => KeyPress::Home,
                       _ => {
                    debug!(target: "rustyline", "unsupported esc sequence: ESC{:?}{:?}", seq1, seq2);
                    KeyPress::UnknownEscSeq
                }
                   })
            }
        } else if seq1 == 'O' {
            // ESC O sequences.
            let seq2 = try!(self.next_char());
            Ok(match seq2 {
                   'A' => KeyPress::Up,
                   'B' => KeyPress::Down,
                   'C' => KeyPress::Right,
                   'D' => KeyPress::Left,
                   'F' => KeyPress::End,
                   'H' => KeyPress::Home,
                   _ => {
                debug!(target: "rustyline", "unsupported esc sequence: ESC{:?}{:?}", seq1, seq2);
                KeyPress::UnknownEscSeq
            }
               })
        } else {
            // TODO ESC-R (r): Undo all changes made to this line.
            Ok(match seq1 {
                   '\x08' => KeyPress::Meta('\x08'), // Backspace
                   '-' => KeyPress::Meta('-'),
                   '0'...'9' => KeyPress::Meta(seq1),
                   '<' => KeyPress::Meta('<'),
                   '>' => KeyPress::Meta('>'),
                   'b' | 'B' => KeyPress::Meta('B'),
                   'c' | 'C' => KeyPress::Meta('C'),
                   'd' | 'D' => KeyPress::Meta('D'),
                   'f' | 'F' => KeyPress::Meta('F'),
                   'l' | 'L' => KeyPress::Meta('L'),
                   'n' | 'N' => KeyPress::Meta('N'),
                   'p' | 'P' => KeyPress::Meta('P'),
                   't' | 'T' => KeyPress::Meta('T'),
                   'u' | 'U' => KeyPress::Meta('U'),
                   'y' | 'Y' => KeyPress::Meta('Y'),
                   '\x7f' => KeyPress::Meta('\x7f'), // Delete
                   _ => {
                debug!(target: "rustyline", "unsupported esc sequence: M-{:?}", seq1);
                KeyPress::UnknownEscSeq
            }
               })
        }
    }
}

impl RawReader for PosixRawReader {
    fn next_key(&mut self) -> Result<KeyPress> {
        let c = try!(self.next_char());

        let mut key = consts::char_to_key_press(c);
        if key == KeyPress::Esc {
            let mut fds =
                [poll::PollFd::new(STDIN_FILENO, poll::EventFlags::POLLIN)];
            match poll::poll(&mut fds, self.timeout_ms) {
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
        debug!(target: "rustyline", "key: {:?}", key);
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
    debug!(target: "rustyline", "SIGWINCH");
}

pub type Terminal = PosixTerminal;

#[derive(Clone,Debug)]
pub struct PosixTerminal {
    unsupported: bool,
    stdin_isatty: bool,
}

impl Term for PosixTerminal {
    type Reader = PosixRawReader;
    type Writer = Stdout;
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
        use nix::sys::termios::{InputFlags, ControlFlags, LocalFlags,/* OPOST, */ SpecialCharacterIndices};
        if !self.stdin_isatty {
            try!(Err(nix::Error::from_errno(ENOTTY)));
        }
        let original_mode = try!(termios::tcgetattr(STDIN_FILENO));
        let mut raw = original_mode.clone();
        // disable BREAK interrupt, CR to NL conversion on input,
        // input parity check, strip high bit (bit 8), output flow control
        raw.input_flags &= !(InputFlags::BRKINT | InputFlags::ICRNL | InputFlags::INPCK | InputFlags::ISTRIP | InputFlags::IXON);
        // we don't want raw output, it turns newlines into straight linefeeds
        // raw.c_oflag = raw.c_oflag & !(OPOST); // disable all output processing
        raw.control_flags |= ControlFlags::CS8; // character-size mark (8 bits)
        // disable echoing, canonical mode, extended input processing and signals
        raw.local_flags &= !(LocalFlags::ECHO | LocalFlags::ICANON | LocalFlags::IEXTEN | LocalFlags::ISIG);
        raw.control_chars[SpecialCharacterIndices::VMIN as usize] = 1; // One character-at-a-time input
        raw.control_chars[SpecialCharacterIndices::VTIME as usize] = 0; // with blocking read
        try!(termios::tcsetattr(STDIN_FILENO, termios::SetArg::TCSADRAIN, &raw));
        Ok(original_mode)
    }

    /// Create a RAW reader
    fn create_reader(&self, config: &Config) -> Result<PosixRawReader> {
        PosixRawReader::new(config)
    }

    fn create_writer(&self) -> Stdout {
        io::stdout()
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
