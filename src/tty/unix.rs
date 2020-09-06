//! Unix specific definitions
use std::cmp::Ordering;
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync;
use std::sync::atomic;

use log::{debug, warn};
use nix::poll::{self, PollFlags};
use nix::sys::signal;
use nix::sys::termios;
use nix::sys::termios::SetArg;
use unicode_segmentation::UnicodeSegmentation;
use utf8parse::{Parser, Receiver};

use super::{width, RawMode, RawReader, Renderer, Term};
use crate::config::{BellStyle, ColorMode, Config, OutputStreamType};
use crate::error;
use crate::highlight::Highlighter;
use crate::keys::{self, KeyEvent, KeyCode as K, Modifiers as M};
use crate::layout::{Layout, Position};
use crate::line_buffer::LineBuffer;
use crate::Result;

const STDIN_FILENO: RawFd = libc::STDIN_FILENO;

/// Unsupported Terminals that don't support RAW mode
const UNSUPPORTED_TERM: [&str; 3] = ["dumb", "cons25", "emacs"];

const BRACKETED_PASTE_ON: &[u8] = b"\x1b[?2004h";
const BRACKETED_PASTE_OFF: &[u8] = b"\x1b[?2004l";

impl AsRawFd for OutputStreamType {
    fn as_raw_fd(&self) -> RawFd {
        match self {
            OutputStreamType::Stdout => libc::STDOUT_FILENO,
            OutputStreamType::Stderr => libc::STDERR_FILENO,
        }
    }
}

nix::ioctl_read_bad!(win_size, libc::TIOCGWINSZ, libc::winsize);

#[allow(clippy::useless_conversion)]
fn get_win_size<T: AsRawFd + ?Sized>(fileno: &T) -> (usize, usize) {
    use std::mem::zeroed;

    if cfg!(test) {
        return (80, 24);
    }

    unsafe {
        let mut size: libc::winsize = zeroed();
        match win_size(fileno.as_raw_fd(), &mut size) {
            Ok(0) => {
                // In linux pseudo-terminals are created with dimensions of
                // zero. If host application didn't initialize the correct
                // size before start we treat zero size as 80 columns and
                // inifinite rows
                let cols = if size.ws_col == 0 {
                    80
                } else {
                    size.ws_col as usize
                };
                let rows = if size.ws_row == 0 {
                    usize::max_value()
                } else {
                    size.ws_row as usize
                };
                (cols, rows)
            }
            _ => (80, 24),
        }
    }
}

/// Check TERM environment variable to see if current term is in our
/// unsupported list
fn is_unsupported_term() -> bool {
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
fn is_a_tty(fd: RawFd) -> bool {
    unsafe { libc::isatty(fd) != 0 }
}

pub struct PosixMode {
    termios: termios::Termios,
    out: Option<OutputStreamType>,
}

#[cfg(not(test))]
pub type Mode = PosixMode;

impl RawMode for PosixMode {
    /// Disable RAW mode for the terminal.
    fn disable_raw_mode(&self) -> Result<()> {
        termios::tcsetattr(STDIN_FILENO, SetArg::TCSADRAIN, &self.termios)?;
        // disable bracketed paste
        if let Some(out) = self.out {
            write_and_flush(out, BRACKETED_PASTE_OFF)?;
        }
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
                libc::read(
                    STDIN_FILENO,
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len() as libc::size_t,
                )
            };
            if res == -1 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted
                    || SIGWINCH.load(atomic::Ordering::Relaxed)
                {
                    return Err(error);
                }
            } else {
                #[allow(clippy::cast_sign_loss)]
                return Ok(res as usize);
            }
        }
    }
}

/// Console input reader
pub struct PosixRawReader {
    stdin: StdinRaw,
    timeout_ms: i32,
    buf: [u8; 1],
    parser: Parser,
    receiver: Utf8,
}

struct Utf8 {
    c: Option<char>,
    valid: bool,
}

const UP: char = 'A'; // kcuu1, kUP*
const DOWN: char = 'B'; // kcud1, kDN*
const RIGHT: char = 'C'; // kcuf1, kRIT*
const LEFT: char = 'D'; // kcub1, kLFT*
const END: char = 'F'; // kend*
const HOME: char = 'H'; // khom*
const INSERT: char = '2'; // kic*
const DELETE: char = '3'; // kdch1, kDC*
const PAGE_UP: char = '5'; // kpp, kPRV*
const PAGE_DOWN: char = '6'; // knp, kNXT*

const RXVT_HOME: char = '7';
const RXVT_END: char = '8';

const SHIFT: char = '2';
const ALT: char = '3';
const ALT_SHIFT: char = '4';
const CTRL: char = '5';
const CTRL_SHIFT: char = '6';
const CTRL_ALT: char = '7';
const CTRL_ALT_SHIFT: char = '8';

const RXVT_SHIFT: char = '$';
const RXVT_CTRL: char = '\x1e';
const RXVT_CTRL_SHIFT: char = '@';

impl PosixRawReader {
    fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            stdin: StdinRaw {},
            timeout_ms: config.keyseq_timeout(),
            buf: [0; 1],
            parser: Parser::new(),
            receiver: Utf8 {
                c: None,
                valid: true,
            },
        })
    }

    /// Handle \E <seq1> sequences
    // https://invisible-island.net/xterm/xterm-function-keys.html
    fn escape_sequence(&mut self) -> Result<KeyEvent> {
        // Read the next byte representing the escape sequence.
        let seq1 = self.next_char()?;
        if seq1 == '[' {
            // \E[ sequences. (CSI)
            self.escape_csi()
        } else if seq1 == 'O' {
            // xterm
            // \EO sequences. (SS3)
            self.escape_o()
        } else if seq1 == '\x1b' {
            // \E\E
            // \E\E[A => Alt-Up
            // \E\E[B => Alt-Down
            // \E\E[C => Alt-Right
            // \E\E[D => Alt-Left
            // ...
            Ok((K::Esc, M::NONE))
        } else {
            // TODO ESC-R (r): Undo all changes made to this line.
            Ok((K::Char(seq1), M::ALT))
        }
    }

    /// Handle \E[ <seq2> escape sequences
    fn escape_csi(&mut self) -> Result<KeyEvent> {
        let seq2 = self.next_char()?;
        if seq2.is_digit(10) {
            match seq2 {
                '0' | '9' => {
                    debug!(target: "rustyline", "unsupported esc sequence: \\E[{:?}", seq2);
                    Ok((K::UnknownEscSeq, M::NONE))
                }
                _ => {
                    // Extended escape, read additional byte.
                    self.extended_escape(seq2)
                }
            }
        } else if seq2 == '[' {
            let seq3 = self.next_char()?;
            // Linux console
            Ok(match seq3 {
                'A' => (K::F(1), M::NONE),
                'B' => (K::F(2), M::NONE),
                'C' => (K::F(3), M::NONE),
                'D' => (K::F(4), M::NONE),
                'E' => (K::F(5), M::NONE),
                _ => {
                    debug!(target: "rustyline", "unsupported esc sequence: \\E[[{:?}", seq3);
                    (K::UnknownEscSeq, M::NONE)
                }
            })
        } else {
            // ANSI
            Ok(match seq2 {
                UP => (K::Up, M::NONE),
                DOWN => (K::Down, M::NONE),
                RIGHT => (K::Right, M::NONE),
                LEFT => (K::Left, M::NONE),
                //'E' => (K::, M::), // Ignore
                END => (K::End, M::NONE),
                //'G' => (K::, M::), // Ignore
                HOME => (K::Home, M::NONE), // khome
                //'J' => (K::, M::), // clr_eos
                //'K' => (K::, M::), // clr_eol
                //'L' => (K::, M::), // il1
                //'M' => (K::, M::), // kmous
                //'P' => (K::Delete, M::NONE), // dch1
                'Z' => (K::BackTab, M::NONE),
                'a' => (K::Up, M::SHIFT), // rxvt: kind or kUP
                'b' => (K::Down, M::SHIFT), // rxvt: kri or kDN
                'c' => (K::Right, M::SHIFT), // rxvt
                'd' => (K::Left, M::SHIFT), // rxvt
                _ => {
                    debug!(target: "rustyline", "unsupported esc sequence: \\E[{:?}", seq2);
                    (K::UnknownEscSeq, M::NONE)
                }
            })
        }
    }

    /// Handle \E[ <seq2:digit> escape sequences
    #[allow(clippy::cognitive_complexity)]
    fn extended_escape(&mut self, seq2: char) -> Result<KeyEvent> {
        let seq3 = self.next_char()?;
        if seq3 == '~' {
            Ok(match seq2 {
                '1' | RXVT_HOME => (K::Home, M::NONE), // tmux, xrvt
                INSERT => (K::Insert, M::NONE),
                DELETE => (K::Delete, M::NONE),
                '4' | RXVT_END => (K::End, M::NONE), // tmux, xrvt
                PAGE_UP => (K::PageUp, M::NONE),
                PAGE_DOWN => (K::PageDown, M::NONE),
                _ => {
                    debug!(target: "rustyline",
                           "unsupported esc sequence: \\E[{}~", seq2);
                    (K::UnknownEscSeq, M::NONE)
                }
            })
        } else if seq3.is_digit(10) {
            let seq4 = self.next_char()?;
            if seq4 == '~' {
                Ok(match (seq2, seq3) {
                    ('1', '1') => (K::F(1), M::NONE),  // rxvt-unicode
                    ('1', '2') => (K::F(2), M::NONE),  // rxvt-unicode
                    ('1', '3') => (K::F(3), M::NONE),  // rxvt-unicode
                    ('1', '4') => (K::F(4), M::NONE),  // rxvt-unicode
                    ('1', '5') => (K::F(5), M::NONE),  // kf5
                    ('1', '7') => (K::F(6), M::NONE),  // kf6
                    ('1', '8') => (K::F(7), M::NONE),  // kf7
                    ('1', '9') => (K::F(8), M::NONE),  // kf8
                    ('2', '0') => (K::F(9), M::NONE),  // kf9
                    ('2', '1') => (K::F(10), M::NONE), // kf10
                    ('2', '3') => (K::F(11), M::NONE), // kf11
                    ('2', '4') => (K::F(12), M::NONE), // kf12
                    //('6', '2') => KeyCode::ScrollUp,
                    //('6', '3') => KeyCode::ScrollDown,
                    _ => {
                        debug!(target: "rustyline",
                               "unsupported esc sequence: \\E[{}{}~", seq2, seq3);
                        (K::UnknownEscSeq, M::NONE)
                    }
                })
            } else if seq4 == ';' {
                let seq5 = self.next_char()?;
                if seq5.is_digit(10) {
                    let seq6 = self.next_char()?;
                    if seq6.is_digit(10) {
                        self.next_char()?; // 'R' expected
                        Ok((K::UnknownEscSeq, M::NONE))
                    } else if seq6 == 'R' {
                        Ok((K::UnknownEscSeq, M::NONE))
                    } else if seq6 == '~' {
                        Ok(match (seq2, seq3, seq5) {
                            ('1', '5', CTRL) => (K::F(5), M::CTRL),
                            //('1', '5', '6') => (K::F(17), M::CTRL),
                            ('1', '7', CTRL) => (K::F(6), M::CTRL),
                            //('1', '7', '6') => (K::F(18), M::CTRL),
                            ('1', '8', CTRL) => (K::F(7), M::CTRL),
                            ('1', '9', CTRL) => (K::F(8), M::CTRL),
                            //('1', '9', '6') => (K::F(19), M::CTRL),
                            ('2', '0', CTRL) => (K::F(9), M::CTRL),
                            //('2', '0', '6') => (K::F(21), M::CTRL),
                            ('2', '1', CTRL) => (K::F(10), M::CTRL),
                            //('2', '1', '6') => (K::F(22), M::CTRL),
                            ('2', '3', CTRL) => (K::F(11), M::CTRL),
                            //('2', '3', '6') => (K::F(23), M::CTRL),
                            ('2', '4', CTRL) => (K::F(12), M::CTRL),
                            //('2', '4', '6') => (K::F(24), M::CTRL),
                            _ => {
                                debug!(target: "rustyline",
                                       "unsupported esc sequence: \\E[{}{};{}~", seq2, seq3, seq5);
                                (K::UnknownEscSeq, M::NONE)
                            }
                        })
                    } else {
                        debug!(target: "rustyline",
                               "unsupported esc sequence: \\E[{}{};{}{}", seq2, seq3, seq5, seq6);
                        Ok((K::UnknownEscSeq, M::NONE))
                    }
                } else {
                    debug!(target: "rustyline",
                           "unsupported esc sequence: \\E[{}{};{:?}", seq2, seq3, seq5);
                    Ok((K::UnknownEscSeq, M::NONE))
                }
            } else if seq4.is_digit(10) {
                let seq5 = self.next_char()?;
                if seq5 == '~' {
                    Ok(match (seq2, seq3, seq4) {
                        ('2', '0', '0') => (K::BracketedPasteStart, M::NONE),
                        ('2', '0', '1') => (K::BracketedPasteEnd, M::NONE),
                        _ => {
                            debug!(target: "rustyline",
                                   "unsupported esc sequence: \\E[{}{}{}~", seq2, seq3, seq4);
                            (K::UnknownEscSeq, M::NONE)
                        }
                    })
                } else {
                    debug!(target: "rustyline",
                           "unsupported esc sequence: \\E[{}{}{}{}", seq2, seq3, seq4, seq5);
                    Ok((K::UnknownEscSeq, M::NONE))
                }
            } else {
                debug!(target: "rustyline",
                       "unsupported esc sequence: \\E[{}{}{:?}", seq2, seq3, seq4);
                Ok((K::UnknownEscSeq, M::NONE))
            }
        } else if seq3 == ';' {
            let seq4 = self.next_char()?;
            if seq4.is_digit(10) {
                let seq5 = self.next_char()?;
                if seq5.is_digit(10) {
                    self.next_char()?; // 'R' expected
                    //('1', '0', UP) => (K::, M::), // Alt + Shift + Up
                    Ok((K::UnknownEscSeq, M::NONE))
                } else if seq2 == '1' {
                    Ok(match (seq4, seq5) {
                        (SHIFT, UP) => (K::Up, M::SHIFT), // ~ key_sr
                        (SHIFT, DOWN) => (K::Down, M::SHIFT), // ~ key_sf
                        (SHIFT, RIGHT) => (K::Right, M::SHIFT),
                        (SHIFT, LEFT) => (K::Left, M::SHIFT),
                        (SHIFT, END) => (K::End, M::SHIFT), // kEND
                        (SHIFT, HOME) => (K::Home, M::SHIFT), // kHOM
                        //('2', 'P') => (K::F(13), M::NONE),
                        //('2', 'Q') => (K::F(14), M::NONE),
                        //('2', 'S') => (K::F(16), M::NONE),
                        (ALT, UP) => (K::Up, M::ALT),
                        (ALT, DOWN) => (K::Down, M::ALT),
                        (ALT, RIGHT) => (K::Right, M::ALT),
                        (ALT, LEFT) => (K::Left, M::ALT),
                        (ALT, END) => (K::End, M::ALT),
                        (ALT, HOME) => (K::Home, M::ALT),
                        (ALT_SHIFT, UP) => (K::Up, M::ALT_SHIFT),
                        (ALT_SHIFT, DOWN) => (K::Down, M::ALT_SHIFT),
                        (ALT_SHIFT, RIGHT) => (K::Right, M::ALT_SHIFT),
                        (ALT_SHIFT, LEFT) => (K::Left, M::ALT_SHIFT),
                        (ALT_SHIFT, END) => (K::End, M::ALT_SHIFT),
                        (ALT_SHIFT, HOME) => (K::Home, M::ALT_SHIFT),
                        (CTRL, UP) => (K::Up, M::CTRL),
                        (CTRL, DOWN) => (K::Down, M::CTRL),
                        (CTRL, RIGHT) => (K::Right, M::CTRL),
                        (CTRL, LEFT) => (K::Left, M::CTRL),
                        (CTRL, END) => (K::End, M::CTRL),
                        (CTRL, HOME) => (K::Home, M::CTRL),
                        (CTRL, 'P') => (K::F(1), M::CTRL),
                        (CTRL, 'Q') => (K::F(2), M::CTRL),
                        (CTRL, 'S') => (K::F(4), M::CTRL),
                        (CTRL, 'p') => (K::Char('0'), M::CTRL),
                        (CTRL, 'q') => (K::Char('1'), M::CTRL),
                        (CTRL, 'r') => (K::Char('2'), M::CTRL),
                        (CTRL, 's') => (K::Char('3'), M::CTRL),
                        (CTRL, 't') => (K::Char('4'), M::CTRL),
                        (CTRL, 'u') => (K::Char('5'), M::CTRL),
                        (CTRL, 'v') => (K::Char('6'), M::CTRL),
                        (CTRL, 'w') => (K::Char('7'), M::CTRL),
                        (CTRL, 'x') => (K::Char('8'), M::CTRL),
                        (CTRL, 'y') => (K::Char('9'), M::CTRL),
                        (CTRL_SHIFT, UP) => (K::Up, M::CTRL_SHIFT),
                        (CTRL_SHIFT, DOWN) => (K::Down, M::CTRL_SHIFT),
                        (CTRL_SHIFT, RIGHT) => (K::Right, M::CTRL_SHIFT),
                        (CTRL_SHIFT, LEFT) => (K::Left, M::CTRL_SHIFT),
                        (CTRL_SHIFT, END) => (K::End, M::CTRL_SHIFT),
                        (CTRL_SHIFT, HOME) => (K::Home, M::CTRL_SHIFT),
                        //('6', 'P') => (K::F(13), M::CTRL),
                        //('6', 'Q') => (K::F(14), M::CTRL),
                        //('6', 'S') => (K::F(16), M::CTRL),
                        (CTRL_SHIFT, 'p') => (K::Char('0'), M::CTRL_SHIFT),
                        (CTRL_SHIFT, 'q') => (K::Char('1'), M::CTRL_SHIFT),
                        (CTRL_SHIFT, 'r') => (K::Char('2'), M::CTRL_SHIFT),
                        (CTRL_SHIFT, 's') => (K::Char('3'), M::CTRL_SHIFT),
                        (CTRL_SHIFT, 't') => (K::Char('4'), M::CTRL_SHIFT),
                        (CTRL_SHIFT, 'u') => (K::Char('5'), M::CTRL_SHIFT),
                        (CTRL_SHIFT, 'v') => (K::Char('6'), M::CTRL_SHIFT),
                        (CTRL_SHIFT, 'w') => (K::Char('7'), M::CTRL_SHIFT),
                        (CTRL_SHIFT, 'x') => (K::Char('8'), M::CTRL_SHIFT),
                        (CTRL_SHIFT, 'y') => (K::Char('9'), M::CTRL_SHIFT),
                        (CTRL_ALT, UP) => (K::Up, M::CTRL_ALT),
                        (CTRL_ALT, DOWN) => (K::Down, M::CTRL_ALT),
                        (CTRL_ALT, RIGHT) => (K::Right, M::CTRL_ALT),
                        (CTRL_ALT, LEFT) => (K::Left, M::CTRL_ALT),
                        (CTRL_ALT, END) => (K::End, M::CTRL_ALT),
                        (CTRL_ALT, HOME) => (K::Home, M::CTRL_ALT),
                        (CTRL_ALT, 'p') => (K::Char('0'), M::CTRL_ALT),
                        (CTRL_ALT, 'q') => (K::Char('1'), M::CTRL_ALT),
                        (CTRL_ALT, 'r') => (K::Char('2'), M::CTRL_ALT),
                        (CTRL_ALT, 's') => (K::Char('3'), M::CTRL_ALT),
                        (CTRL_ALT, 't') => (K::Char('4'), M::CTRL_ALT),
                        (CTRL_ALT, 'u') => (K::Char('5'), M::CTRL_ALT),
                        (CTRL_ALT, 'v') => (K::Char('6'), M::CTRL_ALT),
                        (CTRL_ALT, 'w') => (K::Char('7'), M::CTRL_ALT),
                        (CTRL_ALT, 'x') => (K::Char('8'), M::CTRL_ALT),
                        (CTRL_ALT, 'y') => (K::Char('9'), M::CTRL_ALT),
                        (CTRL_ALT_SHIFT, UP) => (K::Up, M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, DOWN) => (K::Down, M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, RIGHT) => (K::Right, M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, LEFT) => (K::Left, M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, END) => (K::End, M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, HOME) => (K::Home, M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, 'p') => (K::Char('0'), M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, 'q') => (K::Char('1'), M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, 'r') => (K::Char('2'), M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, 's') => (K::Char('3'), M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, 't') => (K::Char('4'), M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, 'u') => (K::Char('5'), M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, 'v') => (K::Char('6'), M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, 'w') => (K::Char('7'), M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, 'x') => (K::Char('8'), M::CTRL_ALT_SHIFT),
                        (CTRL_ALT_SHIFT, 'y') => (K::Char('9'), M::CTRL_ALT_SHIFT),
                        ('9', UP) => (K::Up, M::ALT), // Meta + arrow on (some?) Macs when using iTerm defaults
                        ('9', DOWN) => (K::Down, M::ALT),
                        ('9', RIGHT) => (K::Right, M::ALT),
                        ('9', LEFT) => (K::Left, M::ALT),
                        _ => {
                            debug!(target: "rustyline",
                                   "unsupported esc sequence: \\E[1;{}{:?}", seq4, seq5);
                            (K::UnknownEscSeq, M::NONE)
                        }
                    })
                } else if seq5 == '~'{
                    Ok(match (seq2, seq4) {
                        (INSERT, SHIFT) => (K::Insert, M::SHIFT),
                        (INSERT, ALT) => (K::Insert, M::ALT),
                        (INSERT, ALT_SHIFT) => (K::Insert, M::ALT_SHIFT),
                        (INSERT, CTRL) => (K::Insert, M::CTRL),
                        (INSERT, CTRL_SHIFT) => (K::Insert, M::CTRL_SHIFT),
                        (INSERT, CTRL_ALT) => (K::Insert, M::CTRL_ALT),
                        (INSERT, CTRL_ALT_SHIFT) => (K::Insert, M::CTRL_ALT_SHIFT),
                        (DELETE, SHIFT) => (K::Delete, M::SHIFT),
                        (DELETE, ALT) => (K::Delete, M::ALT),
                        (DELETE, ALT_SHIFT) => (K::Delete, M::ALT_SHIFT),
                        (DELETE, CTRL) => (K::Delete, M::CTRL),
                        (DELETE, CTRL_SHIFT) => (K::Delete, M::CTRL_SHIFT),
                        (DELETE, CTRL_ALT) => (K::Delete, M::CTRL_ALT),
                        (DELETE, CTRL_ALT_SHIFT) => (K::Delete, M::CTRL_ALT_SHIFT),
                        (PAGE_UP, SHIFT) => (K::PageUp, M::SHIFT),
                        (PAGE_UP, ALT) => (K::PageUp, M::ALT),
                        (PAGE_UP, ALT_SHIFT) => (K::PageUp, M::ALT_SHIFT),
                        (PAGE_UP, CTRL) => (K::PageUp, M::CTRL),
                        (PAGE_UP, CTRL_SHIFT) => (K::PageUp, M::CTRL_SHIFT),
                        (PAGE_UP, CTRL_ALT) => (K::PageUp, M::CTRL_ALT),
                        (PAGE_UP, CTRL_ALT_SHIFT) => (K::PageUp, M::CTRL_ALT_SHIFT),
                        (PAGE_DOWN, SHIFT) => (K::PageDown, M::SHIFT),
                        (PAGE_DOWN, ALT) => (K::PageDown, M::ALT),
                        (PAGE_DOWN, ALT_SHIFT) => (K::PageDown, M::ALT_SHIFT),
                        (PAGE_DOWN, CTRL) => (K::PageDown, M::CTRL),
                        (PAGE_DOWN, CTRL_SHIFT) => (K::PageDown, M::CTRL_SHIFT),
                        (PAGE_DOWN, CTRL_ALT) => (K::PageDown, M::CTRL_ALT),
                        (PAGE_DOWN, CTRL_ALT_SHIFT) => (K::PageDown, M::CTRL_ALT_SHIFT),
                        _ => {
                            debug!(target: "rustyline",
                                   "unsupported esc sequence: \\E[{};{:?}~", seq2, seq4);
                            (K::UnknownEscSeq, M::NONE)
                        }
                    })
                } else {
                    debug!(target: "rustyline",
                           "unsupported esc sequence: \\E[{};{}{:?}", seq2, seq4, seq5);
                    Ok((K::UnknownEscSeq, M::NONE))
                }
            } else {
                debug!(target: "rustyline",
                       "unsupported esc sequence: \\E[{};{:?}", seq2, seq4);
                Ok((K::UnknownEscSeq, M::NONE))
            }
        } else {
            Ok(match (seq2, seq3) {
                (DELETE, RXVT_CTRL) => (K::Delete, M::CTRL),
                (DELETE, RXVT_CTRL_SHIFT) => (K::Delete, M::CTRL_SHIFT),
                (CTRL, UP) => (K::Up, M::CTRL),
                (CTRL, DOWN) => (K::Down, M::CTRL),
                (CTRL, RIGHT) => (K::Right, M::CTRL),
                (CTRL, LEFT) => (K::Left, M::CTRL),
                (PAGE_UP, RXVT_CTRL) => (K::PageUp, M::CTRL),
                (PAGE_UP, RXVT_SHIFT) => (K::PageUp, M::SHIFT),
                (PAGE_UP, RXVT_CTRL_SHIFT) => (K::PageUp, M::CTRL_SHIFT),
                (PAGE_DOWN, RXVT_CTRL) => (K::PageDown, M::CTRL),
                (PAGE_DOWN, RXVT_SHIFT) => (K::PageDown, M::SHIFT),
                (PAGE_DOWN, RXVT_CTRL_SHIFT) => (K::PageDown, M::CTRL_SHIFT),
                (RXVT_HOME, RXVT_CTRL) => (K::Home, M::CTRL),
                (RXVT_HOME, RXVT_SHIFT) => (K::Home, M::SHIFT),
                (RXVT_HOME, RXVT_CTRL_SHIFT) => (K::Home, M::CTRL_SHIFT),
                (RXVT_END, RXVT_CTRL) => (K::End, M::CTRL), // kEND5 or kel
                (RXVT_END, RXVT_SHIFT) => (K::End, M::SHIFT),
                (RXVT_END, RXVT_CTRL_SHIFT) => (K::End, M::CTRL_SHIFT),
                _ => {
                    debug!(target: "rustyline",
                           "unsupported esc sequence: \\E[{}{:?}", seq2, seq3);
                    (K::UnknownEscSeq, M::NONE)
                }
            })
        }
    }

    /// Handle \EO <seq2> escape sequences
    fn escape_o(&mut self) -> Result<KeyEvent> {
        let seq2 = self.next_char()?;
        Ok(match seq2 {
            UP => (K::Up, M::NONE),
            DOWN => (K::Down, M::NONE),
            RIGHT => (K::Right, M::NONE),
            LEFT => (K::Left, M::NONE),
            //'E' => (K::, M::),// key_b2, kb2
            END => (K::End, M::NONE),   // kend
            HOME => (K::Home, M::NONE),  // khome
            'M' => (K::Enter, M::NONE),  // kent
            'P' => (K::F(1), M::NONE),  // kf1
            'Q' => (K::F(2), M::NONE),  // kf2
            'R' => (K::F(3), M::NONE),  // kf3
            'S' => (K::F(4), M::NONE),  // kf4
            'a' => (K::Up, M::CTRL),
            'b' => (K::Down, M::CTRL),
            'c' => (K::Right, M::CTRL), // rxvt
            'd' => (K::Left, M::CTRL),  // rxvt
            'l' => (K::F(8), M::NONE),
            't' => (K::F(5), M::NONE),  // kf5 or kb1
            'u' => (K::F(6), M::NONE),  // kf6 or kb2
            'v' => (K::F(7), M::NONE),  // kf7 or kb3
            'w' => (K::F(9), M::NONE), // kf9 or ka1
            'x' => (K::F(10), M::NONE), // kf10 or ka2
            _ => {
                debug!(target: "rustyline", "unsupported esc sequence: \\EO{:?}", seq2);
                (K::UnknownEscSeq, M::NONE)
            }
        })
    }

    fn poll(&mut self, timeout_ms: i32) -> ::nix::Result<i32> {
        let mut fds = [poll::PollFd::new(STDIN_FILENO, PollFlags::POLLIN)];
        poll::poll(&mut fds, timeout_ms)
    }
}

impl RawReader for PosixRawReader {
    fn next_key(&mut self, single_esc_abort: bool) -> Result<KeyEvent> {
        let c = self.next_char()?;

        let mut key = keys::char_to_key_press(c);
        if key == (K::Esc, M::NONE) {
            let timeout_ms = if single_esc_abort && self.timeout_ms == -1 {
                0
            } else {
                self.timeout_ms
            };
            match self.poll(timeout_ms) {
                Ok(n) if n == 0 => {
                    // single escape
                }
                Ok(_) => {
                    // escape sequence
                    key = self.escape_sequence()?
                }
                // Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }
        debug!(target: "rustyline", "key: {:?}", key);
        Ok(key)
    }

    fn next_char(&mut self) -> Result<char> {
        loop {
            let n = self.stdin.read(&mut self.buf)?;
            if n == 0 {
                return Err(error::ReadlineError::Eof);
            }
            let b = self.buf[0];
            self.parser.advance(&mut self.receiver, b);
            if !self.receiver.valid {
                return Err(error::ReadlineError::Utf8Error);
            } else if let Some(c) = self.receiver.c.take() {
                return Ok(c);
            }
        }
    }

    fn read_pasted_text(&mut self) -> Result<String> {
        let mut buffer = String::new();
        loop {
            match self.next_char()? {
                '\x1b' => {
                    let key = self.escape_sequence()?;
                    if key == (K::BracketedPasteEnd, M::NONE) {
                        break;
                    } else {
                        continue; // TODO validate
                    }
                }
                c => buffer.push(c),
            };
        }
        let buffer = buffer.replace("\r\n", "\n");
        let buffer = buffer.replace("\r", "\n");
        Ok(buffer)
    }
}

impl Receiver for Utf8 {
    /// Called whenever a code point is parsed successfully
    fn codepoint(&mut self, c: char) {
        self.c = Some(c);
        self.valid = true;
    }

    /// Called when an invalid_sequence is detected
    fn invalid_sequence(&mut self) {
        self.c = None;
        self.valid = false;
    }
}

/// Console output writer
pub struct PosixRenderer {
    out: OutputStreamType,
    cols: usize, // Number of columns in terminal
    buffer: String,
    tab_stop: usize,
    colors_enabled: bool,
    bell_style: BellStyle,
}

impl PosixRenderer {
    fn new(
        out: OutputStreamType,
        tab_stop: usize,
        colors_enabled: bool,
        bell_style: BellStyle,
    ) -> Self {
        let (cols, _) = get_win_size(&out);
        Self {
            out,
            cols,
            buffer: String::with_capacity(1024),
            tab_stop,
            colors_enabled,
            bell_style,
        }
    }

    fn clear_old_rows(&mut self, layout: &Layout) {
        use std::fmt::Write;
        let current_row = layout.cursor.row;
        let old_rows = layout.end.row;
        // old_rows < cursor_row if the prompt spans multiple lines and if
        // this is the default State.
        let cursor_row_movement = old_rows.saturating_sub(current_row);
        // move the cursor down as required
        if cursor_row_movement > 0 {
            write!(self.buffer, "\x1b[{}B", cursor_row_movement).unwrap();
        }
        // clear old rows
        for _ in 0..old_rows {
            self.buffer.push_str("\r\x1b[0K\x1b[A");
        }
        // clear the line
        self.buffer.push_str("\r\x1b[0K");
    }
}

impl Renderer for PosixRenderer {
    type Reader = PosixRawReader;

    fn move_cursor(&mut self, old: Position, new: Position) -> Result<()> {
        use std::fmt::Write;
        self.buffer.clear();
        let row_ordering = new.row.cmp(&old.row);
        if row_ordering == Ordering::Greater {
            // move down
            let row_shift = new.row - old.row;
            if row_shift == 1 {
                self.buffer.push_str("\x1b[B");
            } else {
                write!(self.buffer, "\x1b[{}B", row_shift).unwrap();
            }
        } else if row_ordering == Ordering::Less {
            // move up
            let row_shift = old.row - new.row;
            if row_shift == 1 {
                self.buffer.push_str("\x1b[A");
            } else {
                write!(self.buffer, "\x1b[{}A", row_shift).unwrap();
            }
        }
        let col_ordering = new.col.cmp(&old.col);
        if col_ordering == Ordering::Greater {
            // move right
            let col_shift = new.col - old.col;
            if col_shift == 1 {
                self.buffer.push_str("\x1b[C");
            } else {
                write!(self.buffer, "\x1b[{}C", col_shift).unwrap();
            }
        } else if col_ordering == Ordering::Less {
            // move left
            let col_shift = old.col - new.col;
            if col_shift == 1 {
                self.buffer.push_str("\x1b[D");
            } else {
                write!(self.buffer, "\x1b[{}D", col_shift).unwrap();
            }
        }
        self.write_and_flush(self.buffer.as_bytes())
    }

    fn refresh_line(
        &mut self,
        prompt: &str,
        line: &LineBuffer,
        hint: Option<&str>,
        old_layout: &Layout,
        new_layout: &Layout,
        highlighter: Option<&dyn Highlighter>,
    ) -> Result<()> {
        use std::fmt::Write;
        self.buffer.clear();

        let default_prompt = new_layout.default_prompt;
        let cursor = new_layout.cursor;
        let end_pos = new_layout.end;

        self.clear_old_rows(old_layout);

        if let Some(highlighter) = highlighter {
            // display the prompt
            self.buffer
                .push_str(&highlighter.highlight_prompt(prompt, default_prompt));
            // display the input line
            self.buffer
                .push_str(&highlighter.highlight(line, line.pos()));
        } else {
            // display the prompt
            self.buffer.push_str(prompt);
            // display the input line
            self.buffer.push_str(line);
        }
        // display hint
        if let Some(hint) = hint {
            if let Some(highlighter) = highlighter {
                self.buffer.push_str(&highlighter.highlight_hint(hint));
            } else {
                self.buffer.push_str(hint);
            }
        }
        // we have to generate our own newline on line wrap
        if end_pos.col == 0 && end_pos.row > 0 && !self.buffer.ends_with('\n') {
            self.buffer.push_str("\n");
        }
        // position the cursor
        let new_cursor_row_movement = end_pos.row - cursor.row;
        // move the cursor up as required
        if new_cursor_row_movement > 0 {
            write!(self.buffer, "\x1b[{}A", new_cursor_row_movement).unwrap();
        }
        // position the cursor within the line
        if cursor.col > 0 {
            write!(self.buffer, "\r\x1b[{}C", cursor.col).unwrap();
        } else {
            self.buffer.push('\r');
        }

        self.write_and_flush(self.buffer.as_bytes())?;

        Ok(())
    }

    fn write_and_flush(&self, buf: &[u8]) -> Result<()> {
        write_and_flush(self.out, buf)
    }

    /// Control characters are treated as having zero width.
    /// Characters with 2 column width are correctly handled (not split).
    fn calculate_position(&self, s: &str, orig: Position) -> Position {
        let mut pos = orig;
        let mut esc_seq = 0;
        for c in s.graphemes(true) {
            if c == "\n" {
                pos.row += 1;
                pos.col = 0;
                continue;
            }
            let cw = if c == "\t" {
                self.tab_stop - (pos.col % self.tab_stop)
            } else {
                width(c, &mut esc_seq)
            };
            pos.col += cw;
            if pos.col > self.cols {
                pos.row += 1;
                pos.col = cw;
            }
        }
        if pos.col == self.cols {
            pos.col = 0;
            pos.row += 1;
        }
        pos
    }

    fn beep(&mut self) -> Result<()> {
        match self.bell_style {
            BellStyle::Audible => {
                io::stderr().write_all(b"\x07")?;
                io::stderr().flush()?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self) -> Result<()> {
        self.write_and_flush(b"\x1b[H\x1b[2J")
    }

    /// Check if a SIGWINCH signal has been received
    fn sigwinch(&self) -> bool {
        SIGWINCH.compare_and_swap(true, false, atomic::Ordering::SeqCst)
    }

    /// Try to update the number of columns in the current terminal,
    fn update_size(&mut self) {
        let (cols, _) = get_win_size(&self.out);
        self.cols = cols;
    }

    fn get_columns(&self) -> usize {
        self.cols
    }

    /// Try to get the number of rows in the current terminal,
    /// or assume 24 if it fails.
    fn get_rows(&self) -> usize {
        let (_, rows) = get_win_size(&self.out);
        rows
    }

    fn colors_enabled(&self) -> bool {
        self.colors_enabled
    }

    fn move_cursor_at_leftmost(&mut self, rdr: &mut PosixRawReader) -> Result<()> {
        if rdr.poll(0)? != 0 {
            debug!(target: "rustyline", "cannot request cursor location");
            return Ok(());
        }
        /* Report cursor location */
        self.write_and_flush(b"\x1b[6n")?;
        /* Read the response: ESC [ rows ; cols R */
        if rdr.poll(100)? == 0
            || rdr.next_char()? != '\x1b'
            || rdr.next_char()? != '['
            || read_digits_until(rdr, ';')?.is_none()
        {
            warn!(target: "rustyline", "cannot read initial cursor location");
            return Ok(());
        }
        let col = read_digits_until(rdr, 'R')?;
        debug!(target: "rustyline", "initial cursor location: {:?}", col);
        if col.is_some() && col != Some(1) {
            self.write_and_flush(b"\n")?;
        }
        Ok(())
    }
}

fn read_digits_until(rdr: &mut PosixRawReader, sep: char) -> Result<Option<u32>> {
    let mut num: u32 = 0;
    loop {
        match rdr.next_char()? {
            digit @ '0'..='9' => {
                num = num
                    .saturating_mul(10)
                    .saturating_add(digit.to_digit(10).unwrap());
                continue;
            }
            c if c == sep => break,
            _ => return Ok(None),
        }
    }
    Ok(Some(num))
}

static SIGWINCH_ONCE: sync::Once = sync::Once::new();
static SIGWINCH: atomic::AtomicBool = atomic::AtomicBool::new(false);

fn install_sigwinch_handler() {
    SIGWINCH_ONCE.call_once(|| unsafe {
        let sigwinch = signal::SigAction::new(
            signal::SigHandler::Handler(sigwinch_handler),
            signal::SaFlags::empty(),
            signal::SigSet::empty(),
        );
        let _ = signal::sigaction(signal::SIGWINCH, &sigwinch);
    });
}

extern "C" fn sigwinch_handler(_: libc::c_int) {
    SIGWINCH.store(true, atomic::Ordering::SeqCst);
    debug!(target: "rustyline", "SIGWINCH");
}

#[cfg(not(test))]
pub type Terminal = PosixTerminal;

#[derive(Clone, Debug)]
pub struct PosixTerminal {
    unsupported: bool,
    stdin_isatty: bool,
    stdstream_isatty: bool,
    pub(crate) color_mode: ColorMode,
    stream_type: OutputStreamType,
    tab_stop: usize,
    bell_style: BellStyle,
}

impl PosixTerminal {
    fn colors_enabled(&self) -> bool {
        match self.color_mode {
            ColorMode::Enabled => self.stdstream_isatty,
            ColorMode::Forced => true,
            ColorMode::Disabled => false,
        }
    }
}

impl Term for PosixTerminal {
    type Mode = PosixMode;
    type Reader = PosixRawReader;
    type Writer = PosixRenderer;

    fn new(
        color_mode: ColorMode,
        stream_type: OutputStreamType,
        tab_stop: usize,
        bell_style: BellStyle,
    ) -> Self {
        let term = Self {
            unsupported: is_unsupported_term(),
            stdin_isatty: is_a_tty(STDIN_FILENO),
            stdstream_isatty: is_a_tty(stream_type.as_raw_fd()),
            color_mode,
            stream_type,
            tab_stop,
            bell_style,
        };
        if !term.unsupported && term.stdin_isatty && term.stdstream_isatty {
            install_sigwinch_handler();
        }
        term
    }

    // Init checks:

    /// Check if current terminal can provide a rich line-editing user
    /// interface.
    fn is_unsupported(&self) -> bool {
        self.unsupported
    }

    /// check if stdin is connected to a terminal.
    fn is_stdin_tty(&self) -> bool {
        self.stdin_isatty
    }

    fn is_output_tty(&self) -> bool {
        self.stdstream_isatty
    }

    // Interactive loop:

    fn enable_raw_mode(&mut self) -> Result<Self::Mode> {
        use nix::errno::Errno::ENOTTY;
        use nix::sys::termios::{ControlFlags, InputFlags, LocalFlags, SpecialCharacterIndices};
        if !self.stdin_isatty {
            return Err(nix::Error::from_errno(ENOTTY).into());
        }
        let original_mode = termios::tcgetattr(STDIN_FILENO)?;
        let mut raw = original_mode.clone();
        // disable BREAK interrupt, CR to NL conversion on input,
        // input parity check, strip high bit (bit 8), output flow control
        raw.input_flags &= !(InputFlags::BRKINT
            | InputFlags::ICRNL
            | InputFlags::INPCK
            | InputFlags::ISTRIP
            | InputFlags::IXON);
        // we don't want raw output, it turns newlines into straight line feeds
        // disable all output processing
        // raw.c_oflag = raw.c_oflag & !(OutputFlags::OPOST);

        // character-size mark (8 bits)
        raw.control_flags |= ControlFlags::CS8;
        // disable echoing, canonical mode, extended input processing and signals
        raw.local_flags &=
            !(LocalFlags::ECHO | LocalFlags::ICANON | LocalFlags::IEXTEN | LocalFlags::ISIG);
        raw.control_chars[SpecialCharacterIndices::VMIN as usize] = 1; // One character-at-a-time input
        raw.control_chars[SpecialCharacterIndices::VTIME as usize] = 0; // with blocking read
        termios::tcsetattr(STDIN_FILENO, SetArg::TCSADRAIN, &raw)?;

        // enable bracketed paste
        let out = if let Err(e) = write_and_flush(self.stream_type, BRACKETED_PASTE_ON) {
            debug!(target: "rustyline", "Cannot enable bracketed paste: {}", e);
            None
        } else {
            Some(self.stream_type)
        };
        Ok(PosixMode {
            termios: original_mode,
            out,
        })
    }

    /// Create a RAW reader
    fn create_reader(&self, config: &Config) -> Result<PosixRawReader> {
        PosixRawReader::new(config)
    }

    fn create_writer(&self) -> PosixRenderer {
        PosixRenderer::new(
            self.stream_type,
            self.tab_stop,
            self.colors_enabled(),
            self.bell_style,
        )
    }
}

#[cfg(not(test))]
pub fn suspend() -> Result<()> {
    use nix::unistd::Pid;
    // suspend the whole process group
    signal::kill(Pid::from_raw(0), signal::SIGTSTP)?;
    Ok(())
}

fn write_and_flush(out: OutputStreamType, buf: &[u8]) -> Result<()> {
    match out {
        OutputStreamType::Stdout => {
            io::stdout().write_all(buf)?;
            io::stdout().flush()?;
        }
        OutputStreamType::Stderr => {
            io::stderr().write_all(buf)?;
            io::stderr().flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::{Position, PosixRenderer, PosixTerminal, Renderer};
    use crate::config::{BellStyle, OutputStreamType};
    use crate::line_buffer::LineBuffer;

    #[test]
    #[ignore]
    fn prompt_with_ansi_escape_codes() {
        let out = PosixRenderer::new(OutputStreamType::Stdout, 4, true, BellStyle::default());
        let pos = out.calculate_position("\x1b[1;32m>>\x1b[0m ", Position::default());
        assert_eq!(3, pos.col);
        assert_eq!(0, pos.row);
    }

    #[test]
    fn test_unsupported_term() {
        ::std::env::set_var("TERM", "xterm");
        assert_eq!(false, super::is_unsupported_term());

        ::std::env::set_var("TERM", "dumb");
        assert_eq!(true, super::is_unsupported_term());
    }

    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PosixTerminal>();
    }

    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PosixTerminal>();
    }

    #[test]
    fn test_line_wrap() {
        let mut out = PosixRenderer::new(OutputStreamType::Stdout, 4, true, BellStyle::default());
        let prompt = "> ";
        let default_prompt = true;
        let prompt_size = out.calculate_position(prompt, Position::default());

        let mut line = LineBuffer::init("", 0, None);
        let old_layout = out.compute_layout(prompt_size, default_prompt, &line, None);
        assert_eq!(Position { col: 2, row: 0 }, old_layout.cursor);
        assert_eq!(old_layout.cursor, old_layout.end);

        assert_eq!(Some(true), line.insert('a', out.cols - prompt_size.col + 1));
        let new_layout = out.compute_layout(prompt_size, default_prompt, &line, None);
        assert_eq!(Position { col: 1, row: 1 }, new_layout.cursor);
        assert_eq!(new_layout.cursor, new_layout.end);
        out.refresh_line(prompt, &line, None, &old_layout, &new_layout, None)
            .unwrap();
        #[rustfmt::skip]
        assert_eq!(
            "\r\u{1b}[0K> aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\r\u{1b}[1C",
            out.buffer
        );
    }
}
