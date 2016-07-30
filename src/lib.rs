//! Readline for Rust
//!
//! This implementation is based on [Antirez's Linenoise](https://github.com/antirez/linenoise)
//!
//! # Example
//!
//! Usage
//!
//! ```
//! let mut rl = rustyline::Editor::<()>::new();
//! let readline = rl.readline(">> ");
//! match readline {
//!     Ok(line) => println!("Line: {:?}",line),
//!     Err(_)   => println!("No input"),
//! }
//! ```

extern crate libc;
#[cfg(unix)]
extern crate nix;
extern crate unicode_width;
extern crate encode_unicode;
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
#[cfg(unix)]
mod char_iter;

#[macro_use]
mod tty;

use std::fmt;
use std::io::{self, Read, Write};
use std::mem;
use std::path::Path;
use std::result;
#[cfg(unix)]
use std::sync;
use std::sync::atomic;
#[cfg(unix)]
use nix::sys::signal;

use encode_unicode::CharExt;
use completion::Completer;
use consts::KeyPress;
use history::History;
use line_buffer::{LineBuffer, MAX_LINE, WordAction};
use kill_ring::KillRing;

/// The error type for I/O and Linux Syscalls (Errno)
pub type Result<T> = result::Result<T, error::ReadlineError>;

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
    output_handle: tty::Handle, // output handle (for windows)
}

#[derive(Copy, Clone, Debug, Default)]
struct Position {
    col: usize,
    row: usize,
}

impl<'out, 'prompt> State<'out, 'prompt> {
    fn new(out: &'out mut Write,
           output_handle: tty::Handle,
           prompt: &'prompt str,
           history_index: usize)
           -> State<'out, 'prompt> {
        let capacity = MAX_LINE;
        let cols = tty::get_columns(output_handle);
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
        self.cols = tty::get_columns(self.output_handle);
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

fn write_and_flush(w: &mut Write, buf: &[u8]) -> Result<()> {
    try!(w.write_all(buf));
    try!(w.flush());
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
                write_and_flush(s.out, ch.to_utf8().as_bytes())
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
    }
    s.refresh_line()
}

/// Substitute the currently edited line with the first/last history entry.
fn edit_history(s: &mut State, history: &History, first: bool) -> Result<()> {
    if history.is_empty() {
        return Ok(());
    }
    if s.history_index == history.len() {
        if first {
            // Save the current edited line before to overwrite it
            s.snapshot();
        } else {
            return Ok(());
        }
    } else if s.history_index == 0 && first {
        return Ok(());
    }
    if first {
        s.history_index = 0;
        let buf = history.get(s.history_index).unwrap();
        s.line.update(buf, buf.len());
    } else {
        s.history_index = history.len();
        // Restore current edited line
        s.snapshot();
    }
    s.refresh_line()
}

/// Completes the line/word
fn complete_line<R: Read>(rdr: &mut tty::RawReader<R>,
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

            key = try!(rdr.next_key(false));
            match key {
                KeyPress::Tab => {
                    i = (i + 1) % (candidates.len() + 1); // Circular
                    if i == candidates.len() {
                        try!(beep());
                    }
                }
                KeyPress::Esc => {
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
fn reverse_incremental_search<R: Read>(rdr: &mut tty::RawReader<R>,
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

        key = try!(rdr.next_key(true));
        if let KeyPress::Char(c) = key {
            search_buf.push(c);
        } else {
            match key {
                KeyPress::Ctrl('H') |
                KeyPress::Backspace => {
                    search_buf.pop();
                    continue;
                }
                KeyPress::Ctrl('R') => {
                    reverse = true;
                    if history_idx > 0 {
                        history_idx -= 1;
                    } else {
                        success = false;
                        continue;
                    }
                }
                KeyPress::Ctrl('S') => {
                    reverse = false;
                    if history_idx < history.len() - 1 {
                        history_idx += 1;
                    } else {
                        success = false;
                        continue;
                    }
                }
                KeyPress::Ctrl('G') => {
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

/// Handles reading and editting the readline buffer.
/// It will also handle special inputs in an appropriate fashion
/// (e.g., C-c will exit readline)
#[cfg_attr(feature="clippy", allow(cyclomatic_complexity))]
fn readline_edit(prompt: &str,
                 history: &mut History,
                 completer: Option<&Completer>,
                 kill_ring: &mut KillRing,
                 original_mode: tty::Mode)
                 -> Result<String> {
    let mut stdout = io::stdout();
    let stdout_handle = try!(tty::stdout_handle());

    kill_ring.reset();
    let mut s = State::new(&mut stdout, stdout_handle, prompt, history.len());
    try!(s.refresh_line());

    let mut rdr = try!(tty::RawReader::new(io::stdin()));

    loop {
        let rk = rdr.next_key(true);
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
        if key == KeyPress::Tab && completer.is_some() {
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
        } else if key == KeyPress::Ctrl('R') {
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
            KeyPress::Ctrl('A') |
            KeyPress::Home => {
                kill_ring.reset();
                // Move to the beginning of line.
                try!(edit_move_home(&mut s))
            }
            KeyPress::Ctrl('B') |
            KeyPress::Left => {
                kill_ring.reset();
                // Move back a character.
                try!(edit_move_left(&mut s))
            }
            KeyPress::Ctrl('C') => {
                kill_ring.reset();
                return Err(error::ReadlineError::Interrupted);
            }
            KeyPress::Ctrl('D') => {
                kill_ring.reset();
                if s.line.is_empty() {
                    return Err(error::ReadlineError::Eof);
                } else {
                    // Delete (forward) one character at point.
                    try!(edit_delete(&mut s))
                }
            }
            KeyPress::Ctrl('E') |
            KeyPress::End => {
                kill_ring.reset();
                // Move to the end of line.
                try!(edit_move_end(&mut s))
            }
            KeyPress::Ctrl('F') |
            KeyPress::Right => {
                kill_ring.reset();
                // Move forward a character.
                try!(edit_move_right(&mut s))
            }
            KeyPress::Ctrl('H') |
            KeyPress::Backspace => {
                kill_ring.reset();
                // Delete one character backward.
                try!(edit_backspace(&mut s))
            }
            KeyPress::Ctrl('K') => {
                // Kill the text from point to the end of the line.
                if let Some(text) = try!(edit_kill_line(&mut s)) {
                    kill_ring.kill(&text, true)
                }
            }
            KeyPress::Ctrl('L') => {
                // Clear the screen leaving the current line at the top of the screen.
                try!(tty::clear_screen(&mut s.out, s.output_handle));
                try!(s.refresh_line())
            }
            KeyPress::Ctrl('N') |
            KeyPress::Down => {
                kill_ring.reset();
                // Fetch the next command from the history list.
                try!(edit_history_next(&mut s, history, false))
            }
            KeyPress::Ctrl('P') |
            KeyPress::Up => {
                kill_ring.reset();
                // Fetch the previous command from the history list.
                try!(edit_history_next(&mut s, history, true))
            }
            KeyPress::Ctrl('T') => {
                kill_ring.reset();
                // Exchange the char before cursor with the character at cursor.
                try!(edit_transpose_chars(&mut s))
            }
            KeyPress::Ctrl('U') => {
                // Kill backward from point to the beginning of the line.
                if let Some(text) = try!(edit_discard_line(&mut s)) {
                    kill_ring.kill(&text, false)
                }
            }
            #[cfg(unix)]
            KeyPress::Ctrl('V') => {
                // Quoted insert
                kill_ring.reset();
                let c = try!(rdr.next_char());
                try!(edit_insert(&mut s, c)) // FIXME
            }
            KeyPress::Ctrl('W') => {
                // Kill the word behind point, using white space as a word boundary
                if let Some(text) = try!(edit_delete_prev_word(&mut s, char::is_whitespace)) {
                    kill_ring.kill(&text, false)
                }
            }
            KeyPress::Ctrl('Y') => {
                // retrieve (yank) last item killed
                if let Some(text) = kill_ring.yank() {
                    try!(edit_yank(&mut s, text))
                }
            }
            #[cfg(unix)]
            KeyPress::Ctrl('Z') => {
                try!(tty::disable_raw_mode(original_mode));
                try!(signal::raise(signal::SIGSTOP));
                try!(tty::enable_raw_mode()); // TODO original_mode may have changed
                try!(s.refresh_line())
            }
            // TODO CTRL-_ // undo
            KeyPress::Enter |
            KeyPress::Ctrl('J') => {
                // Accept the line regardless of where the cursor is.
                kill_ring.reset();
                try!(edit_move_end(&mut s));
                break;
            }
            KeyPress::Meta('\x08') |
            KeyPress::Meta('\x7f') => {
                // kill one word backward
                // Kill from the cursor to the start of the current word, or, if between words, to the start of the previous word.
                if let Some(text) = try!(edit_delete_prev_word(&mut s,
                                                               |ch| !ch.is_alphanumeric())) {
                    kill_ring.kill(&text, false)
                }
            }
            KeyPress::Meta('<') => {
                // move to first entry in history
                kill_ring.reset();
                try!(edit_history(&mut s, history, true))
            }
            KeyPress::Meta('>') => {
                // move to last entry in history
                kill_ring.reset();
                try!(edit_history(&mut s, history, false))
            }
            KeyPress::Meta('B') => {
                // move backwards one word
                kill_ring.reset();
                try!(edit_move_to_prev_word(&mut s))
            }
            KeyPress::Meta('C') => {
                // capitalize word after point
                kill_ring.reset();
                try!(edit_word(&mut s, WordAction::CAPITALIZE))
            }
            KeyPress::Meta('D') => {
                // kill one word forward
                if let Some(text) = try!(edit_delete_word(&mut s)) {
                    kill_ring.kill(&text, true)
                }
            }
            KeyPress::Meta('F') => {
                // move forwards one word
                kill_ring.reset();
                try!(edit_move_to_next_word(&mut s))
            }
            KeyPress::Meta('L') => {
                // lowercase word after point
                kill_ring.reset();
                try!(edit_word(&mut s, WordAction::LOWERCASE))
            }
            KeyPress::Meta('T') => {
                // transpose words
                kill_ring.reset();
                try!(edit_transpose_words(&mut s))
            }
            KeyPress::Meta('U') => {
                // uppercase word after point
                kill_ring.reset();
                try!(edit_word(&mut s, WordAction::UPPERCASE))
            }
            KeyPress::Meta('Y') => {
                // yank-pop
                if let Some((yank_size, text)) = kill_ring.yank_pop() {
                    try!(edit_yank_pop(&mut s, yank_size, text))
                }
            }
            KeyPress::Delete => {
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

struct Guard(tty::Mode);

#[allow(unused_must_use)]
impl Drop for Guard {
    fn drop(&mut self) {
        let Guard(mode) = *self;
        tty::disable_raw_mode(mode);
    }
}

/// Readline method that will enable RAW mode, call the `readline_edit()`
/// method and disable raw mode
fn readline_raw(prompt: &str,
                history: &mut History,
                completer: Option<&Completer>,
                kill_ring: &mut KillRing)
                -> Result<String> {
    let original_mode = try!(tty::enable_raw_mode());
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
pub struct Editor<C> {
    unsupported_term: bool,
    stdin_isatty: bool,
    stdout_isatty: bool,
    // cols: usize, // Number of columns in terminal
    history: History,
    completer: Option<C>,
    kill_ring: KillRing,
}

impl<C> Editor<C> {
    pub fn new() -> Editor<C> {
        // TODO check what is done in rl_initialize()
        // if the number of columns is stored here, we need a SIGWINCH handler...
        let editor = Editor {
            unsupported_term: tty::is_unsupported_term(),
            stdin_isatty: tty::is_a_tty(tty::STDIN_FILENO),
            stdout_isatty: tty::is_a_tty(tty::STDOUT_FILENO),
            history: History::new(),
            completer: None,
            kill_ring: KillRing::new(60),
        };
        if !editor.unsupported_term && editor.stdin_isatty && editor.stdout_isatty {
            install_sigwinch_handler();
        }
        editor
    }

    pub fn history_ignore_dups(mut self, yes: bool) -> Editor<C> {
        self.history.ignore_dups(yes);
        self
    }

    pub fn history_ignore_space(mut self, yes: bool) -> Editor<C> {
        self.history.ignore_space(yes);
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
}

impl<C: Completer> Editor<C> {
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
                         self.completer.as_ref().map(|c| c as &Completer),
                         &mut self.kill_ring)
        }
    }

    /// Register a callback function to be called for tab-completion.
    pub fn set_completer(&mut self, completer: Option<C>) {
        self.completer = completer;
    }
}

impl<C> Default for Editor<C> {
    fn default() -> Editor<C> {
        Editor::new()
    }
}

impl<C> fmt::Debug for Editor<C> {
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
    #[cfg(unix)]
    use completion::Completer;
    use State;
    #[cfg(unix)]
    use super::Result;
    use tty::Handle;

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
    #[cfg(unix)]
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

    #[cfg(unix)]
    struct SimpleCompleter;
    #[cfg(unix)]
    impl Completer for SimpleCompleter {
        fn complete(&self, line: &str, _pos: usize) -> Result<(usize, Vec<String>)> {
            Ok((0, vec![line.to_string() + "t"]))
        }
    }

    #[test]
    #[cfg(unix)]
    fn complete_line() {
        use consts::KeyPress;
        use tty::RawReader;

        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "rus", 3, 80);
        let input = b"\n";
        let mut rdr = RawReader::new(&input[..]).unwrap();
        let completer = SimpleCompleter;
        let key = super::complete_line(&mut rdr, &mut s, &completer).unwrap();
        assert_eq!(Some(KeyPress::Ctrl('J')), key);
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
