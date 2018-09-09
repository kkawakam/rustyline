//! Unix specific definitions
use std;
use std::io::{self, Read, Stdout, Write};
use std::sync;
use std::sync::atomic;

use libc;
use nix;
use nix::poll::{self, EventFlags};
use nix::sys::signal;
use nix::sys::termios;
use nix::sys::termios::SetArg;
use unicode_segmentation::UnicodeSegmentation;
use utf8parse::{Parser, Receiver};

use super::{truncate, width, Position, RawMode, RawReader, Renderer, Term};
use config::{ColorMode, Config};
use error;
use highlight::Highlighter;
use keys::{self, KeyPress};
use line_buffer::LineBuffer;
use Result;

const STDIN_FILENO: libc::c_int = libc::STDIN_FILENO;
const STDOUT_FILENO: libc::c_int = libc::STDOUT_FILENO;

/// Unsupported Terminals that don't support RAW mode
static UNSUPPORTED_TERM: [&'static str; 3] = ["dumb", "cons25", "emacs"];

//#[allow(clippy::identity_conversion)]
fn get_win_size() -> (usize, usize) {
    use std::mem::zeroed;

    unsafe {
        let mut size: libc::winsize = zeroed();
        // https://github.com/rust-lang/libc/pull/704
        // FIXME: ".into()" used as a temporary fix for a libc bug
        match libc::ioctl(STDOUT_FILENO, libc::TIOCGWINSZ.into(), &mut size) {
            0 => (size.ws_col as usize, size.ws_row as usize), // TODO getCursorPosition
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
fn is_a_tty(fd: libc::c_int) -> bool {
    unsafe { libc::isatty(fd) != 0 }
}

pub type Mode = termios::Termios;

impl RawMode for Mode {
    /// Disable RAW mode for the terminal.
    fn disable_raw_mode(&self) -> Result<()> {
        try!(termios::tcsetattr(STDIN_FILENO, SetArg::TCSADRAIN, self));
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

impl PosixRawReader {
    fn new(config: &Config) -> Result<PosixRawReader> {
        Ok(PosixRawReader {
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

    /// Handle ESC <seq1> sequences
    fn escape_sequence(&mut self) -> Result<KeyPress> {
        // Read the next byte representing the escape sequence.
        let seq1 = try!(self.next_char());
        if seq1 == '[' {
            // ESC [ sequences. (CSI)
            self.escape_csi()
        } else if seq1 == 'O' {
            // xterm
            // ESC O sequences. (SS3)
            self.escape_o()
        } else if seq1 == '\x1b' {
            // ESC ESC
            Ok(KeyPress::Esc)
        } else {
            // TODO ESC-R (r): Undo all changes made to this line.
            Ok(KeyPress::Meta(seq1))
        }
    }

    /// Handle ESC [ <seq2> escape sequences
    fn escape_csi(&mut self) -> Result<KeyPress> {
        let seq2 = try!(self.next_char());
        if seq2.is_digit(10) {
            match seq2 {
                '0' | '9' => {
                    debug!(target: "rustyline", "unsupported esc sequence: ESC [ {:?}", seq2);
                    Ok(KeyPress::UnknownEscSeq)
                }
                _ => {
                    // Extended escape, read additional byte.
                    self.extended_escape(seq2)
                }
            }
        } else if seq2 == '[' {
            let seq3 = try!(self.next_char());
            // Linux console
            Ok(match seq3 {
                'A' => KeyPress::F(1),
                'B' => KeyPress::F(2),
                'C' => KeyPress::F(3),
                'D' => KeyPress::F(4),
                'E' => KeyPress::F(5),
                _ => {
                    debug!(target: "rustyline", "unsupported esc sequence: ESC [ [ {:?}", seq3);
                    KeyPress::UnknownEscSeq
                }
            })
        } else {
            // ANSI
            Ok(match seq2 {
                'A' => KeyPress::Up,    // kcuu1
                'B' => KeyPress::Down,  // kcud1
                'C' => KeyPress::Right, // kcuf1
                'D' => KeyPress::Left,  // kcub1
                'F' => KeyPress::End,
                'H' => KeyPress::Home, // khome
                'Z' => KeyPress::BackTab,
                _ => {
                    debug!(target: "rustyline", "unsupported esc sequence: ESC [ {:?}", seq2);
                    KeyPress::UnknownEscSeq
                }
            })
        }
    }

    /// Handle ESC [ <seq2:digit> escape sequences
    fn extended_escape(&mut self, seq2: char) -> Result<KeyPress> {
        let seq3 = try!(self.next_char());
        if seq3 == '~' {
            Ok(match seq2 {
                '1' | '7' => KeyPress::Home, // tmux, xrvt
                '2' => KeyPress::Insert,
                '3' => KeyPress::Delete,    // kdch1
                '4' | '8' => KeyPress::End, // tmux, xrvt
                '5' => KeyPress::PageUp,    // kpp
                '6' => KeyPress::PageDown,  // knp
                _ => {
                    debug!(target: "rustyline",
                           "unsupported esc sequence: ESC [ {} ~", seq2);
                    KeyPress::UnknownEscSeq
                }
            })
        } else if seq3.is_digit(10) {
            let seq4 = try!(self.next_char());
            if seq4 == '~' {
                Ok(match (seq2, seq3) {
                    ('1', '1') => KeyPress::F(1),  // rxvt-unicode
                    ('1', '2') => KeyPress::F(2),  // rxvt-unicode
                    ('1', '3') => KeyPress::F(3),  // rxvt-unicode
                    ('1', '4') => KeyPress::F(4),  // rxvt-unicode
                    ('1', '5') => KeyPress::F(5),  // kf5
                    ('1', '7') => KeyPress::F(6),  // kf6
                    ('1', '8') => KeyPress::F(7),  // kf7
                    ('1', '9') => KeyPress::F(8),  // kf8
                    ('2', '0') => KeyPress::F(9),  // kf9
                    ('2', '1') => KeyPress::F(10), // kf10
                    ('2', '3') => KeyPress::F(11), // kf11
                    ('2', '4') => KeyPress::F(12), // kf12
                    _ => {
                        debug!(target: "rustyline",
                               "unsupported esc sequence: ESC [ {}{} ~", seq2, seq3);
                        KeyPress::UnknownEscSeq
                    }
                })
            } else if seq4 == ';' {
                let seq5 = try!(self.next_char());
                if seq5.is_digit(10) {
                    let seq6 = try!(self.next_char()); // '~' expected
                    debug!(target: "rustyline",
                           "unsupported esc sequence: ESC [ {}{} ; {} {}", seq2, seq3, seq5, seq6);
                } else {
                    debug!(target: "rustyline",
                           "unsupported esc sequence: ESC [ {}{} ; {:?}", seq2, seq3, seq5);
                }
                Ok(KeyPress::UnknownEscSeq)
            } else {
                debug!(target: "rustyline",
                       "unsupported esc sequence: ESC [ {}{} {:?}", seq2, seq3, seq4);
                Ok(KeyPress::UnknownEscSeq)
            }
        } else if seq3 == ';' {
            let seq4 = try!(self.next_char());
            if seq4.is_digit(10) {
                let seq5 = try!(self.next_char());
                if seq2 == '1' {
                    Ok(match (seq4, seq5) {
                        ('5', 'A') => KeyPress::ControlUp,
                        ('5', 'B') => KeyPress::ControlDown,
                        ('5', 'C') => KeyPress::ControlRight,
                        ('5', 'D') => KeyPress::ControlLeft,
                        ('2', 'A') => KeyPress::ShiftUp,
                        ('2', 'B') => KeyPress::ShiftDown,
                        ('2', 'C') => KeyPress::ShiftRight,
                        ('2', 'D') => KeyPress::ShiftLeft,
                        _ => {
                            debug!(target: "rustyline",
                                   "unsupported esc sequence: ESC [ 1 ; {} {:?}", seq4, seq5);
                            KeyPress::UnknownEscSeq
                        }
                    })
                } else {
                    debug!(target: "rustyline",
                           "unsupported esc sequence: ESC [ {} ; {} {:?}", seq2, seq4, seq5);
                    Ok(KeyPress::UnknownEscSeq)
                }
            } else {
                debug!(target: "rustyline",
                       "unsupported esc sequence: ESC [ {} ; {:?}", seq2, seq4);
                Ok(KeyPress::UnknownEscSeq)
            }
        } else {
            Ok(match (seq2, seq3) {
                ('5', 'A') => KeyPress::ControlUp,
                ('5', 'B') => KeyPress::ControlDown,
                ('5', 'C') => KeyPress::ControlRight,
                ('5', 'D') => KeyPress::ControlLeft,
                _ => {
                    debug!(target: "rustyline",
                           "unsupported esc sequence: ESC [ {} {:?}", seq2, seq3);
                    KeyPress::UnknownEscSeq
                }
            })
        }
    }

    /// Handle ESC O <seq2> escape sequences
    fn escape_o(&mut self) -> Result<KeyPress> {
        let seq2 = try!(self.next_char());
        Ok(match seq2 {
            'A' => KeyPress::Up,    // kcuu1
            'B' => KeyPress::Down,  // kcud1
            'C' => KeyPress::Right, // kcuf1
            'D' => KeyPress::Left,  // kcub1
            'F' => KeyPress::End,   // kend
            'H' => KeyPress::Home,  // khome
            'P' => KeyPress::F(1),  // kf1
            'Q' => KeyPress::F(2),  // kf2
            'R' => KeyPress::F(3),  // kf3
            'S' => KeyPress::F(4),  // kf4
            'a' => KeyPress::ControlUp,
            'b' => KeyPress::ControlDown,
            'c' => KeyPress::ControlRight, // rxvt
            'd' => KeyPress::ControlLeft,  // rxvt
            _ => {
                debug!(target: "rustyline", "unsupported esc sequence: ESC O {:?}", seq2);
                KeyPress::UnknownEscSeq
            }
        })
    }
}

impl RawReader for PosixRawReader {
    fn next_key(&mut self, single_esc_abort: bool) -> Result<KeyPress> {
        let c = try!(self.next_char());

        let mut key = keys::char_to_key_press(c);
        if key == KeyPress::Esc {
            let timeout_ms = if single_esc_abort && self.timeout_ms == -1 {
                0
            } else {
                self.timeout_ms
            };
            let mut fds = [poll::PollFd::new(STDIN_FILENO, EventFlags::POLLIN)];
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
        debug!(target: "rustyline", "key: {:?}", key);
        Ok(key)
    }

    fn next_char(&mut self) -> Result<char> {
        loop {
            let n = try!(self.stdin.read(&mut self.buf));
            if n == 0 {
                return Err(error::ReadlineError::Eof);
            }
            let b = self.buf[0];
            self.parser.advance(&mut self.receiver, b);
            if !self.receiver.valid {
                return Err(error::ReadlineError::Utf8Error);
            } else if self.receiver.c.is_some() {
                return Ok(self.receiver.c.take().unwrap());
            }
        }
    }
}

impl Receiver for Utf8 {
    /// Called whenever a codepoint is parsed successfully
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
    out: Stdout,
    cols: usize, // Number of columns in terminal
    buffer: String,
}

impl PosixRenderer {
    fn new() -> PosixRenderer {
        let (cols, _) = get_win_size();
        PosixRenderer {
            out: io::stdout(),
            cols,
            buffer: String::with_capacity(1024),
        }
    }
}

impl Renderer for PosixRenderer {
    fn move_cursor(&mut self, old: Position, new: Position) -> Result<()> {
        use std::fmt::Write;
        let mut ab = String::new();
        if new.row > old.row {
            // move down
            let row_shift = new.row - old.row;
            if row_shift == 1 {
                ab.push_str("\x1b[B");
            } else {
                write!(ab, "\x1b[{}B", row_shift).unwrap();
            }
        } else if new.row < old.row {
            // move up
            let row_shift = old.row - new.row;
            if row_shift == 1 {
                ab.push_str("\x1b[A");
            } else {
                write!(ab, "\x1b[{}A", row_shift).unwrap();
            }
        }
        if new.col > old.col {
            // move right
            let col_shift = new.col - old.col;
            if col_shift == 1 {
                ab.push_str("\x1b[C");
            } else {
                write!(ab, "\x1b[{}C", col_shift).unwrap();
            }
        } else if new.col < old.col {
            // move left
            let col_shift = old.col - new.col;
            if col_shift == 1 {
                ab.push_str("\x1b[D");
            } else {
                write!(ab, "\x1b[{}D", col_shift).unwrap();
            }
        }
        self.write_and_flush(ab.as_bytes())
    }

    fn refresh_line(
        &mut self,
        prompt: &str,
        prompt_size: Position,
        line: &LineBuffer,
        hint: Option<String>,
        current_row: usize,
        old_rows: usize,
        highlighter: Option<&Highlighter>,
    ) -> Result<(Position, Position)> {
        use std::fmt::Write;
        self.buffer.clear();

        // calculate the position of the end of the input line
        let end_pos = self.calculate_position(line, prompt_size);
        // calculate the desired position of the cursor
        let cursor = self.calculate_position(&line[..line.pos()], prompt_size);

        // self.old_rows < self.cursor.row if the prompt spans multiple lines and if
        // this is the default State.
        let cursor_row_movement = old_rows.checked_sub(current_row).unwrap_or(0);
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

        if let Some(highlighter) = highlighter {
            // display the prompt
            self.buffer.push_str(&highlighter.highlight_prompt(prompt));
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
            let truncate = truncate(&hint, end_pos.col, self.cols);
            if let Some(highlighter) = highlighter {
                self.buffer.push_str(&highlighter.highlight_hint(truncate));
            } else {
                self.buffer.push_str(truncate);
            }
        }
        // we have to generate our own newline on line wrap
        if end_pos.col == 0 && end_pos.row > 0 {
            self.buffer.push_str("\n");
        }
        // position the cursor
        let cursor_row_movement = end_pos.row - cursor.row;
        // move the cursor up as required
        if cursor_row_movement > 0 {
            write!(self.buffer, "\x1b[{}A", cursor_row_movement).unwrap();
        }
        // position the cursor within the line
        if cursor.col > 0 {
            write!(self.buffer, "\r\x1b[{}C", cursor.col).unwrap();
        } else {
            self.buffer.push('\r');
        }

        try!(self.out.write_all(self.buffer.as_bytes()));
        try!(self.out.flush());
        Ok((cursor, end_pos))
    }

    fn write_and_flush(&mut self, buf: &[u8]) -> Result<()> {
        try!(self.out.write_all(buf));
        try!(self.out.flush());
        Ok(())
    }

    /// Control characters are treated as having zero width.
    /// Characters with 2 column width are correctly handled (not splitted).
    fn calculate_position(&self, s: &str, orig: Position) -> Position {
        let mut pos = orig;
        let mut esc_seq = 0;
        for c in s.graphemes(true) {
            if c == "\n" {
                pos.row += 1;
                pos.col = 0;
                continue;
            }
            let cw = width(c, &mut esc_seq);
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
        let (cols, _) = get_win_size();
        self.cols = cols;
    }

    fn get_columns(&self) -> usize {
        self.cols
    }

    /// Try to get the number of rows in the current terminal,
    /// or assume 24 if it fails.
    fn get_rows(&self) -> usize {
        let (_, rows) = get_win_size();
        rows
    }
}

static SIGWINCH_ONCE: sync::Once = sync::ONCE_INIT;
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

pub type Terminal = PosixTerminal;

#[derive(Clone, Debug)]
pub struct PosixTerminal {
    unsupported: bool,
    stdin_isatty: bool,
    stdout_isatty: bool,
    pub(crate) color_mode: ColorMode,
}

impl Term for PosixTerminal {
    type Mode = Mode;
    type Reader = PosixRawReader;
    type Writer = PosixRenderer;

    fn new(color_mode: ColorMode) -> PosixTerminal {
        let term = PosixTerminal {
            unsupported: is_unsupported_term(),
            stdin_isatty: is_a_tty(STDIN_FILENO),
            stdout_isatty: is_a_tty(STDOUT_FILENO),
            color_mode,
        };
        if !term.unsupported && term.stdin_isatty && term.stdout_isatty {
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

    /// Check if output supports colors.
    fn colors_enabled(&self) -> bool {
        match self.color_mode {
            ColorMode::Enabled => self.stdout_isatty,
            ColorMode::Forced => true,
            ColorMode::Disabled => false,
        }
    }

    // Interactive loop:

    fn enable_raw_mode(&mut self) -> Result<Mode> {
        use nix::errno::Errno::ENOTTY;
        use nix::sys::termios::{ControlFlags, InputFlags, LocalFlags, SpecialCharacterIndices};
        if !self.stdin_isatty {
            try!(Err(nix::Error::from_errno(ENOTTY)));
        }
        let original_mode = try!(termios::tcgetattr(STDIN_FILENO));
        let mut raw = original_mode.clone();
        // disable BREAK interrupt, CR to NL conversion on input,
        // input parity check, strip high bit (bit 8), output flow control
        raw.input_flags &= !(InputFlags::BRKINT
            | InputFlags::ICRNL
            | InputFlags::INPCK
            | InputFlags::ISTRIP
            | InputFlags::IXON);
        // we don't want raw output, it turns newlines into straight linefeeds
        // disable all output processing
        // raw.c_oflag = raw.c_oflag & !(OutputFlags::OPOST);

        // character-size mark (8 bits)
        raw.control_flags |= ControlFlags::CS8;
        // disable echoing, canonical mode, extended input processing and signals
        raw.local_flags &=
            !(LocalFlags::ECHO | LocalFlags::ICANON | LocalFlags::IEXTEN | LocalFlags::ISIG);
        raw.control_chars[SpecialCharacterIndices::VMIN as usize] = 1; // One character-at-a-time input
        raw.control_chars[SpecialCharacterIndices::VTIME as usize] = 0; // with blocking read
        try!(termios::tcsetattr(STDIN_FILENO, SetArg::TCSADRAIN, &raw));
        Ok(original_mode)
    }

    /// Create a RAW reader
    fn create_reader(&self, config: &Config) -> Result<PosixRawReader> {
        PosixRawReader::new(config)
    }

    fn create_writer(&self) -> PosixRenderer {
        PosixRenderer::new()
    }
}

#[cfg(unix)]
pub fn suspend() -> Result<()> {
    use nix::unistd::Pid;
    // suspend the whole process group
    try!(signal::kill(Pid::from_raw(0), signal::SIGTSTP));
    Ok(())
}

#[cfg(all(unix, test))]
mod test {
    use super::{Position, Renderer};
    use std::io::{self, Stdout};

    #[test]
    fn prompt_with_ansi_escape_codes() {
        let out = io::stdout();
        let pos = out.calculate_position("\x1b[1;32m>>\x1b[0m ", Position::default(), 80);
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
}
