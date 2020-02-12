//! Unix specific definitions
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
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
use unicode_width::UnicodeWidthStr;
use utf8parse::{Parser, Receiver};

use super::{RawMode, RawReader, Renderer, Term};
use crate::config::{BellStyle, ColorMode, Config, OutputStreamType};
use crate::error;
use crate::highlight::Highlighter;
use crate::keys::{self, Key, KeyMods, KeyPress};
use crate::layout::{Layout, Position};
use crate::line_buffer::LineBuffer;
use crate::tty::add_prompt_and_highlight;
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

#[allow(clippy::identity_conversion)]
fn get_win_size<T: AsRawFd + ?Sized>(fileno: &T) -> (usize, usize) {
    use std::mem::zeroed;

    unsafe {
        let mut size: libc::winsize = zeroed();
        match win_size(fileno.as_raw_fd(), &mut size) {
            Ok(0) => (size.ws_col as usize, size.ws_row as usize), // TODO getCursorPosition
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

// Escape sequences tend to be small (They also aren't all UTF8 but in practice
// they pretty much are)
type EscapeSeq = smallvec::SmallVec<[u8; 12]>;

/// Basically a dictionary from escape sequences to keys. It can also tell us if
/// a sequence cannot possibly result in
struct EscapeBindings {
    bindings: HashMap<EscapeSeq, KeyPress>,
    // Stores each prefix of a binding in bindings. For example: if we have
    // b"\x1b[18;2~" as a binding, `prefixes` will store `\x1b`, `\x1b[`,
    // `\x1b[1`, `\x1b[18`, `\x1b[18;`, and so on. We also use it to detect if
    // there's an ambiguous binding during startup.
    //
    // In an ideal world we'd use some prefix tree structure, but I surprisingly
    // couldn't find one that actually supported an `item_with_prefix_exists`
    // (or similar) function.
    //
    // This exists so we know when to emit an "unknown escape code" complaint.
    prefixes: HashSet<EscapeSeq>,
    max_len: usize,
}

impl EscapeBindings {
    pub fn common_unix() -> Self {
        let mut res = Self {
            bindings: HashMap::new(),
            prefixes: HashSet::new(),
            max_len: 0,
        };
        res.add_common_unix();
        res
    }

    // Note: the overwrite flag is always false during initialization. It means
    // we panic if there's a dupe or if one escape is the prefix of another. and
    // just here for the hard-coded binding list, that we don't accidentally
    // break various bindings when stuff inevitably gets added to the default
    // list. If we implement populating the binding set dynamically (via
    // terminfo, a config file, ...) then we should allow overwriting.
    //
    // Also this takes string input only because it debug formats much nicer. As
    // I mentioned above, escape sequences aren't generally utf8-encoded.
    fn bind(&mut self, binding: impl Into<KeyPress>, seq_str: &str, overwrite: bool) {
        let binding = binding.into();
        let seq = seq_str.as_bytes();
        assert!(!seq.is_empty());
        assert!(overwrite || !self.prefixes.contains(seq), "{:?}", seq_str);
        assert!(
            overwrite || !self.bindings.contains_key(seq),
            "{:?}",
            seq_str
        );
        if seq.len() > 1 {
            // Go in reverse so that we don't add the same prefixes again and
            // again --
            for i in (1..seq.len()).rev() {
                let existed = self.prefixes.insert(seq[..i].into());
                if existed {
                    break;
                }
                // Check if the thing we're adding is already blocked because it
                // has a prefix.
                assert!(
                    overwrite || !self.bindings.contains_key(&seq[..i]),
                    "{:?} is prefix of {:?}: {:?}",
                    &seq_str[..i],
                    seq_str,
                    binding
                );
            }
        }
        let existed = self.bindings.insert(seq.into(), binding);
        if !overwrite {
            assert_eq!(existed, None, "{:?} {:?}", seq_str, binding);
        }
        if seq.len() >= self.max_len {
            self.max_len = seq.len();
        }
    }

    // Ideally some of this would be read out of terminfo, but... that's a pain
    // and terminfo doesn't have entries for everything, and sometime's it's
    // just wrong anyway (*cough* iTerm *cough*). So instead we just add
    // more-or-less everything we care about to the set of bindings. That said,
    // this is actually fine. These are the strings that are coming out of the
    // terminal, so long as two terminals don't use the same strings to mean
    // different things, it won't be a problem that we have so many items in our list.
    fn add_common_unix(&mut self) {
        // Ansi, vt220+, xterm, ...
        self.bind(Key::Up, "\x1b[A", false);
        self.bind(Key::Down, "\x1b[B", false);
        self.bind(Key::Right, "\x1b[C", false);
        self.bind(Key::Left, "\x1b[D", false);
        self.bind(Key::End, "\x1b[F", false);
        self.bind(Key::Home, "\x1b[H", false);
        self.bind(Key::BackTab, "\x1b[Z", false);

        // v220-style special keys
        self.bind(Key::Home, "\x1b[1~", false);
        self.bind(Key::Insert, "\x1b[2~", false);
        self.bind(Key::Delete, "\x1b[3~", false);
        self.bind(Key::End, "\x1b[4~", false);
        self.bind(Key::PageUp, "\x1b[5~", false);
        self.bind(Key::PageDown, "\x1b[6~", false);

        // Apparently from tmux or rxvt? okay.
        self.bind(Key::Home, "\x1b[7~", false);
        self.bind(Key::End, "\x1b[8~", false);

        // xterm family "app mode"
        self.bind(Key::Up, "\x1bOA", false);
        self.bind(Key::Down, "\x1bOB", false);
        self.bind(Key::Right, "\x1bOC", false);
        self.bind(Key::Left, "\x1bOD", false);
        self.bind(Key::Home, "\x1bOH", false);
        self.bind(Key::End, "\x1bOF", false);

        // Present in the last version of this code, but I think i
        self.bind(Key::Up.ctrl(), "\x1bOa", false);
        self.bind(Key::Down.ctrl(), "\x1bOb", false);
        self.bind(Key::Right.ctrl(), "\x1bOc", false);
        self.bind(Key::Left.ctrl(), "\x1bOd", false);

        // vt100-style (F1-F4 are more common)
        self.bind(Key::F(1), "\x1bOP", false);
        self.bind(Key::F(2), "\x1bOQ", false);
        self.bind(Key::F(3), "\x1bOR", false);
        self.bind(Key::F(4), "\x1bOS", false);
        self.bind(Key::F(5), "\x1bOt", false);
        self.bind(Key::F(6), "\x1bOu", false);
        self.bind(Key::F(7), "\x1bOv", false);
        self.bind(Key::F(8), "\x1bOl", false);
        self.bind(Key::F(9), "\x1bOw", false);
        self.bind(Key::F(10), "\x1bOx", false);

        // linux console
        self.bind(Key::F(1), "\x1b[[A", false);
        self.bind(Key::F(2), "\x1b[[B", false);
        self.bind(Key::F(3), "\x1b[[C", false);
        self.bind(Key::F(4), "\x1b[[D", false);
        self.bind(Key::F(5), "\x1b[[E", false);
        // rxvt-family, but follows the v220 format
        self.bind(Key::F(1), "\x1b[11~", false);
        self.bind(Key::F(2), "\x1b[12~", false);
        self.bind(Key::F(3), "\x1b[13~", false);
        self.bind(Key::F(4), "\x1b[14~", false);
        self.bind(Key::F(5), "\x1b[15~", false);
        // these are common, though.
        self.bind(Key::F(6), "\x1b[17~", false);
        self.bind(Key::F(7), "\x1b[18~", false);
        self.bind(Key::F(8), "\x1b[19~", false);
        self.bind(Key::F(9), "\x1b[20~", false);
        self.bind(Key::F(10), "\x1b[21~", false);
        self.bind(Key::F(11), "\x1b[23~", false);
        self.bind(Key::F(12), "\x1b[24~", false);

        // RXVT among others
        self.bind(Key::Up.ctrl(), "\x1b[Oa", false);
        self.bind(Key::Down.ctrl(), "\x1b[Ob", false);
        self.bind(Key::Right.ctrl(), "\x1b[Oc", false);
        self.bind(Key::Left.ctrl(), "\x1b[Od", false);
        self.bind(Key::Home.ctrl(), "\x1b[7^", false);
        self.bind(Key::End.ctrl(), "\x1b[8^", false);

        self.bind(Key::Up.shift(), "\x1b[a", false);
        self.bind(Key::Down.shift(), "\x1b[b", false);
        self.bind(Key::Right.shift(), "\x1b[c", false);
        self.bind(Key::Left.shift(), "\x1b[d", false);
        self.bind(Key::Home.shift(), "\x1b[7$", false);
        self.bind(Key::End.shift(), "\x1b[8$", false);

        if cfg!(target_os = "macos") {
            // Ugh. These are annoying, since I like these terminals, but the
            // codes they send are annoying special cases, so we actually check
            // for them directly. Thankfully, they announce their presence via
            // the `TERM_PROGRAM` var.
            if let Ok(v) = std::env::var("TERM_PROGRAM") {
                debug!(target: "rustyline", "term program: {}", v);
                match v.as_str() {
                    "Apple_Terminal" => {
                        // Yep, really.
                        self.bind(Key::Left.meta(), "\x1bb", false);
                        self.bind(Key::Right.meta(), "\x1bf", false);
                    }
                    "iTerm.app" => {
                        self.bind(Key::Up.meta(), "\x1b\x1b[A", false);
                        self.bind(Key::Down.meta(), "\x1b\x1b[B", false);
                        self.bind(Key::Right.meta(), "\x1b\x1b[C", false);
                        self.bind(Key::Left.meta(), "\x1b\x1b[D", false);
                    }
                    _ => {}
                }
            }
        }

        // xterm style key mods. Some of these are in terminfos (kLFT3 and so
        // on), at least in the extensions section (e.g. pass -x to infocmp).
        // But they're all documented here:
        // https://invisible-island.net/xterm/ctlseqs/ctlseqs.html
        let mods = [
            ("2", KeyMods::SHIFT),
            // Bind alt to meta, since it should be harmless to do so, and might
            // prevent issues for someone.
            ("3", KeyMods::META),       // Alt
            ("4", KeyMods::META_SHIFT), // Alt + Shift
            ("5", KeyMods::CTRL),
            ("6", KeyMods::CTRL_SHIFT),
            ("7", KeyMods::CTRL_META),       // Ctrl + Alt
            ("8", KeyMods::CTRL_META_SHIFT), // Ctrl + Alt + Shift
            ("9", KeyMods::META),
            ("10", KeyMods::META_SHIFT),
            ("11", KeyMods::META),       // Meta + Alt
            ("12", KeyMods::META_SHIFT), // Meta + Alt + Shift
            ("13", KeyMods::CTRL_META),
            ("14", KeyMods::CTRL_META_SHIFT),
            ("15", KeyMods::CTRL_META),       // Meta + Ctrl + Alt
            ("16", KeyMods::CTRL_META_SHIFT), // Meta + Ctrl + Alt + Shift
        ];
        let keys = [
            ("A", Key::Up),
            ("B", Key::Down),
            ("C", Key::Right),
            ("D", Key::Left),
            ("H", Key::Home),
            ("F", Key::End),
        ];

        for &(num, m) in mods.iter() {
            for &(ch, k) in keys.iter() {
                // e.g. \E[1;2A
                self.bind(k.with_mods(m), &format!("\x1b[1;{}{}", num, ch), false);
            }
        }
        // still xterm, seems to be a slight variation...
        self.bind(Key::PageUp.shift(), "\x1b[5;2~", false);
        self.bind(Key::PageDown.shift(), "\x1b[6;2~", false);

        self.bind(Key::BracketedPasteStart, "\x1b[200~", false);
        self.bind(Key::BracketedPasteEnd, "\x1b[201~", false);
    }

    pub fn lookup(&self, v: &[u8]) -> EscapeSearchResult {
        if let Some(k) = self.bindings.get(v) {
            EscapeSearchResult::Matched(*k)
        } else if self.prefixes.contains(v) {
            EscapeSearchResult::IsPrefix
        } else {
            EscapeSearchResult::NoMatch
        }
    }
}

struct EscDebug<'a>(&'a [u8]);
impl<'a> std::fmt::Debug for EscDebug<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, &b) in self.0.iter().enumerate() {
            if i != 0 {
                f.write_str(" ")?;
            }
            if b == 0x1b {
                f.write_str("ESC")?;
            } else if b.is_ascii_graphic() && !b.is_ascii_whitespace() {
                write!(f, "{}", b as char)?;
            } else {
                write!(f, "'\\x{:02x}'", b)?;
            }
        }
        Ok(())
    }
}

/// Return value of `EscapeBinding::lookup`
#[derive(Clone, Copy)]
enum EscapeSearchResult {
    Matched(KeyPress),
    IsPrefix,
    NoMatch,
}

/// Console input reader
pub struct PosixRawReader {
    stdin: StdinRaw,
    timeout_ms: i32,
    buf: [u8; 1],
    parser: Parser,
    receiver: Utf8,
    escapes: EscapeBindings,
}

struct Utf8 {
    c: Option<char>,
    valid: bool,
}

impl PosixRawReader {
    fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            stdin: StdinRaw {},
            timeout_ms: config.keyseq_timeout(),
            buf: [0; 1],
            parser: Parser::new(),
            escapes: EscapeBindings::common_unix(),
            receiver: Utf8 {
                c: None,
                valid: true,
            },
        })
    }

    fn escape_sequence(&mut self) -> Result<KeyPress> {
        let mut buffer = EscapeSeq::new();
        buffer.push(b'\x1b');
        let mut first = true;
        loop {
            let c = self.next_char()?;
            buffer.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());

            match self.escapes.lookup(&buffer) {
                EscapeSearchResult::Matched(k) => {
                    return Ok(k);
                }
                EscapeSearchResult::IsPrefix => {}
                EscapeSearchResult::NoMatch => {
                    return if first {
                        // This is a bit cludgey, but works for now. Ideally we'd do
                        // this based on timing out on a read instead.
                        if c == '\x1b' {
                            // ESC ESC
                            Ok(KeyPress::ESC)
                        } else {
                            // TODO ESC-R (r): Undo all changes made to this line.
                            Ok(KeyPress::meta(c))
                        }
                    } else {
                        debug!(target: "rustyline", "unsupported esc sequence: {:?}", EscDebug(&buffer));
                        Ok(Key::UnknownEscSeq.into())
                    };
                }
            }
            first = false;
        }
    }

    fn poll(&mut self, timeout_ms: i32) -> ::nix::Result<i32> {
        let mut fds = [poll::PollFd::new(STDIN_FILENO, PollFlags::POLLIN)];
        poll::poll(&mut fds, timeout_ms)
    }
}

impl RawReader for PosixRawReader {
    fn next_key(&mut self, single_esc_abort: bool) -> Result<KeyPress> {
        let c = self.next_char()?;

        let mut key = keys::char_to_key_press(c);
        if key == KeyPress::ESC {
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
                    if key.key == Key::BracketedPasteEnd {
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
        let mut cursor = new_layout.cursor;
        let end_pos = new_layout.end;
        let current_row = old_layout.cursor.row;
        let old_rows = old_layout.end.row;

        // old_rows < cursor.row if the prompt spans multiple lines and if
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

        add_prompt_and_highlight(
            &mut self.buffer,
            highlighter,
            line,
            prompt,
            default_prompt,
            &new_layout,
            &mut cursor,
        );
        // display hint
        if let Some(hint) = hint {
            if let Some(highlighter) = highlighter {
                self.buffer.push_str(&highlighter.highlight_hint(hint));
            } else {
                self.buffer.push_str(hint);
            }
        }
        // position the cursor
        let new_cursor_row_movement = end_pos.row - cursor.row;
        // move the cursor up as required
        if new_cursor_row_movement > 0 {
            write!(self.buffer, "\x1b[{}A", new_cursor_row_movement).unwrap();
        }
        // position the cursor within the line
        if cursor.col == 0 {
            self.buffer.push('\r');
        } else {
            write!(self.buffer, "\r\x1b[{}C", cursor.col).unwrap();
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

fn width(s: &str, esc_seq: &mut u8) -> usize {
    if *esc_seq == 1 {
        if s == "[" {
            // CSI
            *esc_seq = 2;
        } else {
            // two-character sequence
            *esc_seq = 0;
        }
        0
    } else if *esc_seq == 2 {
        if s == ";" || (s.as_bytes()[0] >= b'0' && s.as_bytes()[0] <= b'9') {
            /*} else if s == "m" {
            // last
             *esc_seq = 0;*/
        } else {
            // not supported
            *esc_seq = 0;
        }
        0
    } else if s == "\x1b" {
        *esc_seq = 1;
        0
    } else if s == "\n" {
        0
    } else {
        s.width()
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
}
