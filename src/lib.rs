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
#![allow(unknown_lints)]

extern crate libc;
extern crate encode_unicode;
extern crate unicode_width;
#[cfg(unix)]
extern crate nix;
#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate kernel32;

pub mod completion;
mod consts;
pub mod error;
pub mod history;
mod keymap;
mod kill_ring;
pub mod line_buffer;
#[cfg(unix)]
mod char_iter;
pub mod config;

mod tty;

use std::fmt;
use std::io::{self, Write};
use std::mem;
use std::path::Path;
use std::result;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use tty::{RawMode, RawReader, Terminal, Term};

use encode_unicode::CharExt;
use completion::{Completer, longest_common_prefix};
use history::{Direction, History};
use line_buffer::{LineBuffer, MAX_LINE, WordAction};
use keymap::{Anchor, At, CharSearch, Cmd, EditState, Word};
use kill_ring::{Mode, KillRing};
pub use config::{CompletionType, Config, EditMode, HistoryDuplicates};

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
    term: Terminal, // terminal
    edit_state: EditState,
}

#[derive(Copy, Clone, Debug, Default)]
struct Position {
    col: usize,
    row: usize,
}

impl<'out, 'prompt> State<'out, 'prompt> {
    fn new(out: &'out mut Write,
           term: Terminal,
           config: &Config,
           prompt: &'prompt str,
           history_index: usize)
           -> State<'out, 'prompt> {
        let capacity = MAX_LINE;
        let cols = term.get_columns();
        let prompt_size = calculate_position(prompt, Position::default(), cols);
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
            term: term,
            edit_state: EditState::new(config),
        }
    }

    fn next_cmd<R: RawReader>(&mut self, rdr: &mut R, config: &Config) -> Result<Cmd> {
        loop {
            let rc = self.edit_state.next_cmd(rdr, config);
            if rc.is_err() && self.term.sigwinch() {
                self.update_columns();
                try!(self.refresh_line());
                continue;
            }
            return rc;
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
        let prompt_size = calculate_position(prompt, Position::default(), self.cols);
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
        // calculate the position of the end of the input line
        let end_pos = calculate_position(&self.line, prompt_size, self.cols);
        // calculate the desired position of the cursor
        let cursor = calculate_position(&self.line[..self.line.pos()], prompt_size, self.cols);

        // position at the start of the prompt, clear to end of previous input
        let mut info = try!(self.term.get_console_screen_buffer_info());
        info.dwCursorPosition.X = 0;
        info.dwCursorPosition.Y -= self.cursor.row as i16;
        try!(self.term.set_console_cursor_position(info.dwCursorPosition));
        let mut _count = 0;
        try!(self.term
            .fill_console_output_character((info.dwSize.X * (self.old_rows as i16 + 1)) as u32,
                                           info.dwCursorPosition));
        let mut ab = String::new();
        // display the prompt
        ab.push_str(prompt); // TODO handle ansi escape code (SetConsoleTextAttribute)
        // display the input line
        ab.push_str(&self.line);
        try!(write_and_flush(self.out, ab.as_bytes()));

        // position the cursor
        let mut info = try!(self.term.get_console_screen_buffer_info());
        info.dwCursorPosition.X = cursor.col as i16;
        info.dwCursorPosition.Y -= (end_pos.row - cursor.row) as i16;
        try!(self.term.set_console_cursor_position(info.dwCursorPosition));

        self.cursor = cursor;
        self.old_rows = end_pos.row;

        Ok(())
    }

    fn update_columns(&mut self) {
        self.cols = self.term.get_columns();
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
#[allow(if_same_then_else)]
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
            c.width()
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
fn edit_insert(s: &mut State, ch: char, n: usize) -> Result<()> {
    if let Some(push) = s.line.insert(ch, n) {
        if push {
            if n == 1 && s.cursor.col + ch.width().unwrap_or(0) < s.cols {
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

/// Replace a single (or n) character(s) under the cursor (Vi mode)
fn edit_replace_char(s: &mut State, ch: char, n: usize) -> Result<()> {
    if let Some(chars) = s.line.delete(n) {
        let count = chars.graphemes(true).count();
        s.line.insert(ch, count);
        s.line.move_left(1);
        s.refresh_line()
    } else {
        Ok(())
    }
}

// Yank/paste `text` at current position.
fn edit_yank(s: &mut State, text: &str, anchor: Anchor, n: usize) -> Result<()> {
    if s.line.yank(text, anchor, n).is_some() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

// Delete previously yanked text and yank/paste `text` at current position.
fn edit_yank_pop(s: &mut State, yank_size: usize, text: &str) -> Result<()> {
    s.line.yank_pop(yank_size, text);
    edit_yank(s, text, Anchor::Before, 1)
}

/// Move cursor on the left.
fn edit_move_left(s: &mut State, n: usize) -> Result<()> {
    if s.line.move_left(n) {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Move cursor on the right.
fn edit_move_right(s: &mut State, n: usize) -> Result<()> {
    if s.line.move_right(n) {
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
fn edit_delete(s: &mut State, n: usize) -> Result<()> {
    if s.line.delete(n).is_some() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Backspace implementation.
fn edit_backspace(s: &mut State, n: usize) -> Result<()> {
    if s.line.backspace(n).is_some() {
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

fn edit_move_to_prev_word(s: &mut State, word_def: Word, n: usize) -> Result<()> {
    if s.line.move_to_prev_word(word_def, n) {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Delete the previous word, maintaining the cursor at the start of the
/// current word.
fn edit_delete_prev_word(s: &mut State, word_def: Word, n: usize) -> Result<Option<String>> {
    if let Some(text) = s.line.delete_prev_word(word_def, n) {
        try!(s.refresh_line());
        Ok(Some(text))
    } else {
        Ok(None)
    }
}

fn edit_move_to_next_word(s: &mut State, at: At, word_def: Word, n: usize) -> Result<()> {
    if s.line.move_to_next_word(at, word_def, n) {
        s.refresh_line()
    } else {
        Ok(())
    }
}

fn edit_move_to(s: &mut State, cs: CharSearch, n: usize) -> Result<()> {
    if s.line.move_to(cs, n) {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Kill from the cursor to the end of the current word, or, if between words, to the end of the next word.
fn edit_delete_word(s: &mut State, at: At, word_def: Word, n: usize) -> Result<Option<String>> {
    if let Some(text) = s.line.delete_word(at, word_def, n) {
        try!(s.refresh_line());
        Ok(Some(text))
    } else {
        Ok(None)
    }
}

fn edit_delete_to(s: &mut State, cs: CharSearch, n: usize) -> Result<Option<String>> {
    if let Some(text) = s.line.delete_to(cs, n) {
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
fn complete_line<R: RawReader>(rdr: &mut R,
                               s: &mut State,
                               completer: &Completer,
                               config: &Config)
                               -> Result<Option<Cmd>> {
    // get a list of completions
    let (start, candidates) = try!(completer.complete(&s.line, s.line.pos()));
    // if no completions, we are done
    if candidates.is_empty() {
        try!(beep());
        Ok(None)
    } else if CompletionType::Circular == config.completion_type() {
        // Save the current edited line before to overwrite it
        s.backup();
        let mut cmd;
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

            cmd = try!(s.next_cmd(rdr, config));
            match cmd {
                Cmd::Complete => {
                    i = (i + 1) % (candidates.len() + 1); // Circular
                    if i == candidates.len() {
                        try!(beep());
                    }
                }
                Cmd::Abort => {
                    // Re-show original buffer
                    s.snapshot();
                    if i < candidates.len() {
                        try!(s.refresh_line());
                    }
                    return Ok(None);
                }
                _ => {
                    if i == candidates.len() {
                        s.snapshot();
                    }
                    break;
                }
            }
        }
        Ok(Some(cmd))
    } else if CompletionType::List == config.completion_type() {
        // beep if ambiguous
        if candidates.len() > 1 {
            try!(beep());
        }
        if let Some(lcp) = longest_common_prefix(&candidates) {
            // if we can extend the item, extend it and return to main loop
            if lcp.len() > s.line.pos() - start {
                completer.update(&mut s.line, start, lcp);
                try!(s.refresh_line());
                return Ok(None);
            }
        }
        // we can't complete any further, wait for second tab
        let mut cmd = try!(s.next_cmd(rdr, config));
        // if any character other than tab, pass it to the main loop
        if cmd != Cmd::Complete {
            return Ok(Some(cmd));
        }
        // move cursor to EOL to avoid overwriting the command line
        let save_pos = s.line.pos();
        try!(edit_move_end(s));
        s.line.set_pos(save_pos);
        // we got a second tab, maybe show list of possible completions
        let mut show_completions = true;
        if candidates.len() > config.completion_prompt_limit() {
            let msg = format!("\nDisplay all {} possibilities? (y or n)", candidates.len());
            try!(write_and_flush(s.out, msg.as_bytes()));
            s.old_rows += 1;
            while cmd != Cmd::SelfInsert(1, 'y') && cmd != Cmd::SelfInsert(1, 'Y') &&
                  cmd != Cmd::SelfInsert(1, 'n') &&
                  cmd != Cmd::SelfInsert(1, 'N') &&
                  cmd != Cmd::BackwardDeleteChar(1) {
                cmd = try!(s.next_cmd(rdr, config));
            }
            show_completions = match cmd {
                Cmd::SelfInsert(1, 'y') |
                Cmd::SelfInsert(1, 'Y') => true,
                _ => false,
            };
        }
        if show_completions {
            page_completions(rdr, s, config, &candidates)
        } else {
            try!(s.refresh_line());
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn page_completions<R: RawReader>(rdr: &mut R,
                                  s: &mut State,
                                  config: &Config,
                                  candidates: &[String])
                                  -> Result<Option<Cmd>> {
    use std::cmp;

    let min_col_pad = 2;
    let max_width = cmp::min(s.cols,
                             candidates.into_iter()
                                 .map(|s| s.as_str().width())
                                 .max()
                                 .unwrap() + min_col_pad);
    let num_cols = s.cols / max_width;

    let mut pause_row = s.term.get_rows() - 1;
    let num_rows = (candidates.len() + num_cols - 1) / num_cols;
    let mut ab = String::new();
    for row in 0..num_rows {
        if row == pause_row {
            try!(write_and_flush(s.out, b"\n--More--"));
            let mut cmd = Cmd::Noop;
            while cmd != Cmd::SelfInsert(1, 'y') && cmd != Cmd::SelfInsert(1_, 'Y') &&
                  cmd != Cmd::SelfInsert(1, 'n') &&
                  cmd != Cmd::SelfInsert(1_, 'N') &&
                  cmd != Cmd::SelfInsert(1, 'q') &&
                  cmd != Cmd::SelfInsert(1, 'Q') &&
                  cmd != Cmd::SelfInsert(1, ' ') &&
                  cmd != Cmd::BackwardDeleteChar(1) && cmd != Cmd::AcceptLine {
                cmd = try!(s.next_cmd(rdr, config));
            }
            match cmd {
                Cmd::SelfInsert(1, 'y') |
                Cmd::SelfInsert(1, 'Y') |
                Cmd::SelfInsert(1, ' ') => {
                    pause_row += s.term.get_rows() - 1;
                }
                Cmd::AcceptLine => {
                    pause_row += 1;
                }
                _ => break,
            }
            try!(write_and_flush(s.out, b"\n"));
        } else {
            try!(write_and_flush(s.out, b"\n"));
        }
        ab.clear();
        for col in 0..num_cols {
            let i = (col * num_rows) + row;
            if i < candidates.len() {
                let candidate = &candidates[i];
                ab.push_str(candidate);
                let width = candidate.as_str().width();
                if ((col + 1) * num_rows) + row < candidates.len() {
                    for _ in width..max_width {
                        ab.push(' ');
                    }
                }
            }
        }
        try!(write_and_flush(s.out, ab.as_bytes()));
    }
    try!(write_and_flush(s.out, b"\n"));
    try!(s.refresh_line());
    Ok(None)
}

/// Incremental search
fn reverse_incremental_search<R: RawReader>(rdr: &mut R,
                                            s: &mut State,
                                            history: &History,
                                            config: &Config)
                                            -> Result<Option<Cmd>> {
    if history.is_empty() {
        return Ok(None);
    }
    // Save the current edited line (and cursor position) before to overwrite it
    s.snapshot();

    let mut search_buf = String::new();
    let mut history_idx = history.len() - 1;
    let mut direction = Direction::Reverse;
    let mut success = true;

    let mut cmd;
    // Display the reverse-i-search prompt and process chars
    loop {
        let prompt = if success {
            format!("(reverse-i-search)`{}': ", search_buf)
        } else {
            format!("(failed reverse-i-search)`{}': ", search_buf)
        };
        try!(s.refresh_prompt_and_line(&prompt));

        cmd = try!(s.next_cmd(rdr, config));
        if let Cmd::SelfInsert(_, c) = cmd {
            search_buf.push(c);
        } else {
            match cmd {
                Cmd::BackwardDeleteChar(_) => {
                    search_buf.pop();
                    continue;
                }
                Cmd::ReverseSearchHistory => {
                    direction = Direction::Reverse;
                    if history_idx > 0 {
                        history_idx -= 1;
                    } else {
                        success = false;
                        continue;
                    }
                }
                Cmd::ForwardSearchHistory => {
                    direction = Direction::Forward;
                    if history_idx < history.len() - 1 {
                        history_idx += 1;
                    } else {
                        success = false;
                        continue;
                    }
                }
                Cmd::Abort => {
                    // Restore current edited line (before search)
                    s.snapshot();
                    try!(s.refresh_line());
                    return Ok(None);
                }
                _ => break,
            }
        }
        success = match history.search(&search_buf, history_idx, direction) {
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
    Ok(Some(cmd))
}

/// Handles reading and editting the readline buffer.
/// It will also handle special inputs in an appropriate fashion
/// (e.g., C-c will exit readline)
#[allow(let_unit_value)]
fn readline_edit<C: Completer>(prompt: &str,
                               editor: &mut Editor<C>,
                               original_mode: tty::Mode)
                               -> Result<String> {
    let completer = editor.completer.as_ref().map(|c| c as &Completer);

    let mut stdout = editor.term.create_writer();

    editor.kill_ring.reset();
    let mut s = State::new(&mut stdout,
                           editor.term.clone(),
                           &editor.config,
                           prompt,
                           editor.history.len());
    try!(s.refresh_line());

    let mut rdr = try!(s.term.create_reader());

    loop {
        let rc = s.next_cmd(&mut rdr, &editor.config);
        let mut cmd = try!(rc);

        // autocomplete
        if cmd == Cmd::Complete && completer.is_some() {
            let next = try!(complete_line(&mut rdr, &mut s, completer.unwrap(), &editor.config));
            if next.is_some() {
                editor.kill_ring.reset();
                cmd = next.unwrap();
            } else {
                continue;
            }
        }

        if let Cmd::SelfInsert(n, c) = cmd {
            editor.kill_ring.reset();
            try!(edit_insert(&mut s, c, n));
            continue;
        }

        if cmd == Cmd::ReverseSearchHistory {
            // Search history backward
            let next =
                try!(reverse_incremental_search(&mut rdr, &mut s, &editor.history, &editor.config));
            if next.is_some() {
                cmd = next.unwrap();
            } else {
                continue;
            }
        }

        match cmd {
            Cmd::BeginningOfLine => {
                editor.kill_ring.reset();
                // Move to the beginning of line.
                try!(edit_move_home(&mut s))
            }
            Cmd::BackwardChar(n) => {
                editor.kill_ring.reset();
                // Move back a character.
                try!(edit_move_left(&mut s, n))
            }
            Cmd::DeleteChar(n) => {
                editor.kill_ring.reset();
                // Delete (forward) one character at point.
                try!(edit_delete(&mut s, n))
            }
            Cmd::Replace(n, c) => {
                editor.kill_ring.reset();
                try!(edit_replace_char(&mut s, c, n));
            }
            Cmd::EndOfFile => {
                editor.kill_ring.reset();
                if !s.edit_state.is_emacs_mode() || s.line.is_empty() {
                    return Err(error::ReadlineError::Eof);
                } else {
                    try!(edit_delete(&mut s, 1))
                }
            }
            Cmd::EndOfLine => {
                editor.kill_ring.reset();
                // Move to the end of line.
                try!(edit_move_end(&mut s))
            }
            Cmd::ForwardChar(n) => {
                editor.kill_ring.reset();
                // Move forward a character.
                try!(edit_move_right(&mut s, n))
            }
            Cmd::BackwardDeleteChar(n) => {
                editor.kill_ring.reset();
                // Delete one character backward.
                try!(edit_backspace(&mut s, n))
            }
            Cmd::KillLine => {
                // Kill the text from point to the end of the line.
                if let Some(text) = try!(edit_kill_line(&mut s)) {
                    editor.kill_ring.kill(&text, Mode::Append)
                }
            }
            Cmd::KillWholeLine => {
                try!(edit_move_home(&mut s));
                if let Some(text) = try!(edit_kill_line(&mut s)) {
                    editor.kill_ring.kill(&text, Mode::Append)
                }
            }
            Cmd::ClearScreen => {
                // Clear the screen leaving the current line at the top of the screen.
                try!(s.term.clear_screen(&mut s.out));
                try!(s.refresh_line())
            }
            Cmd::NextHistory => {
                editor.kill_ring.reset();
                // Fetch the next command from the history list.
                try!(edit_history_next(&mut s, &editor.history, false))
            }
            Cmd::PreviousHistory => {
                editor.kill_ring.reset();
                // Fetch the previous command from the history list.
                try!(edit_history_next(&mut s, &editor.history, true))
            }
            Cmd::TransposeChars => {
                editor.kill_ring.reset();
                // Exchange the char before cursor with the character at cursor.
                try!(edit_transpose_chars(&mut s))
            }
            Cmd::UnixLikeDiscard => {
                // Kill backward from point to the beginning of the line.
                if let Some(text) = try!(edit_discard_line(&mut s)) {
                    editor.kill_ring.kill(&text, Mode::Prepend)
                }
            }
            #[cfg(unix)]
            Cmd::QuotedInsert => {
                // Quoted insert
                editor.kill_ring.reset();
                let c = try!(rdr.next_char());
                try!(edit_insert(&mut s, c, 1)) // FIXME
            }
            Cmd::Yank(n, anchor) => {
                // retrieve (yank) last item killed
                if let Some(text) = editor.kill_ring.yank() {
                    try!(edit_yank(&mut s, text, anchor, n))
                }
            }
            // TODO CTRL-_ // undo
            Cmd::AcceptLine => {
                // Accept the line regardless of where the cursor is.
                editor.kill_ring.reset();
                try!(edit_move_end(&mut s));
                break;
            }
            Cmd::BackwardKillWord(n, word_def) => {
                // kill one word backward
                if let Some(text) = try!(edit_delete_prev_word(&mut s, word_def, n)) {
                    editor.kill_ring.kill(&text, Mode::Prepend)
                }
            }
            Cmd::BeginningOfHistory => {
                // move to first entry in history
                editor.kill_ring.reset();
                try!(edit_history(&mut s, &editor.history, true))
            }
            Cmd::EndOfHistory => {
                // move to last entry in history
                editor.kill_ring.reset();
                try!(edit_history(&mut s, &editor.history, false))
            }
            Cmd::BackwardWord(n, word_def) => {
                // move backwards one word
                editor.kill_ring.reset();
                try!(edit_move_to_prev_word(&mut s, word_def, n))
            }
            Cmd::CapitalizeWord => {
                // capitalize word after point
                editor.kill_ring.reset();
                try!(edit_word(&mut s, WordAction::CAPITALIZE))
            }
            Cmd::KillWord(n, at, word_def) => {
                // kill one word forward
                if let Some(text) = try!(edit_delete_word(&mut s, at, word_def, n)) {
                    editor.kill_ring.kill(&text, Mode::Append)
                }
            }
            Cmd::ForwardWord(n, at, word_def) => {
                // move forwards one word
                editor.kill_ring.reset();
                try!(edit_move_to_next_word(&mut s, at, word_def, n))
            }
            Cmd::DowncaseWord => {
                // lowercase word after point
                editor.kill_ring.reset();
                try!(edit_word(&mut s, WordAction::LOWERCASE))
            }
            Cmd::TransposeWords => {
                // transpose words
                editor.kill_ring.reset();
                try!(edit_transpose_words(&mut s))
            }
            Cmd::UpcaseWord => {
                // uppercase word after point
                editor.kill_ring.reset();
                try!(edit_word(&mut s, WordAction::UPPERCASE))
            }
            Cmd::YankPop => {
                // yank-pop
                if let Some((yank_size, text)) = editor.kill_ring.yank_pop() {
                    try!(edit_yank_pop(&mut s, yank_size, text))
                }
            }
            Cmd::ViCharSearch(n, cs) => {
                editor.kill_ring.reset();
                try!(edit_move_to(&mut s, cs, n))
            }
            Cmd::ViDeleteTo(n, cs) => {
                if let Some(text) = try!(edit_delete_to(&mut s, cs, n)) {
                    editor.kill_ring.kill(&text, Mode::Append)
                }
            }
            Cmd::Interrupt => {
                editor.kill_ring.reset();
                return Err(error::ReadlineError::Interrupted);
            }
            #[cfg(unix)]
            Cmd::Suspend => {
                try!(original_mode.disable_raw_mode());
                try!(tty::suspend());
                try!(s.term.enable_raw_mode()); // TODO original_mode may have changed
                try!(s.refresh_line());
                continue;
            }
            Cmd::Noop => {}
            _ => {
                editor.kill_ring.reset();
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
        mode.disable_raw_mode();
    }
}

/// Readline method that will enable RAW mode, call the `readline_edit()`
/// method and disable raw mode
fn readline_raw<C: Completer>(prompt: &str, editor: &mut Editor<C>) -> Result<String> {
    let original_mode = try!(editor.term.enable_raw_mode());
    let guard = Guard(original_mode);
    let user_input = readline_edit(prompt, editor, original_mode);
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
pub struct Editor<C: Completer> {
    term: Terminal,
    history: History,
    completer: Option<C>,
    kill_ring: KillRing,
    config: Config,
}

impl<C: Completer> Editor<C> {
    pub fn new() -> Editor<C> {
        Self::with_config(Config::default())
    }

    pub fn with_config(config: Config) -> Editor<C> {
        let term = Terminal::new();
        Editor {
            term: term,
            history: History::with_config(config),
            completer: None,
            kill_ring: KillRing::new(60),
            config: config,
        }
    }

    /// This method will read a line from STDIN and will display a `prompt`
    pub fn readline(&mut self, prompt: &str) -> Result<String> {
        if self.term.is_unsupported() {
            // Write prompt and flush it to stdout
            let mut stdout = io::stdout();
            try!(write_and_flush(&mut stdout, prompt.as_bytes()));

            readline_direct()
        } else if !self.term.is_stdin_tty() {
            // Not a tty: read from file / pipe.
            readline_direct()
        } else {
            readline_raw(prompt, self)
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
    pub fn add_history_entry<S: AsRef<str> + Into<String>>(&mut self, line: S) -> bool {
        self.history.add(line)
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
    pub fn set_completer(&mut self, completer: Option<C>) {
        self.completer = completer;
    }

    /// ```
    /// let mut rl = rustyline::Editor::<()>::new();
    /// for readline in rl.iter("> ") {
    ///     match readline {
    ///         Ok(line) => {
    ///             println!("Line: {}", line);
    ///         },
    ///         Err(err) => {
    ///             println!("Error: {:?}", err);
    ///             break
    ///         }
    ///     }
    /// }
    /// ```
    pub fn iter<'a>(&'a mut self, prompt: &'a str) -> Iter<C> {
        Iter {
            editor: self,
            prompt: prompt,
        }
    }
}

impl<C: Completer> fmt::Debug for Editor<C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Editor")
            .field("term", &self.term)
            .field("config", &self.config)
            .finish()
    }
}

pub struct Iter<'a, C: Completer>
    where C: 'a
{
    editor: &'a mut Editor<C>,
    prompt: &'a str,
}

impl<'a, C: Completer> Iterator for Iter<'a, C> {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Result<String>> {
        let readline = self.editor.readline(self.prompt);
        match readline {
            Ok(l) => {
                self.editor.add_history_entry(l.as_ref()); // TODO Validate
                Some(Ok(l))
            }
            Err(error::ReadlineError::Eof) => None,
            e @ Err(_) => Some(e),
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::Write;
    use line_buffer::LineBuffer;
    use history::History;
    use completion::Completer;
    use config::Config;
    use consts::KeyPress;
    use keymap::{Cmd, EditState};
    use super::{Editor, Position, Result, State};
    use tty::{Terminal, Term};

    fn init_state<'out>(out: &'out mut Write,
                        line: &str,
                        pos: usize,
                        cols: usize)
                        -> State<'out, 'static> {
        let term = Terminal::new();
        let config = Config::default();
        State {
            out: out,
            prompt: "",
            prompt_size: Position::default(),
            line: LineBuffer::init(line, pos),
            cursor: Position::default(),
            cols: cols,
            old_rows: 0,
            history_index: 0,
            snapshot: LineBuffer::with_capacity(100),
            term: term,
            edit_state: EditState::new(&config),
        }
    }

    fn init_editor(keys: &[KeyPress]) -> Editor<()> {
        let mut editor = Editor::<()>::new();
        editor.term.keys.extend(keys.iter().cloned());
        editor
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
        let keys = &[KeyPress::Enter];
        let mut rdr = keys.iter();
        let completer = SimpleCompleter;
        let cmd = super::complete_line(&mut rdr, &mut s, &completer, &Config::default()).unwrap();
        assert_eq!(Some(Cmd::AcceptLine), cmd);
        assert_eq!("rust", s.line.as_str());
        assert_eq!(4, s.line.pos());
    }

    #[test]
    fn prompt_with_ansi_escape_codes() {
        let pos = super::calculate_position("\x1b[1;32m>>\x1b[0m ", Position::default(), 80);
        assert_eq!(3, pos.col);
        assert_eq!(0, pos.row);
    }

    fn assert_line(keys: &[KeyPress], expected_line: &str) {
        let mut editor = init_editor(keys);
        let actual_line = editor.readline(&">>").unwrap();
        assert_eq!(expected_line, actual_line);
    }

    #[test]
    fn delete_key() {
        assert_line(&[KeyPress::Char('a'), KeyPress::Delete, KeyPress::Enter],
                    "a");
        assert_line(&[KeyPress::Char('a'), KeyPress::Left, KeyPress::Delete, KeyPress::Enter],
                    "");
    }

    #[test]
    fn down_key() {
        assert_line(&[KeyPress::Down, KeyPress::Enter], "");
    }

    #[test]
    fn end_key() {
        assert_line(&[KeyPress::End, KeyPress::Enter], "");
    }

    #[test]
    fn home_key() {
        assert_line(&[KeyPress::Home, KeyPress::Enter], "");
    }

    #[test]
    fn left_key() {
        assert_line(&[KeyPress::Left, KeyPress::Enter], "");
    }

    #[test]
    fn meta_backspace_key() {
        assert_line(&[KeyPress::Meta('\x08'), KeyPress::Enter], "");
    }

    #[test]
    fn page_down_key() {
        assert_line(&[KeyPress::PageDown, KeyPress::Enter], "");
    }

    #[test]
    fn page_up_key() {
        assert_line(&[KeyPress::PageUp, KeyPress::Enter], "");
    }

    #[test]
    fn right_key() {
        assert_line(&[KeyPress::Right, KeyPress::Enter], "");
    }

    #[test]
    fn up_key() {
        assert_line(&[KeyPress::Up, KeyPress::Enter], "");
    }
}
