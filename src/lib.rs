//! Readline for Rust
//!
//! This implementation is based on [Antirez's Linenoise](https://github.com/antirez/linenoise)
//!
//! # Example
//!
//! Usage
//!
//! ```
//! let mut rl = rustyline::Editor::new();
//! let readline = rl.readline(">> ");
//! match readline {
//!     Ok(line) => println!("Line: {:?}",line),
//!     Err(_)   => println!("No input"),
//! }
//! ```
#![feature(io)]
#![feature(unicode)]

extern crate libc;
#[cfg(unix)]
extern crate nix;
extern crate unicode_width;
#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate kernel32;

pub mod completion;
#[allow(non_camel_case_types)]
mod consts;
pub mod error;
pub mod history;
mod kill_ring;
pub mod line_buffer;

use std::fmt;
use std::io::{self, Read, Write};
#[cfg(windows)]
use std::marker::PhantomData;
use std::mem;
use std::path::Path;
use std::result;
#[cfg(unix)]
use std::sync;
use std::sync::atomic;
#[cfg(unix)]
use nix::sys::signal;
#[cfg(unix)]
use nix::sys::termios;

use completion::Completer;
use consts::KeyPress;
use history::History;
use line_buffer::{LineBuffer, MAX_LINE, WordAction};
use kill_ring::KillRing;

/// The error type for I/O and Linux Syscalls (Errno)
pub type Result<T> = result::Result<T, error::ReadlineError>;

#[cfg(unix)]
type Handle = ();
#[cfg(windows)]
type Handle = winapi::HANDLE;
#[cfg(windows)]
macro_rules! check {
    ($funcall:expr) => {
        {
        let rc = unsafe { $funcall };
        if rc == 0 {
            try!(Err(io::Error::last_os_error()));
        }
        rc
        }
    };
}

// Represent the state during line editing.
struct State<'out, 'prompt> {
    out: &'out mut Write,
    prompt: &'prompt str, // Prompt to display
    prompt_size: Position, // Prompt Unicode width and height
    line: LineBuffer, // Edited line buffer
    cursor: Position, // Cursor position (relative to the start of the prompt for `row`)
    cols: usize, // Number of columns in terminal
    old_rows: usize, // Number of rows used so far (from start of prompt to end of input)
    history_index: usize, // The history index we are currently editing
    snapshot: LineBuffer, // Current edited line before history browsing/completion
    output_handle: Handle, // output handle (for windows)
}

#[derive(Copy, Clone, Debug, Default)]
struct Position {
    col: usize,
    row: usize,
}

impl<'out, 'prompt> State<'out, 'prompt> {
    fn new(out: &'out mut Write,
           output_handle: Handle,
           prompt: &'prompt str,
           history_index: usize)
           -> State<'out, 'prompt> {
        let capacity = MAX_LINE;
        let cols = get_columns(output_handle);
        let prompt_size = calculate_position(prompt, Default::default(), cols);
        State {
            out: out,
            prompt: prompt,
            prompt_size: prompt_size,
            line: LineBuffer::with_capacity(capacity),
            cursor: prompt_size,
            cols: cols,
            old_rows: prompt_size.row,
            history_index: history_index,
            snapshot: LineBuffer::with_capacity(capacity),
            output_handle: output_handle,
        }
    }

    fn snapshot(&mut self) {
        mem::swap(&mut self.line, &mut self.snapshot);
    }

    fn backup(&mut self) {
        self.snapshot.backup(&self.line);
    }

    /// Rewrite the currently edited line accordingly to the buffer content,
    /// cursor position, and number of columns of the terminal.
    fn refresh_line(&mut self) -> Result<()> {
        let prompt_size = self.prompt_size;
        self.refresh(self.prompt, prompt_size)
    }

    fn refresh_prompt_and_line(&mut self, prompt: &str) -> Result<()> {
        let prompt_size = calculate_position(prompt, Default::default(), self.cols);
        self.refresh(prompt, prompt_size)
    }

    #[cfg(unix)]
    fn refresh(&mut self, prompt: &str, prompt_size: Position) -> Result<()> {
        use std::fmt::Write;

        // calculate the position of the end of the input line
        let end_pos = calculate_position(&self.line, prompt_size, self.cols);
        // calculate the desired position of the cursor
        let cursor = calculate_position(&self.line[..self.line.pos()], prompt_size, self.cols);

        let mut ab = String::new();

        let cursor_row_movement = self.old_rows - self.cursor.row;
        // move the cursor down as required
        if cursor_row_movement > 0 {
            write!(ab, "\x1b[{}B", cursor_row_movement).unwrap();
        }
        // clear old rows
        for _ in 0..self.old_rows {
            ab.push_str("\r\x1b[0K\x1b[1A");
        }
        // clear the line
        ab.push_str("\r\x1b[0K");

        // display the prompt
        ab.push_str(prompt);
        // display the input line
        ab.push_str(&self.line);
        // we have to generate our own newline on line wrap
        if end_pos.col == 0 && end_pos.row > 0 {
            ab.push_str("\n");
        }
        // position the cursor
        let cursor_row_movement = end_pos.row - cursor.row;
        // move the cursor up as required
        if cursor_row_movement > 0 {
            write!(ab, "\x1b[{}A", cursor_row_movement).unwrap();
        }
        // position the cursor within the line
        if cursor.col > 0 {
            write!(ab, "\r\x1b[{}C", cursor.col).unwrap();
        } else {
            ab.push('\r');
        }

        self.cursor = cursor;
        self.old_rows = end_pos.row;

        write_and_flush(self.out, ab.as_bytes())
    }

    #[cfg(windows)]
    fn refresh(&mut self, prompt: &str, prompt_size: Position) -> Result<()> {
        let handle = self.output_handle;
        if cfg!(test) && handle.is_null() {
            return Ok(());
        }
        // calculate the position of the end of the input line
        let end_pos = calculate_position(&self.line, prompt_size, self.cols);
        // calculate the desired position of the cursor
        let cursor = calculate_position(&self.line[..self.line.pos()], prompt_size, self.cols);

        // position at the start of the prompt, clear to end of previous input
        let mut info = unsafe { mem::zeroed() };
        check!(kernel32::GetConsoleScreenBufferInfo(handle, &mut info));
        info.dwCursorPosition.X = 0;
        info.dwCursorPosition.Y -= self.cursor.row as i16;
        check!(kernel32::SetConsoleCursorPosition(handle, info.dwCursorPosition));
        let mut _count = 0;
        check!(kernel32::FillConsoleOutputCharacterA(handle,
                                                 ' ' as winapi::CHAR,
                                                 (info.dwSize.X * (self.old_rows as i16 +1)) as winapi::DWORD,
                                                 info.dwCursorPosition,
                                                 &mut _count));
        let mut ab = String::new();
        // display the prompt
        ab.push_str(prompt); // TODO handle ansi escape code (SetConsoleTextAttribute)
        // display the input line
        ab.push_str(&self.line);
        try!(write_and_flush(self.out, ab.as_bytes()));

        // position the cursor
        check!(kernel32::GetConsoleScreenBufferInfo(handle, &mut info));
        info.dwCursorPosition.X = cursor.col as i16;
        info.dwCursorPosition.Y -= (end_pos.row - cursor.row) as i16;
        check!(kernel32::SetConsoleCursorPosition(handle, info.dwCursorPosition));

        self.cursor = cursor;
        self.old_rows = end_pos.row;

        Ok(())
    }

    fn update_columns(&mut self) {
        self.cols = get_columns(self.output_handle);
    }
}

impl<'out, 'prompt> fmt::Debug for State<'out, 'prompt> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("State")
            .field("prompt", &self.prompt)
            .field("prompt_size", &self.prompt_size)
            .field("buf", &self.line)
            .field("cursor", &self.cursor)
            .field("cols", &self.cols)
            .field("old_rows", &self.old_rows)
            .field("history_index", &self.history_index)
            .field("snapshot", &self.snapshot)
            .finish()
    }
}

/// Unsupported Terminals that don't support RAW mode
static UNSUPPORTED_TERM: [&'static str; 3] = ["dumb", "cons25", "emacs"];

/// Check to see if `fd` is a TTY
#[cfg(unix)]
fn is_a_tty(fd: libc::c_int) -> bool {
    unsafe { libc::isatty(fd) != 0 }
}
#[cfg(windows)]
fn is_a_tty(fd: winapi::DWORD) -> bool {
    let handle = get_std_handle(fd);
    match handle {
        Ok(handle) => {
            // If this function doesn't fail then fd is a TTY
            get_console_mode(handle).is_ok()
        }
        Err(_) => false,
    }
}

/// Check to see if the current `TERM` is unsupported
fn is_unsupported_term() -> bool {
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

#[cfg(unix)]
type Mode = termios::Termios;
#[cfg(unix)]
const STDIN_FILENO: libc::c_int = libc::STDIN_FILENO;
#[cfg(unix)]
const STDOUT_FILENO: libc::c_int = libc::STDOUT_FILENO;
#[cfg(windows)]
type Mode = winapi::DWORD;
#[cfg(windows)]
const STDIN_FILENO: winapi::DWORD = winapi::STD_INPUT_HANDLE;
#[cfg(windows)]
const STDOUT_FILENO: winapi::DWORD = winapi::STD_OUTPUT_HANDLE;
#[cfg(windows)]
fn get_std_handle(fd: winapi::DWORD) -> Result<winapi::HANDLE> {
    let handle = unsafe { kernel32::GetStdHandle(fd) };
    if handle == winapi::INVALID_HANDLE_VALUE {
        try!(Err(io::Error::last_os_error()));
    } else if handle.is_null() {
        try!(Err(io::Error::new(io::ErrorKind::Other,
                                "no stdio handle available for this process")));
    }
    Ok(handle)
}

/// Enable raw mode for the TERM
#[cfg(unix)]
fn enable_raw_mode() -> Result<Mode> {
    use nix::errno::Errno::ENOTTY;
    use nix::sys::termios::{BRKINT, CS8, ECHO, ICANON, ICRNL, IEXTEN, INPCK, ISIG, ISTRIP, IXON,
                            /* OPOST, */ VMIN, VTIME};
    if !is_a_tty(STDIN_FILENO) {
        try!(Err(nix::Error::from_errno(ENOTTY)));
    }
    let original_term = try!(termios::tcgetattr(STDIN_FILENO));
    let mut raw = original_term;
    raw.c_iflag = raw.c_iflag & !(BRKINT | ICRNL | INPCK | ISTRIP | IXON); // disable BREAK interrupt, CR to NL conversion on input, input parity check, strip high bit (bit 8), output flow control
    // we don't want raw output, it turns newlines into straight linefeeds
    //raw.c_oflag = raw.c_oflag & !(OPOST); // disable all output processing
    raw.c_cflag = raw.c_cflag | (CS8); // character-size mark (8 bits)
    raw.c_lflag = raw.c_lflag & !(ECHO | ICANON | IEXTEN | ISIG); // disable echoing, canonical mode, extended input processing and signals
    raw.c_cc[VMIN] = 1; // One character-at-a-time input
    raw.c_cc[VTIME] = 0; // with blocking read
    try!(termios::tcsetattr(STDIN_FILENO, termios::TCSAFLUSH, &raw));
    Ok(original_term)
}
#[cfg(windows)]
fn enable_raw_mode() -> Result<Mode> {
    let handle = try!(get_std_handle(STDIN_FILENO));
    let original_mode = try!(get_console_mode(handle));
    let raw = original_mode &
              !(winapi::wincon::ENABLE_LINE_INPUT | winapi::wincon::ENABLE_ECHO_INPUT |
                winapi::wincon::ENABLE_PROCESSED_INPUT);
    check!(kernel32::SetConsoleMode(handle, raw));
    Ok(original_mode)
}
#[cfg(windows)]
fn get_console_mode(handle: winapi::HANDLE) -> Result<Mode> {
    let mut original_mode = 0;
    check!(kernel32::GetConsoleMode(handle, &mut original_mode));
    Ok(original_mode)
}

/// Disable Raw mode for the term
#[cfg(unix)]
fn disable_raw_mode(original_mode: Mode) -> Result<()> {
    try!(termios::tcsetattr(STDIN_FILENO, termios::TCSAFLUSH, &original_mode));
    Ok(())
}
#[cfg(windows)]
fn disable_raw_mode(original_mode: Mode) -> Result<()> {
    let handle = try!(get_std_handle(STDIN_FILENO));
    check!(kernel32::SetConsoleMode(handle, original_mode));
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
const TIOCGWINSZ: libc::c_ulong = 0x40087468;

#[cfg(any(target_os = "linux", target_os = "android"))]
const TIOCGWINSZ: libc::c_ulong = 0x5413;

/// Try to get the number of columns in the current terminal,
/// or assume 80 if it fails.
#[cfg(any(target_os = "linux",
          target_os = "android",
          target_os = "macos",
          target_os = "freebsd"))]
fn get_columns(_: Handle) -> usize {
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

        let mut size: winsize = mem::zeroed();
        match libc::ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut size) {
            0 => size.ws_col as usize, // TODO getCursorPosition
            _ => 80,
        }
    }
}
#[cfg(windows)]
fn get_columns(handle: Handle) -> usize {
    let mut info = unsafe { mem::zeroed() };
    match unsafe { kernel32::GetConsoleScreenBufferInfo(handle, &mut info) } {
        0 => 80,
        _ => info.dwSize.X as usize,
    }
}

fn write_and_flush(w: &mut Write, buf: &[u8]) -> Result<()> {
    try!(w.write_all(buf));
    try!(w.flush());
    Ok(())
}

/// Clear the screen. Used to handle ctrl+l
#[cfg(unix)]
fn clear_screen(s: &mut State) -> Result<()> {
    write_and_flush(s.out, b"\x1b[H\x1b[2J")
}
#[cfg(windows)]
fn clear_screen(s: &mut State) -> Result<()> {
    let handle = s.output_handle;
    let mut info = unsafe { mem::zeroed() };
    check!(kernel32::GetConsoleScreenBufferInfo(handle, &mut info));
    let coord = winapi::COORD { X: 0, Y: 0 };
    check!(kernel32::SetConsoleCursorPosition(handle, coord));
    let mut _count = 0;
    check!(kernel32::FillConsoleOutputCharacterA(handle,
                                                 ' ' as winapi::CHAR,
                                                 (info.dwSize.X * info.dwSize.Y) as winapi::DWORD,
                                                 coord,
                                                 &mut _count));
    Ok(())
}

/// Beep, used for completion when there is nothing to complete or when all
/// the choices were already shown.
fn beep() -> Result<()> {
    write_and_flush(&mut io::stderr(), b"\x07") // TODO bell-style
}

/// Calculate the number of columns and rows used to display `s` on a `cols` width terminal
/// starting at `orig`.
/// Control characters are treated as having zero width.
/// Characters with 2 column width are correctly handled (not splitted).
#[cfg_attr(feature="clippy", allow(if_same_then_else))]
fn calculate_position(s: &str, orig: Position, cols: usize) -> Position {
    let mut pos = orig;
    let mut esc_seq = 0;
    for c in s.chars() {
        let cw = if esc_seq == 1 {
            if c == '[' {
                // CSI
                esc_seq = 2;
            } else {
                // two-character sequence
                esc_seq = 0;
            }
            None
        } else if esc_seq == 2 {
            if c == ';' || (c >= '0' && c <= '9') {
            } else if c == 'm' {
                // last
                esc_seq = 0;
            } else {
                // not supported
                esc_seq = 0;
            }
            None
        } else if c == '\x1b' {
            esc_seq = 1;
            None
        } else if c == '\n' {
            pos.col = 0;
            pos.row += 1;
            None
        } else {
            unicode_width::UnicodeWidthChar::width(c)
        };
        if let Some(cw) = cw {
            pos.col += cw;
            if pos.col > cols {
                pos.row += 1;
                pos.col = cw;
            }
        }
    }
    if pos.col == cols {
        pos.col = 0;
        pos.row += 1;
    }
    pos
}

/// Insert the character `ch` at cursor current position.
fn edit_insert(s: &mut State, ch: char) -> Result<()> {
    if let Some(push) = s.line.insert(ch) {
        if push {
            if s.cursor.col + unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) < s.cols {
                // Avoid a full update of the line in the trivial case.
                let cursor = calculate_position(&s.line[..s.line.pos()], s.prompt_size, s.cols);
                s.cursor = cursor;
                let bits = ch.encode_utf8();
                let bits = bits.as_slice();
                write_and_flush(s.out, bits)
            } else {
                s.refresh_line()
            }
        } else {
            s.refresh_line()
        }
    } else {
        Ok(())
    }
}

// Yank/paste `text` at current position.
fn edit_yank(s: &mut State, text: &str) -> Result<()> {
    if let Some(_) = s.line.yank(text) {
        s.refresh_line()
    } else {
        Ok(())
    }
}

// Delete previously yanked text and yank/paste `text` at current position.
fn edit_yank_pop(s: &mut State, yank_size: usize, text: &str) -> Result<()> {
    s.line.yank_pop(yank_size, text);
    edit_yank(s, text)
}

/// Move cursor on the left.
fn edit_move_left(s: &mut State) -> Result<()> {
    if s.line.move_left() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Move cursor on the right.
fn edit_move_right(s: &mut State) -> Result<()> {
    if s.line.move_right() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Move cursor to the start of the line.
fn edit_move_home(s: &mut State) -> Result<()> {
    if s.line.move_home() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Move cursor to the end of the line.
fn edit_move_end(s: &mut State) -> Result<()> {
    if s.line.move_end() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Delete the character at the right of the cursor without altering the cursor
/// position. Basically this is what happens with the "Delete" keyboard key.
fn edit_delete(s: &mut State) -> Result<()> {
    if s.line.delete() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Backspace implementation.
fn edit_backspace(s: &mut State) -> Result<()> {
    if s.line.backspace() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Kill the text from point to the end of the line.
fn edit_kill_line(s: &mut State) -> Result<Option<String>> {
    if let Some(text) = s.line.kill_line() {
        try!(s.refresh_line());
        Ok(Some(text))
    } else {
        Ok(None)
    }
}

/// Kill backward from point to the beginning of the line.
fn edit_discard_line(s: &mut State) -> Result<Option<String>> {
    if let Some(text) = s.line.discard_line() {
        try!(s.refresh_line());
        Ok(Some(text))
    } else {
        Ok(None)
    }
}

/// Exchange the char before cursor with the character at cursor.
fn edit_transpose_chars(s: &mut State) -> Result<()> {
    if s.line.transpose_chars() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

fn edit_move_to_prev_word(s: &mut State) -> Result<()> {
    if s.line.move_to_prev_word() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Delete the previous word, maintaining the cursor at the start of the
/// current word.
fn edit_delete_prev_word<F>(s: &mut State, test: F) -> Result<Option<String>>
    where F: Fn(char) -> bool
{
    if let Some(text) = s.line.delete_prev_word(test) {
        try!(s.refresh_line());
        Ok(Some(text))
    } else {
        Ok(None)
    }
}

fn edit_move_to_next_word(s: &mut State) -> Result<()> {
    if s.line.move_to_next_word() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Kill from the cursor to the end of the current word, or, if between words, to the end of the next word.
fn edit_delete_word(s: &mut State) -> Result<Option<String>> {
    if let Some(text) = s.line.delete_word() {
        try!(s.refresh_line());
        Ok(Some(text))
    } else {
        Ok(None)
    }
}

fn edit_word(s: &mut State, a: WordAction) -> Result<()> {
    if s.line.edit_word(a) {
        s.refresh_line()
    } else {
        Ok(())
    }
}

fn edit_transpose_words(s: &mut State) -> Result<()> {
    if s.line.transpose_words() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Substitute the currently edited line with the next or previous history
/// entry.
fn edit_history_next(s: &mut State, history: &History, prev: bool) -> Result<()> {
    if history.is_empty() {
        return Ok(());
    }
    if s.history_index == history.len() {
        if prev {
            // Save the current edited line before to overwrite it
            s.snapshot();
        } else {
            return Ok(());
        }
    } else if s.history_index == 0 && prev {
        return Ok(());
    }
    if prev {
        s.history_index -= 1;
    } else {
        s.history_index += 1;
    }
    if s.history_index < history.len() {
        let buf = history.get(s.history_index).unwrap();
        s.line.update(buf, buf.len());
    } else {
        // Restore current edited line
        s.snapshot();
    };
    s.refresh_line()
}

/// Completes the line/word
fn complete_line<R: Read>(rdr: &mut RawReader<R>,
                          s: &mut State,
                          completer: &Completer)
                          -> Result<Option<KeyPress>> {
    let (start, candidates) = try!(completer.complete(&s.line, s.line.pos()));
    if candidates.is_empty() {
        try!(beep());
        Ok(None)
    } else {
        // Save the current edited line before to overwrite it
        s.backup();
        let mut key;
        let mut i = 0;
        loop {
            // Show completion or original buffer
            if i < candidates.len() {
                completer.update(&mut s.line, start, &candidates[i]);
                try!(s.refresh_line());
            } else {
                // Restore current edited line
                s.snapshot();
                try!(s.refresh_line());
                s.snapshot();
            }

            key = try!(rdr.next_key());
            match key {
                KeyPress::TAB => {
                    i = (i + 1) % (candidates.len() + 1); // Circular
                    if i == candidates.len() {
                        try!(beep());
                    }
                }
                KeyPress::ESC => {
                    // Re-show original buffer
                    s.snapshot();
                    if i < candidates.len() {
                        try!(s.refresh_line());
                    }
                    return Ok(None);
                }
                _ => {
                    break;
                }
            }
        }
        Ok(Some(key))
    }
}

/// Incremental search
#[cfg_attr(feature="clippy", allow(if_not_else))]
fn reverse_incremental_search<R: Read>(rdr: &mut RawReader<R>,
                                       s: &mut State,
                                       history: &History)
                                       -> Result<Option<KeyPress>> {
    if history.is_empty() {
        return Ok(None);
    }
    // Save the current edited line (and cursor position) before to overwrite it
    s.snapshot();

    let mut search_buf = String::new();
    let mut history_idx = history.len() - 1;
    let mut reverse = true;
    let mut success = true;

    let mut key;
    // Display the reverse-i-search prompt and process chars
    loop {
        let prompt = if success {
            format!("(reverse-i-search)`{}': ", search_buf)
        } else {
            format!("(failed reverse-i-search)`{}': ", search_buf)
        };
        try!(s.refresh_prompt_and_line(&prompt));

        key = try!(rdr.next_key());
        if let KeyPress::Char(c) = key {
            search_buf.push(c);
        } else {
            match key {
                KeyPress::CTRL_H | KeyPress::BACKSPACE => {
                    search_buf.pop();
                    continue;
                }
                KeyPress::CTRL_R => {
                    reverse = true;
                    if history_idx > 0 {
                        history_idx -= 1;
                    } else {
                        success = false;
                        continue;
                    }
                }
                KeyPress::CTRL_S => {
                    reverse = false;
                    if history_idx < history.len() - 1 {
                        history_idx += 1;
                    } else {
                        success = false;
                        continue;
                    }
                }
                KeyPress::CTRL_G => {
                    // Restore current edited line (before search)
                    s.snapshot();
                    try!(s.refresh_line());
                    return Ok(None);
                }
                _ => break,
            }
        }
        success = match history.search(&search_buf, history_idx, reverse) {
            Some(idx) => {
                history_idx = idx;
                let entry = history.get(idx).unwrap();
                let pos = entry.find(&search_buf).unwrap();
                s.line.update(entry, pos);
                true
            }
            _ => false,
        };
    }
    Ok(Some(key))
}

/// Console input reader
#[cfg(unix)]
struct RawReader<R> {
    chars: io::Chars<R>,
}

#[cfg(unix)]
impl<R: Read> RawReader<R> {
    fn new(stdin: R) -> Result<RawReader<R>> {
        Ok(RawReader { chars: stdin.chars() })
    }

    fn next_key(&mut self) -> Result<KeyPress> {
        use consts::char_to_key_press;

        let c = try!(self.next_char());
        if !c.is_control() {
            return Ok(KeyPress::Char(c));
        }

        let mut key = char_to_key_press(c);
        if key == KeyPress::ESC {
            // escape sequence
            key = try!(self.escape_sequence());
        }
        Ok(key)
    }

    fn next_char(&mut self) -> Result<char> {
        match self.chars.next() {
            Some(c) => {
                Ok(try!(c)) // TODO SIGWINCH
            }
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
                        '3' => Ok(KeyPress::ESC_SEQ_DELETE),
                        // TODO '1' // Home
                        // TODO '4' // End
                        _ => Ok(KeyPress::UNKNOWN_ESC_SEQ),
                    }
                } else {
                    Ok(KeyPress::UNKNOWN_ESC_SEQ)
                }
            } else {
                match seq2 {
                    'A' => Ok(KeyPress::CTRL_P), // Up
                    'B' => Ok(KeyPress::CTRL_N), // Down
                    'C' => Ok(KeyPress::CTRL_F), // Right
                    'D' => Ok(KeyPress::CTRL_B), // Left
                    'F' => Ok(KeyPress::CTRL_E), // End
                    'H' => Ok(KeyPress::CTRL_A), // Home
                    _ => Ok(KeyPress::UNKNOWN_ESC_SEQ),
                }
            }
        } else if seq1 == 'O' {
            // ESC O sequences.
            let seq2 = try!(self.next_char());
            match seq2 {
                'F' => Ok(KeyPress::CTRL_E),
                'H' => Ok(KeyPress::CTRL_A),
                _ => Ok(KeyPress::UNKNOWN_ESC_SEQ),
            }
        } else {
            // TODO ESC-N (n): search history forward not interactively
            // TODO ESC-P (p): search history backward not interactively
            // TODO ESC-R (r): Undo all changes made to this line.
            // TODO ESC-<: move to first entry in history
            // TODO ESC->: move to last entry in history
            match seq1 {
                'b' | 'B' => Ok(KeyPress::ESC_B),
                'c' | 'C' => Ok(KeyPress::ESC_C),
                'd' | 'D' => Ok(KeyPress::ESC_D),
                'f' | 'F' => Ok(KeyPress::ESC_F),
                'l' | 'L' => Ok(KeyPress::ESC_L),
                't' | 'T' => Ok(KeyPress::ESC_T),
                'u' | 'U' => Ok(KeyPress::ESC_U),
                'y' | 'Y' => Ok(KeyPress::ESC_Y),
                '\x08' | '\x7f' => Ok(KeyPress::ESC_BACKSPACE),
                _ => {
                    // writeln!(io::stderr(), "key: {:?}, seq1: {:?}", KeyPress::ESC, seq1).unwrap();
                    Ok(KeyPress::UNKNOWN_ESC_SEQ)
                }
            }
        }
    }
}

#[cfg(windows)]
struct RawReader<R> {
    handle: winapi::HANDLE,
    buf: Option<u16>,
    phantom: PhantomData<R>,
}

#[cfg(windows)]
impl<R: Read> RawReader<R> {
    fn new(stdin: R) -> Result<RawReader<R>> {
        let handle = try!(get_std_handle(STDIN_FILENO));
        Ok(RawReader {
            handle: handle,
            buf: None,
        })
    }

    fn next_key(&mut self) -> Result<KeyPress> {
        use std::char::decode_utf16;

        let mut rec: winapi::INPUT_RECORD = unsafe { mem::zeroed() };
        let mut count = 0;
        let mut esc_seen = false;
        loop {
            check!(kernel32::ReadConsoleInputW(self.0, &mut rec, 1 as winapi::DWORD, &mut count));

            // TODO ENABLE_WINDOW_INPUT ???
            if rec.EventType == winapi::WINDOW_BUFFER_SIZE_EVENT {
                SIGWINCH.store(true, atomic::Ordering::SeqCst);
                return Err(error::ReadlineError::BufferSizeEvent);
            } else if rec.EventType != winapi::KEY_EVENT {
                continue;
            }
            let key_event = unsafe { rec.KeyEvent() };
            if key_event.bKeyDown == 0 &&
               key_event.wVirtualKeyCode != winapi::VK_MENU as winapi::WORD {
                continue;
            }

            let key_state = self.key_state.borrow_mut();
            let ctrl = key_event.dwControlKeyState &
                       (winapi::LEFT_CTRL_PRESSED | winapi::RIGHT_CTRL_PRESSED) ==
                       (winapi::LEFT_CTRL_PRESSED | winapi::RIGHT_CTRL_PRESSED);
            let meta = (key_event.dwControlKeyState &
                        (winapi::LEFT_ALT_PRESSED | winapi::RIGHT_ALT_PRESSED) ==
                        (winapi::LEFT_ALT_PRESSED | winapi::RIGHT_ALT_PRESSED)) ||
                       esc_seen;

            // TODO How to support surrogate pair ?
            let utf16 = key_event.UnicodeChar;
            if utf16 == 0 {
                match key_event.wVirtualKeyCode as i32 {
                    winapi::VK_LEFT => return Ok(KeyPress::CTRL_B),
                    winapi::VK_RIGHT => return Ok(KeyPress::CTRL_F),
                    winapi::VK_UP => return Ok(KeyPress::CTRL_P),
                    winapi::VK_DOWN => return Ok(KeyPress::CTRL_N),
                    winapi::VK_DELETE => return Ok(KeyPress::ESC_SEQ_DELETE),
                    winapi::VK_HOME => return Ok(KeyPress::CTRL_A),
                    winapi::VK_END => return Ok(KeyPress::CTRL_E),
                    _ => continue,
                };
            } else if utf16 == 27 {
                esc_seen = true;
                continue;
            } else {
                if ctrl {
                    unimplemented!()
                } else if meta {
                    unimplemented!()
                } else {
                    self.buf = Some(utf16);
                    match decode_utf16(self).next() {
                        Some(item) => Ok(KeyPress::Char(try!(item))),
                        None => return Err(error::ReadlineError::Eof),
                    }
                }
                let (bytes, len) = try!(RawReader::wide_char_to_multi_byte(utf16));
                return (&bytes[..len]).read(buf);
            }
        }
    }
}
#[cfg(windows)]
impl<R: Read> Iterator for RawReader<R> {
    type Item = u16;

    fn next(&mut self) -> Option<u16> {
        let buf = self.buf;
        self.buf = None;
        buf
    }
}

#[cfg(unix)]
fn stdout_handle() -> Result<Handle> {
    Ok(())
}
#[cfg(windows)]
fn stdout_handle() -> Result<Handle> {
    let handle = try!(get_std_handle(STDOUT_FILENO));
    Ok(handle)
}

/// Handles reading and editting the readline buffer.
/// It will also handle special inputs in an appropriate fashion
/// (e.g., C-c will exit readline)
#[cfg_attr(feature="clippy", allow(cyclomatic_complexity))]
fn readline_edit(prompt: &str,
                 history: &mut History,
                 completer: Option<&Completer>,
                 kill_ring: &mut KillRing,
                 original_mode: Mode)
                 -> Result<String> {
    let mut stdout = io::stdout();
    let stdout_handle = try!(stdout_handle());

    kill_ring.reset();
    let mut s = State::new(&mut stdout, stdout_handle, prompt, history.len());
    try!(s.refresh_line());

    let mut rdr = try!(RawReader::new(io::stdin()));

    loop {
        let rk = rdr.next_key();
        if rk.is_err() && SIGWINCH.compare_and_swap(true, false, atomic::Ordering::SeqCst) {
            s.update_columns();
            try!(s.refresh_line());
            continue;
        }
        let mut key = try!(rk);
        if let KeyPress::Char(c) = key {
            kill_ring.reset();
            try!(edit_insert(&mut s, c));
            continue;
        }

        // autocomplete
        if key == KeyPress::TAB && completer.is_some() {
            let next = try!(complete_line(&mut rdr, &mut s, completer.unwrap()));
            if next.is_some() {
                kill_ring.reset();
                key = next.unwrap();
                if let KeyPress::Char(c) = key {
                    try!(edit_insert(&mut s, c));
                    continue;
                }
            } else {
                continue;
            }
        } else if key == KeyPress::CTRL_R {
            // Search history backward
            let next = try!(reverse_incremental_search(&mut rdr, &mut s, history));
            if next.is_some() {
                key = next.unwrap();
            } else {
                continue;
            }
        } else if key == KeyPress::UNKNOWN_ESC_SEQ {
            continue;
        }

        match key {
            KeyPress::CTRL_A => {
                kill_ring.reset();
                // Move to the beginning of line.
                try!(edit_move_home(&mut s))
            }
            KeyPress::CTRL_B => {
                kill_ring.reset();
                // Move back a character.
                try!(edit_move_left(&mut s))
            }
            KeyPress::CTRL_C => {
                kill_ring.reset();
                return Err(error::ReadlineError::Interrupted);
            }
            KeyPress::CTRL_D => {
                kill_ring.reset();
                if s.line.is_empty() {
                    return Err(error::ReadlineError::Eof);
                } else {
                    // Delete (forward) one character at point.
                    try!(edit_delete(&mut s))
                }
            }
            KeyPress::CTRL_E => {
                kill_ring.reset();
                // Move to the end of line.
                try!(edit_move_end(&mut s))
            }
            KeyPress::CTRL_F => {
                kill_ring.reset();
                // Move forward a character.
                try!(edit_move_right(&mut s))
            }
            KeyPress::CTRL_H | KeyPress::BACKSPACE => {
                kill_ring.reset();
                // Delete one character backward.
                try!(edit_backspace(&mut s))
            }
            KeyPress::CTRL_K => {
                // Kill the text from point to the end of the line.
                if let Some(text) = try!(edit_kill_line(&mut s)) {
                    kill_ring.kill(&text, true)
                }
            }
            KeyPress::CTRL_L => {
                // Clear the screen leaving the current line at the top of the screen.
                try!(clear_screen(&mut s));
                try!(s.refresh_line())
            }
            KeyPress::CTRL_N => {
                kill_ring.reset();
                // Fetch the next command from the history list.
                try!(edit_history_next(&mut s, history, false))
            }
            KeyPress::CTRL_P => {
                kill_ring.reset();
                // Fetch the previous command from the history list.
                try!(edit_history_next(&mut s, history, true))
            }
            KeyPress::CTRL_T => {
                kill_ring.reset();
                // Exchange the char before cursor with the character at cursor.
                try!(edit_transpose_chars(&mut s))
            }
            KeyPress::CTRL_U => {
                // Kill backward from point to the beginning of the line.
                if let Some(text) = try!(edit_discard_line(&mut s)) {
                    kill_ring.kill(&text, false)
                }
            }
            KeyPress::CTRL_V => {
                // Quoted insert
                kill_ring.reset();
                let rk = rdr.next_key();
                let key = try!(rk);
                if let KeyPress::Char(c) = key {
                    try!(edit_insert(&mut s, c))
                }
            }
            KeyPress::CTRL_W => {
                // Kill the word behind point, using white space as a word boundary
                if let Some(text) = try!(edit_delete_prev_word(&mut s, char::is_whitespace)) {
                    kill_ring.kill(&text, false)
                }
            }
            KeyPress::CTRL_Y => {
                // retrieve (yank) last item killed
                if let Some(text) = kill_ring.yank() {
                    try!(edit_yank(&mut s, text))
                }
            }
            #[cfg(unix)]
            KeyPress::CTRL_Z => {
                try!(disable_raw_mode(original_mode));
                try!(signal::raise(signal::SIGSTOP));
                try!(enable_raw_mode()); // TODO original_mode may have changed
                try!(s.refresh_line())
            }
            // TODO CTRL-_ // undo
            KeyPress::ENTER | KeyPress::CTRL_J => {
                // Accept the line regardless of where the cursor is.
                kill_ring.reset();
                try!(edit_move_end(&mut s));
                break;
            }
            KeyPress::ESC_BACKSPACE => {
                // kill one word backward
                // Kill from the cursor to the start of the current word, or, if between words, to the start of the previous word.
                if let Some(text) = try!(edit_delete_prev_word(&mut s,
                                                               |ch| !ch.is_alphanumeric())) {
                    kill_ring.kill(&text, false)
                }
            }
            KeyPress::ESC_B => {
                // move backwards one word
                kill_ring.reset();
                try!(edit_move_to_prev_word(&mut s))
            }
            KeyPress::ESC_C => {
                // capitalize word after point
                kill_ring.reset();
                try!(edit_word(&mut s, WordAction::CAPITALIZE))
            }
            KeyPress::ESC_D => {
                // kill one word forward
                if let Some(text) = try!(edit_delete_word(&mut s)) {
                    kill_ring.kill(&text, true)
                }
            }
            KeyPress::ESC_F => {
                // move forwards one word
                kill_ring.reset();
                try!(edit_move_to_next_word(&mut s))
            }
            KeyPress::ESC_L => {
                // lowercase word after point
                kill_ring.reset();
                try!(edit_word(&mut s, WordAction::LOWERCASE))
            }
            KeyPress::ESC_T => {
                // transpose words
                kill_ring.reset();
                try!(edit_transpose_words(&mut s))
            }
            KeyPress::ESC_U => {
                // uppercase word after point
                kill_ring.reset();
                try!(edit_word(&mut s, WordAction::UPPERCASE))
            }
            KeyPress::ESC_Y => {
                // yank-pop
                if let Some((yank_size, text)) = kill_ring.yank_pop() {
                    try!(edit_yank_pop(&mut s, yank_size, text))
                }
            }
            KeyPress::ESC_SEQ_DELETE => {
                kill_ring.reset();
                try!(edit_delete(&mut s))
            }
            _ => {
                kill_ring.reset();
                // Ignore the character typed.
            }
        }
    }
    Ok(s.line.into_string())
}

struct Guard(Mode);

#[allow(unused_must_use)]
impl Drop for Guard {
    fn drop(&mut self) {
        let Guard(mode) = *self;
        disable_raw_mode(mode);
    }
}

/// Readline method that will enable RAW mode, call the `readline_edit()`
/// method and disable raw mode
fn readline_raw(prompt: &str,
                history: &mut History,
                completer: Option<&Completer>,
                kill_ring: &mut KillRing)
                -> Result<String> {
    let original_mode = try!(enable_raw_mode());
    let guard = Guard(original_mode);
    let user_input = readline_edit(prompt, history, completer, kill_ring, original_mode);
    drop(guard); // try!(disable_raw_mode(original_mode));
    println!("");
    user_input
}

fn readline_direct() -> Result<String> {
    let mut line = String::new();
    if try!(io::stdin().read_line(&mut line)) > 0 {
        Ok(line)
    } else {
        Err(error::ReadlineError::Eof)
    }
}

/// Line editor
pub struct Editor<'completer> {
    unsupported_term: bool,
    stdin_isatty: bool,
    stdout_isatty: bool,
    // cols: usize, // Number of columns in terminal
    history: History,
    completer: Option<&'completer Completer>,
    kill_ring: KillRing,
}

impl<'completer> Editor<'completer> {
    pub fn new() -> Editor<'completer> {
        // TODO check what is done in rl_initialize()
        // if the number of columns is stored here, we need a SIGWINCH handler...
        let editor = Editor {
            unsupported_term: is_unsupported_term(),
            stdin_isatty: is_a_tty(STDIN_FILENO),
            stdout_isatty: is_a_tty(STDOUT_FILENO),
            history: History::new(),
            completer: None,
            kill_ring: KillRing::new(60),
        };
        if !editor.unsupported_term && editor.stdin_isatty && editor.stdout_isatty {
            install_sigwinch_handler();
        }
        editor
    }

    /// This method will read a line from STDIN and will display a `prompt`
    #[cfg_attr(feature="clippy", allow(if_not_else))]
    pub fn readline(&mut self, prompt: &str) -> Result<String> {
        if self.unsupported_term {
            // Write prompt and flush it to stdout
            let mut stdout = io::stdout();
            try!(write_and_flush(&mut stdout, prompt.as_bytes()));

            readline_direct()
        } else if !self.stdin_isatty {
            // Not a tty: read from file / pipe.
            readline_direct()
        } else {
            readline_raw(prompt,
                         &mut self.history,
                         self.completer,
                         &mut self.kill_ring)
        }
    }

    pub fn history_ignore_space(mut self, yes: bool) -> Editor<'completer> {
        self.history.ignore_space(yes);
        self
    }

    pub fn history_ignore_dups(mut self, yes: bool) -> Editor<'completer> {
        self.history.ignore_dups(yes);
        self
    }

    /// Load the history from the specified file.
    pub fn load_history<P: AsRef<Path> + ?Sized>(&mut self, path: &P) -> Result<()> {
        self.history.load(path)
    }
    /// Save the history in the specified file.
    pub fn save_history<P: AsRef<Path> + ?Sized>(&self, path: &P) -> Result<()> {
        self.history.save(path)
    }
    /// Add a new entry in the history.
    pub fn add_history_entry(&mut self, line: &str) -> bool {
        self.history.add(line)
    }
    /// Set the maximum length for the history.
    pub fn set_history_max_len(&mut self, max_len: usize) {
        self.history.set_max_len(max_len)
    }
    /// Clear history.
    pub fn clear_history(&mut self) {
        self.history.clear()
    }
    /// Return a reference to the history object.
    pub fn get_history(&mut self) -> &mut History {
        &mut self.history
    }

    /// Register a callback function to be called for tab-completion.
    pub fn set_completer(&mut self, completer: Option<&'completer Completer>) {
        self.completer = completer;
    }
}

impl<'completer> Default for Editor<'completer> {
    fn default() -> Editor<'completer> {
        Editor::new()
    }
}

impl<'completer> fmt::Debug for Editor<'completer> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("State")
            .field("unsupported_term", &self.unsupported_term)
            .field("stdin_isatty", &self.stdin_isatty)
            .finish()
    }
}

#[cfg(unix)]
static SIGWINCH_ONCE: sync::Once = sync::ONCE_INIT;
static SIGWINCH: atomic::AtomicBool = atomic::ATOMIC_BOOL_INIT;
#[cfg(unix)]
fn install_sigwinch_handler() {
    SIGWINCH_ONCE.call_once(|| unsafe {
        let sigwinch = signal::SigAction::new(signal::SigHandler::Handler(sigwinch_handler),
                                              signal::SaFlag::empty(),
                                              signal::SigSet::empty());
        let _ = signal::sigaction(signal::SIGWINCH, &sigwinch);
    });
}
#[cfg(unix)]
extern "C" fn sigwinch_handler(_: signal::SigNum) {
    SIGWINCH.store(true, atomic::Ordering::SeqCst);
}
#[cfg(windows)]
fn install_sigwinch_handler() {
    // See ReadConsoleInputW && WINDOW_BUFFER_SIZE_EVENT
}

#[cfg(test)]
mod test {
    use std::io::Write;
    use line_buffer::LineBuffer;
    use history::History;
    use completion::Completer;
    use consts::KeyPress;
    use State;
    use super::{Handle, RawReader, Result};

    #[cfg(unix)]
    fn default_handle() -> Handle {
        ()
    }
    #[cfg(windows)]
    fn default_handle() -> Handle {
        ::std::ptr::null_mut()
        // super::get_std_handle(super::STDOUT_FILENO).expect("Valid stdout")
    }

    fn init_state<'out>(out: &'out mut Write,
                        line: &str,
                        pos: usize,
                        cols: usize)
                        -> State<'out, 'static> {
        State {
            out: out,
            prompt: "",
            prompt_size: Default::default(),
            line: LineBuffer::init(line, pos),
            cursor: Default::default(),
            cols: cols,
            old_rows: 0,
            history_index: 0,
            snapshot: LineBuffer::with_capacity(100),
            output_handle: default_handle(),
        }
    }

    #[test]
    fn edit_history_next() {
        let mut out = ::std::io::sink();
        let line = "current edited line";
        let mut s = init_state(&mut out, line, 6, 80);
        let mut history = History::new();
        history.add("line0");
        history.add("line1");
        s.history_index = history.len();

        for _ in 0..2 {
            super::edit_history_next(&mut s, &history, false).unwrap();
            assert_eq!(line, s.line.as_str());
        }

        super::edit_history_next(&mut s, &history, true).unwrap();
        assert_eq!(line, s.snapshot.as_str());
        assert_eq!(1, s.history_index);
        assert_eq!("line1", s.line.as_str());

        for _ in 0..2 {
            super::edit_history_next(&mut s, &history, true).unwrap();
            assert_eq!(line, s.snapshot.as_str());
            assert_eq!(0, s.history_index);
            assert_eq!("line0", s.line.as_str());
        }

        super::edit_history_next(&mut s, &history, false).unwrap();
        assert_eq!(line, s.snapshot.as_str());
        assert_eq!(1, s.history_index);
        assert_eq!("line1", s.line.as_str());

        super::edit_history_next(&mut s, &history, false).unwrap();
        // assert_eq!(line, s.snapshot);
        assert_eq!(2, s.history_index);
        assert_eq!(line, s.line.as_str());
    }

    struct SimpleCompleter;
    impl Completer for SimpleCompleter {
        fn complete(&self, line: &str, _pos: usize) -> Result<(usize, Vec<String>)> {
            Ok((0, vec![line.to_string() + "t"]))
        }
    }

    #[test]
    fn complete_line() {
        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "rus", 3, 80);
        let input = b"\n";
        let mut rdr = RawReader::new(&input[..]).unwrap();
        let completer = SimpleCompleter;
        let key = super::complete_line(&mut rdr, &mut s, &completer).unwrap();
        assert_eq!(Some(KeyPress::CTRL_J), key);
        assert_eq!("rust", s.line.as_str());
        assert_eq!(4, s.line.pos());
    }

    #[test]
    fn prompt_with_ansi_escape_codes() {
        let pos = super::calculate_position("\x1b[1;32m>>\x1b[0m ", Default::default(), 80);
        assert_eq!(3, pos.col);
        assert_eq!(0, pos.row);
    }
}
