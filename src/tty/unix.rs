use std;
use std::io::{Read, Write};
use std::sync;
use std::sync::atomic;
use libc;
use nix;
use nix::sys::signal;
use nix::sys::termios;

use char_iter;
use consts::{self, KeyPress};
use ::Result;
use ::error;

pub type Mode = termios::Termios;
const STDIN_FILENO: libc::c_int = libc::STDIN_FILENO;
const STDOUT_FILENO: libc::c_int = libc::STDOUT_FILENO;

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
fn get_columns() -> usize {
    let (cols, _) = get_win_size();
    cols
}

/// Try to get the number of rows in the current terminal,
/// or assume 24 if it fails.
fn get_rows() -> usize {
    let (_, rows) = get_win_size();
    rows
}

fn get_win_size() -> (usize, usize) {
    use std::mem::zeroed;
    use libc::c_ushort;

    unsafe {
        #[repr(C)]
        struct winsize {
            ws_row: c_ushort,
            ws_col: c_ushort,
            ws_xpixel: c_ushort,
            ws_ypixel: c_ushort,
        }

        let mut size: winsize = zeroed();
        match libc::ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut size) {
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
                    return true
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

/// Enable RAW mode for the terminal.
pub fn enable_raw_mode() -> Result<Mode> {
    use nix::errno::Errno::ENOTTY;
    use nix::sys::termios::{BRKINT, CS8, ECHO, ICANON, ICRNL, IEXTEN, INPCK, ISIG, ISTRIP, IXON,
                            /* OPOST, */ VMIN, VTIME};
    if !is_a_tty(STDIN_FILENO) {
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
    try!(termios::tcsetattr(STDIN_FILENO, termios::TCSAFLUSH, &raw));
    Ok(original_mode)
}

/// Disable RAW mode for the terminal.
pub fn disable_raw_mode(original_mode: Mode) -> Result<()> {
    try!(termios::tcsetattr(STDIN_FILENO, termios::TCSAFLUSH, &original_mode));
    Ok(())
}

fn clear_screen(w: &mut Write) -> Result<()> {
    try!(w.write_all(b"\x1b[H\x1b[2J"));
    try!(w.flush());
    Ok(())
}

/// Console input reader
pub struct RawReader<R> {
    chars: char_iter::Chars<R>,
}

impl<R: Read> RawReader<R> {
    pub fn new(stdin: R) -> Result<RawReader<R>> {
        Ok(RawReader { chars: char_iter::chars(stdin) })
    }

    // As there is no read timeout to properly handle single ESC key,
    // we make possible to deactivate escape sequence processing.
    pub fn next_key(&mut self, esc_seq: bool) -> Result<KeyPress> {
        let c = try!(self.next_char());

        let mut key = consts::char_to_key_press(c);
        if esc_seq && key == KeyPress::Esc {
            // escape sequence
            key = try!(self.escape_sequence());
        }
        Ok(key)
    }

    pub fn next_char(&mut self) -> Result<char> {
        match self.chars.next() {
            Some(c) => Ok(try!(c)),
            None => Err(error::ReadlineError::Eof),
        }
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
                        // '1' => Ok(KeyPress::Home),
                        '3' => Ok(KeyPress::Delete),
                        // '4' => Ok(KeyPress::End),
                        '5' => Ok(KeyPress::PageUp),
                        '6' => Ok(KeyPress::PageDown),
                        '7' => Ok(KeyPress::Home),
                        '8' => Ok(KeyPress::End),
                        _ => Ok(KeyPress::UnknownEscSeq),
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

static SIGWINCH_ONCE: sync::Once = sync::ONCE_INIT;
static SIGWINCH: atomic::AtomicBool = atomic::ATOMIC_BOOL_INIT;

fn install_sigwinch_handler() {
    SIGWINCH_ONCE.call_once(|| unsafe {
        let sigwinch = signal::SigAction::new(signal::SigHandler::Handler(sigwinch_handler),
                                              signal::SaFlag::empty(),
                                              signal::SigSet::empty());
        let _ = signal::sigaction(signal::SIGWINCH, &sigwinch);
    });
}

extern "C" fn sigwinch_handler(_: signal::SigNum) {
    SIGWINCH.store(true, atomic::Ordering::SeqCst);
}

pub type Terminal = PosixTerminal;

#[derive(Clone, Debug)]
pub struct PosixTerminal {
    unsupported: bool,
    stdin_isatty: bool,
}

impl PosixTerminal {
    pub fn new() -> PosixTerminal {
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
    pub fn is_unsupported(&self) -> bool {
        self.unsupported
    }

    /// check if stdin is connected to a terminal.
    pub fn is_stdin_tty(&self) -> bool {
        self.stdin_isatty
    }

    // Interactive loop:

    /// Get the number of columns in the current terminal.
    pub fn get_columns(&self) -> usize {
        get_columns()
    }

    /// Get the number of rows in the current terminal.
    pub fn get_rows(&self) -> usize {
        get_rows()
    }

    /// Create a RAW reader
    pub fn create_reader(&self) -> Result<RawReader<std::io::Stdin>> {
        RawReader::new(std::io::stdin())
    }

    /// Check if a SIGWINCH signal has been received
    pub fn sigwinch(&self) -> bool {
        SIGWINCH.compare_and_swap(true, false, atomic::Ordering::SeqCst)
    }

    /// Clear the screen. Used to handle ctrl+l
    pub fn clear_screen(&mut self, w: &mut Write) -> Result<()> {
        clear_screen(w)
    }
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
