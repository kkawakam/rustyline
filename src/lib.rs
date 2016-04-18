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
#![feature(iter_arith)]
#![feature(unicode)]
#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate libc;
extern crate nix;
extern crate unicode_width;

pub mod completion;
#[allow(non_camel_case_types)]
mod consts;
pub mod error;
pub mod history;
mod kill_ring;

use std::fmt;
use std::io::{self, Read, Write};
use std::mem;
use std::path::Path;
use std::result;
use std::sync;
use std::sync::atomic;
use nix::errno::Errno;
use nix::sys::signal;
use nix::sys::termios;

use completion::Completer;
use consts::{KeyPress, char_to_key_press};
use history::History;
use kill_ring::KillRing;

/// The error type for I/O and Linux Syscalls (Errno)
pub type Result<T> = result::Result<T, error::ReadlineError>;

// Represent the state during line editing.
struct State<'out, 'prompt> {
    out: &'out mut Write,
    prompt: &'prompt str, // Prompt to display
    prompt_size: Position, // Prompt Unicode width and height
    buf: String, // Edited line buffer
    pos: usize, // Current cursor position (byte position)
    cursor: Position, // Cursor position (relative to the start of the prompt for `row`)
    cols: usize, // Number of columns in terminal
    history_index: usize, // The history index we are currently editing.
    snapshot: String, // Current edited line before history browsing/completion
    snapshot_pos: usize, // Current cursor position before history browsing/completion
}

#[derive(Copy, Clone, Debug, Default)]
struct Position {
    col: usize,
    row: usize,
}

impl<'out, 'prompt> State<'out, 'prompt> {
    fn new(out: &'out mut Write,
           prompt: &'prompt str,
           capacity: usize,
           cols: usize,
           history_index: usize)
           -> State<'out, 'prompt> {
        let prompt_size = calculate_position(prompt, Default::default(), cols);
        State {
            out: out,
            prompt: prompt,
            prompt_size: prompt_size,
            buf: String::with_capacity(capacity),
            pos: 0,
            cursor: prompt_size,
            cols: cols,
            history_index: history_index,
            snapshot: String::with_capacity(capacity),
            snapshot_pos: 0,
        }
    }

    fn update_buf(&mut self, buf: &str, pos: usize) {
        self.buf.clear();
        if buf.len() > MAX_LINE {
            self.buf.push_str(&buf[..MAX_LINE]);
            if pos > MAX_LINE {
                self.pos = MAX_LINE;
            } else {
                self.pos = pos;
            }
        } else {
            self.buf.push_str(buf);
            self.pos = pos;

        }
    }

    fn snapshot(&mut self) {
        mem::swap(&mut self.buf, &mut self.snapshot);
        mem::swap(&mut self.pos, &mut self.snapshot_pos);
    }
    fn backup(&mut self) {
        self.snapshot.clear();
        self.snapshot.push_str(&self.buf);
        self.snapshot_pos = self.pos;
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

    fn refresh(&mut self, prompt: &str, prompt_size: Position) -> Result<()> {
        use std::fmt::Write;

        let end_pos = calculate_position(&self.buf, prompt_size, self.cols);
        let cursor = calculate_position(&self.buf[..self.pos], prompt_size, self.cols);

        let mut ab = String::new();
        let cursor_row_movement = self.cursor.row - self.prompt_size.row;
        // move the cursor up as required
        if cursor_row_movement > 0 {
            write!(ab, "\x1b[{}A", cursor_row_movement).unwrap();
        }
        // position at the start of the prompt, clear to end of screen
        ab.push_str("\r\x1b[J");
        // display the prompt
        ab.push_str(prompt);
        // display the input line
        ab.push_str(&self.buf);
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

        write_and_flush(self.out, ab.as_bytes())
    }

    fn char_at_cursor(&self) -> Option<char> {
        if self.pos == self.buf.len() {
            None
        } else {
            self.buf[self.pos..].chars().next()
        }
    }
    fn char_before_cursor(&self) -> Option<char> {
        if self.pos == 0 {
            None
        } else {
            self.buf[..self.pos].chars().next_back()
        }
    }
}

impl<'out, 'prompt> fmt::Debug for State<'out, 'prompt> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("State")
         .field("prompt", &self.prompt)
         .field("prompt_size", &self.prompt_size)
         .field("buf", &self.buf)
         .field("buf length", &self.buf.len())
         .field("buf capacity", &self.buf.capacity())
         .field("pos", &self.pos)
         .field("cursor", &self.cursor)
         .field("cols", &self.cols)
         .field("history_index", &self.history_index)
         .field("snapshot", &self.snapshot)
         .finish()
    }
}

/// Maximum buffer size for the line read
static MAX_LINE: usize = 4096;

/// Unsupported Terminals that don't support RAW mode
static UNSUPPORTED_TERM: [&'static str; 3] = ["dumb", "cons25", "emacs"];

/// Check to see if `fd` is a TTY
fn is_a_tty(fd: libc::c_int) -> bool {
    unsafe { libc::isatty(fd) != 0 }
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

fn from_errno(errno: Errno) -> error::ReadlineError {
    error::ReadlineError::from(nix::Error::from_errno(errno))
}

/// Enable raw mode for the TERM
fn enable_raw_mode() -> Result<termios::Termios> {
    use nix::sys::termios::{BRKINT, ICRNL, INPCK, ISTRIP, IXON, OPOST, CS8, ECHO, ICANON, IEXTEN,
                            ISIG, VMIN, VTIME};
    if !is_a_tty(libc::STDIN_FILENO) {
        return Err(from_errno(Errno::ENOTTY));
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
fn disable_raw_mode(original_termios: termios::Termios) -> Result<()> {
    try!(termios::tcsetattr(libc::STDIN_FILENO, termios::TCSAFLUSH, &original_termios));
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
fn get_columns() -> usize {
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

fn write_and_flush(w: &mut Write, buf: &[u8]) -> Result<()> {
    try!(w.write_all(buf));
    try!(w.flush());
    Ok(())
}

/// Clear the screen. Used to handle ctrl+l
fn clear_screen(out: &mut Write) -> Result<()> {
    write_and_flush(out, b"\x1b[H\x1b[2J")
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
    if s.buf.len() < s.buf.capacity() {
        if s.pos == s.buf.len() {
            s.buf.push(ch);
            s.pos += ch.len_utf8();
            if s.cursor.col + unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) < s.cols {
                // Avoid a full update of the line in the trivial case.
                let bits = ch.encode_utf8();
                let bits = bits.as_slice();
                write_and_flush(s.out, bits)
            } else {
                s.refresh_line()
            }
        } else {
            s.buf.insert(s.pos, ch);
            s.pos += ch.len_utf8();
            s.refresh_line()
        }
    } else {
        Ok(())
    }
}

// Yank/paste `text` at current position.
fn edit_yank(s: &mut State, text: &str) -> Result<()> {
    if text.is_empty() || (s.buf.len() + text.len()) > s.buf.capacity() {
        return Ok(());
    }
    if s.pos == s.buf.len() {
        s.buf.push_str(text);
    } else {
        insert_str(&mut s.buf, s.pos, text);
    }
    s.pos += text.len();
    s.refresh_line()
}

fn insert_str(buf: &mut String, idx: usize, s: &str) {
    use std::ptr;

    let len = buf.len();
    assert!(idx <= len);
    assert!(buf.is_char_boundary(idx));
    let amt = s.len();
    buf.reserve(amt);

    unsafe {
        let v = buf.as_mut_vec();
        ptr::copy(v.as_ptr().offset(idx as isize),
                  v.as_mut_ptr().offset((idx + amt) as isize),
                  len - idx);
        ptr::copy_nonoverlapping(s.as_ptr(), v.as_mut_ptr().offset(idx as isize), amt);
        v.set_len(len + amt);
    }
}

// Delete previously yanked text and yank/paste `text` at current position.
fn edit_yank_pop(s: &mut State, yank_size: usize, text: &str) -> Result<()> {
    s.buf.drain((s.pos - yank_size)..s.pos);
    s.pos -= yank_size;
    edit_yank(s, text)
}

/// Move cursor on the left.
fn edit_move_left(s: &mut State) -> Result<()> {
    if let Some(ch) = s.char_before_cursor() {
        s.pos -= ch.len_utf8();
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Move cursor on the right.
fn edit_move_right(s: &mut State) -> Result<()> {
    if let Some(ch) = s.char_at_cursor() {
        s.pos += ch.len_utf8();
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Move cursor to the start of the line.
fn edit_move_home(s: &mut State) -> Result<()> {
    if s.pos > 0 {
        s.pos = 0;
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Move cursor to the end of the line.
fn edit_move_end(s: &mut State) -> Result<()> {
    if s.pos == s.buf.len() {
        return Ok(());
    }
    s.pos = s.buf.len();
    s.refresh_line()
}

/// Delete the character at the right of the cursor without altering the cursor
/// position. Basically this is what happens with the "Delete" keyboard key.
fn edit_delete(s: &mut State) -> Result<()> {
    if !s.buf.is_empty() && s.pos < s.buf.len() {
        s.buf.remove(s.pos);
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Backspace implementation.
fn edit_backspace(s: &mut State) -> Result<()> {
    if let Some(ch) = s.char_before_cursor() {
        s.pos -= ch.len_utf8();
        s.buf.remove(s.pos);
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Kill the text from point to the end of the line.
fn edit_kill_line(s: &mut State) -> Result<Option<String>> {
    if !s.buf.is_empty() && s.pos < s.buf.len() {
        let text = s.buf.drain(s.pos..).collect();
        try!(s.refresh_line());
        Ok(Some(text))
    } else {
        Ok(None)
    }
}

/// Kill backward from point to the beginning of the line.
fn edit_discard_line(s: &mut State) -> Result<Option<String>> {
    if s.pos > 0 && !s.buf.is_empty() {
        let text = s.buf.drain(..s.pos).collect();
        s.pos = 0;
        try!(s.refresh_line());
        Ok(Some(text))
    } else {
        Ok(None)
    }
}

/// Exchange the char before cursor with the character at cursor.
fn edit_transpose_chars(s: &mut State) -> Result<()> {
    if s.pos > 0 && s.pos < s.buf.len() {
        // TODO should work even if s.pos == s.buf.len()
        let ch = s.buf.remove(s.pos);
        let size = ch.len_utf8();
        let other_ch = s.char_before_cursor().unwrap();
        let other_size = other_ch.len_utf8();
        s.buf.insert(s.pos - other_size, ch);
        if s.pos != s.buf.len() - size {
            s.pos += size;
        } else if size >= other_size {
            s.pos += size - other_size;
        } else {
            s.pos -= other_size - size;
        }
        s.refresh_line()
    } else {
        Ok(())
    }
}

fn prev_word_pos<F>(s: &State, test: F) -> Option<usize>
    where F: Fn(char) -> bool
{
    if s.pos > 0 {
        let mut pos = s.pos;
        // eat any spaces on the left
        pos -= s.buf[..pos]
                   .chars()
                   .rev()
                   .take_while(|ch| test(*ch))
                   .map(char::len_utf8)
                   .sum();
        if pos > 0 {
            // eat any non-spaces on the left
            pos -= s.buf[..pos]
                       .chars()
                       .rev()
                       .take_while(|ch| !test(*ch))
                       .map(char::len_utf8)
                       .sum();
        }
        Some(pos)
    } else {
        None
    }
}

fn edit_move_to_prev_word(s: &mut State) -> Result<()> {
    if let Some(pos) = prev_word_pos(s, |ch| !ch.is_alphanumeric()) {
        s.pos = pos;
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
    if let Some(pos) = prev_word_pos(s, test) {
        let text = s.buf.drain(pos..s.pos).collect();
        s.pos = pos;
        try!(s.refresh_line());
        Ok(Some(text))
    } else {
        Ok(None)
    }
}

fn next_word_pos(s: &State) -> Option<(usize, usize)> {
    if s.pos < s.buf.len() {
        let mut pos = s.pos;
        // eat any spaces
        pos += s.buf[pos..]
                   .chars()
                   .take_while(|ch| !ch.is_alphanumeric())
                   .map(char::len_utf8)
                   .sum();
        let start = pos;
        if pos < s.buf.len() {
            // eat any non-spaces
            pos += s.buf[pos..]
                       .chars()
                       .take_while(|ch| ch.is_alphanumeric())
                       .map(char::len_utf8)
                       .sum();
        }
        Some((start, pos))
    } else {
        None
    }
}

fn edit_move_to_next_word(s: &mut State) -> Result<()> {
    if let Some((_, end)) = next_word_pos(s) {
        s.pos = end;
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Kill from the cursor to the end of the current word, or, if between words, to the end of the next word.
fn edit_delete_word(s: &mut State) -> Result<Option<String>> {
    if let Some((_, end)) = next_word_pos(s) {
        let text = s.buf.drain(s.pos..end).collect();
        try!(s.refresh_line());
        Ok(Some(text))
    } else {
        Ok(None)
    }
}

enum WordAction {
    CAPITALIZE,
    LOWERCASE,
    UPPERCASE,
}

fn edit_word(s: &mut State, a: WordAction) -> Result<()> {
    if let Some((start, end)) = next_word_pos(s) {
        if start == end {
            return Ok(());
        }
        s.backup();
        s.buf.clear();
        s.buf.push_str(&s.snapshot[..start]);
        match a {
            WordAction::CAPITALIZE => {
                if let Some(ch) = s.snapshot[start..end].chars().next() {
                    let cap = ch.to_uppercase().collect::<String>();
                    s.buf.push_str(&cap);
                    s.buf.push_str(&s.snapshot[start + ch.len_utf8()..end]);
                } else {
                    s.buf.push_str(&s.snapshot[start..end]);
                }
            }
            WordAction::LOWERCASE => s.buf.push_str(&s.snapshot[start..end].to_lowercase()),
            WordAction::UPPERCASE => s.buf.push_str(&s.snapshot[start..end].to_uppercase()),
        }
        s.buf.push_str(&s.snapshot[end..]);
        s.pos = end;
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
        s.update_buf(buf, buf.len());
    } else {
        // Restore current edited line
        s.snapshot();
    };
    s.refresh_line()
}

/// Completes the line/word
fn complete_line<R: io::Read>(chars: &mut io::Chars<R>,
                              s: &mut State,
                              completer: &Completer)
                              -> Result<Option<char>> {
    let (start, candidates) = try!(completer.complete(&s.buf, s.pos));
    if candidates.is_empty() {
        try!(beep());
        Ok(None)
    } else {
        let mut ch;
        let mut i = 0;
        loop {
            // Show completion or original buffer
            if i < candidates.len() {
                // Save the current edited line before to overwrite it
                s.backup();
                let (tmp_buf, tmp_pos) = completer.update(&s.buf, s.pos, start, &candidates[i]);
                s.update_buf(&tmp_buf, tmp_pos);
                try!(s.refresh_line());
                // Restore current edited line (but no refresh)
                s.snapshot();
            } else {
                try!(s.refresh_line());
            }

            ch = try!(chars.next().unwrap());
            let key = char_to_key_press(ch);
            match key {
                KeyPress::TAB => {
                    i = (i + 1) % (candidates.len() + 1); // Circular
                    if i == candidates.len() {
                        try!(beep());
                    }
                }
                KeyPress::ESC => {
                    // Re-show original buffer
                    if i < candidates.len() {
                        try!(s.refresh_line());
                    }
                    return Ok(None);
                }
                _ => {
                    // Update buffer and return
                    if i < candidates.len() {
                        let (buf, pos) = completer.update(&s.buf, s.pos, start, &candidates[i]);
                        s.update_buf(&buf, pos);
                    }
                    break;
                }
            }
        }
        Ok(Some(ch))
    }
}

/// Incremental search
#[cfg_attr(feature="clippy", allow(if_not_else))]
fn reverse_incremental_search<R: io::Read>(chars: &mut io::Chars<R>,
                                           s: &mut State,
                                           history: &History)
                                           -> Result<Option<KeyPress>> {
    // Save the current edited line (and cursor position) before to overwrite it
    s.snapshot();

    let mut search_buf = String::new();
    let mut history_idx = history.len() - 1;
    let mut success = true;

    let mut ch;
    let mut key;
    // Display the reverse-i-search prompt and process chars
    loop {
        let prompt = if success {
            format!("(reverse-i-search)`{}': ", search_buf)
        } else {
            format!("(failed reverse-i-search)`{}': ", search_buf)
        };
        try!(s.refresh_prompt_and_line(&prompt));

        ch = try!(chars.next().unwrap());
        if !ch.is_control() {
            search_buf.push(ch);
        } else {
            key = char_to_key_press(ch);
            if key == KeyPress::ESC {
                key = try!(escape_sequence(chars));
            }
            match key {
                KeyPress::CTRL_H | KeyPress::BACKSPACE => {
                    search_buf.pop();
                    continue;
                }
                KeyPress::CTRL_R => {
                    if history_idx > 0 {
                        history_idx -= 1;
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
        success = match history.search(&search_buf, history_idx, true) {
            Some(idx) => {
                history_idx = idx;
                let entry = history.get(idx).unwrap();
                let pos = entry.find(&search_buf).unwrap();
                s.update_buf(entry, pos);
                true
            }
            _ => false,
        };
    }
    Ok(Some(key))
}

fn escape_sequence<R: io::Read>(chars: &mut io::Chars<R>) -> Result<KeyPress> {
    // Read the next two bytes representing the escape sequence.
    let seq1 = try!(chars.next().unwrap());
    if seq1 == '[' {
        // ESC [ sequences.
        let seq2 = try!(chars.next().unwrap());
        if seq2.is_digit(10) {
            // Extended escape, read additional byte.
            let seq3 = try!(chars.next().unwrap());
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
        let seq2 = try!(chars.next().unwrap());
        match seq2 {
            'F' => Ok(KeyPress::CTRL_E),
            'H' => Ok(KeyPress::CTRL_A),
            _ => Ok(KeyPress::UNKNOWN_ESC_SEQ),
        }
    } else {
        // TODO ESC-N (n): search history forward not interactively
        // TODO ESC-P (p): search history backward not interactively
        // TODO ESC-R (r): Undo all changes made to this line.
        // TODO EST-T (t): transpose words
        // TODO ESC-<: move to first entry in history
        // TODO ESC->: move to last entry in history
        match seq1 {
            'b' | 'B' => Ok(KeyPress::ESC_B),
            'c' | 'C' => Ok(KeyPress::ESC_C),
            'd' | 'D' => Ok(KeyPress::ESC_D),
            'f' | 'F' => Ok(KeyPress::ESC_F),
            'l' | 'L' => Ok(KeyPress::ESC_L),
            'u' | 'U' => Ok(KeyPress::ESC_U),
            'y' | 'Y' => Ok(KeyPress::ESC_Y),
            '\x08' | '\x7f' => Ok(KeyPress::ESC_BACKSPACE),
            _ => {
                writeln!(io::stderr(), "key: {:?}, seq1, {:?}", KeyPress::ESC, seq1).unwrap();
                Ok(KeyPress::UNKNOWN_ESC_SEQ)
            }
        }
    }
}

/// Handles reading and editting the readline buffer.
/// It will also handle special inputs in an appropriate fashion
/// (e.g., C-c will exit readline)
#[cfg_attr(feature="clippy", allow(cyclomatic_complexity))]
fn readline_edit(prompt: &str,
                 history: &mut History,
                 completer: Option<&Completer>,
                 kill_ring: &mut KillRing,
                 original_termios: termios::Termios)
                 -> Result<String> {
    let mut stdout = io::stdout();
    try!(write_and_flush(&mut stdout, prompt.as_bytes()));

    kill_ring.reset();
    let mut s = State::new(&mut stdout, prompt, MAX_LINE, get_columns(), history.len());
    let stdin = io::stdin();
    let mut chars = stdin.lock().chars();
    loop {
        let c = chars.next().unwrap();
        if c.is_err() && SIGWINCH.compare_and_swap(true, false, atomic::Ordering::SeqCst) {
            s.cols = get_columns();
            try!(s.refresh_line());
            continue;
        }
        let mut ch = try!(c);
        if !ch.is_control() {
            kill_ring.reset();
            try!(edit_insert(&mut s, ch));
            continue;
        }

        let mut key = char_to_key_press(ch);
        // autocomplete
        if key == KeyPress::TAB && completer.is_some() {
            let next = try!(complete_line(&mut chars, &mut s, completer.unwrap()));
            if next.is_some() {
                kill_ring.reset();
                ch = next.unwrap();
                if !ch.is_control() {
                    try!(edit_insert(&mut s, ch));
                    continue;
                }
                key = char_to_key_press(ch);
            } else {
                continue;
            }
        } else if key == KeyPress::CTRL_R {
            // Search history backward
            let next = try!(reverse_incremental_search(&mut chars, &mut s, history));
            if next.is_some() {
                key = next.unwrap();
            } else {
                continue;
            }
        } else if key == KeyPress::ESC {
            // escape sequence
            key = try!(escape_sequence(&mut chars));
            if key == KeyPress::UNKNOWN_ESC_SEQ {
                continue;
            }
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
                if s.buf.is_empty() {
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
                try!(clear_screen(s.out));
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
            // TODO CTRL_V // Quoted insert
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
            KeyPress::CTRL_Z => {
                try!(disable_raw_mode(original_termios));
                try!(signal::raise(signal::SIGSTOP));
                try!(enable_raw_mode()); // TODO original_termios may have changed
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
                // Kill from the cursor the start of the current word, or, if between words, to the start of the previous word.
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
                // Insert the character typed.
                try!(edit_insert(&mut s, ch))
            }
        }
    }
    Ok(s.buf)
}

struct Guard(termios::Termios);

#[allow(unused_must_use)]
impl Drop for Guard {
    fn drop(&mut self) {
        let Guard(termios) = *self;
        disable_raw_mode(termios);
    }
}

/// Readline method that will enable RAW mode, call the `readline_edit()`
/// method and disable raw mode
fn readline_raw(prompt: &str,
                history: &mut History,
                completer: Option<&Completer>,
                kill_ring: &mut KillRing)
                -> Result<String> {
    let original_termios = try!(enable_raw_mode());
    let guard = Guard(original_termios);
    let user_input = readline_edit(prompt, history, completer, kill_ring, original_termios);
    drop(guard); // try!(disable_raw_mode(original_termios));
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
            stdin_isatty: is_a_tty(libc::STDIN_FILENO),
            stdout_isatty: is_a_tty(libc::STDOUT_FILENO),
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

#[cfg(test)]
mod test {
    use std::io::Write;
    use history::History;
    use completion::Completer;
    use State;
    use super::{Result, WordAction};

    fn init_state<'out>(out: &'out mut Write,
                        line: &str,
                        pos: usize,
                        cols: usize)
                        -> State<'out, 'static> {
        State {
            out: out,
            prompt: "",
            prompt_size: Default::default(),
            buf: String::from(line),
            pos: pos,
            cursor: Default::default(),
            cols: cols,
            history_index: 0,
            snapshot: String::new(),
            snapshot_pos: 0,
        }
    }

    #[test]
    fn insert() {
        let mut out = ::std::io::sink();
        let mut s = State::new(&mut out, "", 128, 80, 0);
        super::edit_insert(&mut s, 'α').unwrap();
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);

        super::edit_insert(&mut s, 'ß').unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);

        s.pos = 0;
        super::edit_insert(&mut s, 'γ').unwrap();
        assert_eq!("γαß", s.buf);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn moves() {
        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "αß", 4, 80);
        super::edit_move_left(&mut s).unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(2, s.pos);

        super::edit_move_right(&mut s).unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);

        super::edit_move_home(&mut s).unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(0, s.pos);

        super::edit_move_end(&mut s).unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);
    }

    #[test]
    fn delete() {
        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "αß", 2, 80);
        super::edit_delete(&mut s).unwrap();
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);

        super::edit_backspace(&mut s).unwrap();
        assert_eq!("", s.buf);
        assert_eq!(0, s.pos);
    }

    #[test]
    fn kill() {
        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "αßγδε", 6, 80);
        let text = super::edit_kill_line(&mut s).unwrap();
        assert_eq!("αßγ", s.buf);
        assert_eq!(6, s.pos);
        assert_eq!(Some("δε".to_string()), text);

        s.pos = 4;
        let text = super::edit_discard_line(&mut s).unwrap();
        assert_eq!("γ", s.buf);
        assert_eq!(0, s.pos);
        assert_eq!(Some("αß".to_string()), text);
    }

    #[test]
    fn transpose() {
        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "aßc", 1, 80);
        super::edit_transpose_chars(&mut s).unwrap();
        assert_eq!("ßac", s.buf);
        assert_eq!(3, s.pos);

        s.buf = String::from("aßc");
        s.pos = 3;
        super::edit_transpose_chars(&mut s).unwrap();
        assert_eq!("acß", s.buf);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn move_to_prev_word() {
        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "a ß  c", 6, 80);
        super::edit_move_to_prev_word(&mut s).unwrap();
        assert_eq!("a ß  c", s.buf);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn delete_prev_word() {
        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "a ß  c", 6, 80);
        let text = super::edit_delete_prev_word(&mut s, char::is_whitespace).unwrap();
        assert_eq!("a c", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(Some("ß  ".to_string()), text);
    }

    #[test]
    fn move_to_next_word() {
        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "a ß  c", 1, 80);
        super::edit_move_to_next_word(&mut s).unwrap();
        assert_eq!("a ß  c", s.buf);
        assert_eq!(4, s.pos);
    }

    #[test]
    fn delete_word() {
        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "a ß  c", 1, 80);
        let text = super::edit_delete_word(&mut s).unwrap();
        assert_eq!("a  c", s.buf);
        assert_eq!(1, s.pos);
        assert_eq!(Some(" ß".to_string()), text);
    }

    #[test]
    fn edit_word() {
        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "a ßeta  c", 1, 80);
        super::edit_word(&mut s, WordAction::UPPERCASE).unwrap();
        assert_eq!("a SSETA  c", s.buf);
        assert_eq!(7, s.pos);

        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "a ßetA  c", 1, 80);
        super::edit_word(&mut s, WordAction::LOWERCASE).unwrap();
        assert_eq!("a ßeta  c", s.buf);
        assert_eq!(7, s.pos);

        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "a ßeta  c", 1, 80);
        super::edit_word(&mut s, WordAction::CAPITALIZE).unwrap();
        assert_eq!("a SSeta  c", s.buf);
        assert_eq!(7, s.pos);
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
        s.buf = String::from(line);

        for _ in 0..2 {
            super::edit_history_next(&mut s, &history, false).unwrap();
            assert_eq!(line, s.buf);
        }

        super::edit_history_next(&mut s, &history, true).unwrap();
        assert_eq!(line, s.snapshot);
        assert_eq!(1, s.history_index);
        assert_eq!("line1", s.buf);

        for _ in 0..2 {
            super::edit_history_next(&mut s, &history, true).unwrap();
            assert_eq!(line, s.snapshot);
            assert_eq!(0, s.history_index);
            assert_eq!("line0", s.buf);
        }

        super::edit_history_next(&mut s, &history, false).unwrap();
        assert_eq!(line, s.snapshot);
        assert_eq!(1, s.history_index);
        assert_eq!("line1", s.buf);

        super::edit_history_next(&mut s, &history, false).unwrap();
        // assert_eq!(line, s.snapshot);
        assert_eq!(2, s.history_index);
        assert_eq!(line, s.buf);
    }

    struct SimpleCompleter;
    impl Completer for SimpleCompleter {
        fn complete(&self, line: &str, _pos: usize) -> Result<(usize, Vec<String>)> {
            Ok((0, vec![line.to_string() + "t"]))
        }
    }

    #[test]
    fn complete_line() {
        use std::io::Read;

        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "rus", 3, 80);
        let input = b"\n";
        let mut chars = input.chars();
        let completer = SimpleCompleter;
        let ch = super::complete_line(&mut chars, &mut s, &completer).unwrap();
        assert_eq!(Some('\n'), ch);
        assert_eq!("rust", s.buf);
        assert_eq!(4, s.pos);
    }

    #[test]
    fn prompt_with_ansi_escape_codes() {
        let pos = super::calculate_position("\x1b[1;32m>>\x1b[0m ", Default::default(), 80);
        assert_eq!(3, pos.col);
        assert_eq!(0, pos.row);
    }
}
