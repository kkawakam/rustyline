//! Readline for Rust
//!
//! This implementation is based on [Antirez's
//! Linenoise](https://github.com/antirez/linenoise)
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
#![feature(io)]
#![feature(unicode)]
#![allow(unknown_lints)]

extern crate libc;
#[macro_use]
extern crate log;
#[cfg(unix)]
extern crate nix;
extern crate std_unicode;
extern crate unicode_segmentation;
extern crate unicode_width;
#[cfg(windows)]
extern crate winapi;

pub mod completion;
mod consts;
pub mod error;
pub mod hint;
pub mod history;
mod keymap;
mod kill_ring;
pub mod line_buffer;
pub mod config;
mod undo;

mod tty;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};
use std::path::Path;
use std::result;
use std::rc::Rc;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use tty::{Position, RawMode, RawReader, Renderer, Term, Terminal};

use completion::{longest_common_prefix, Completer};
use hint::Hinter;
use history::{Direction, History};
use line_buffer::{LineBuffer, WordAction, MAX_LINE};
pub use keymap::{Anchor, At, CharSearch, Cmd, Movement, RepeatCount, Word};
use keymap::EditState;
use kill_ring::{KillRing, Mode};
pub use config::{CompletionType, Config, EditMode, HistoryDuplicates};
use undo::Changeset;
pub use consts::KeyPress;

/// The error type for I/O and Linux Syscalls (Errno)
pub type Result<T> = result::Result<T, error::ReadlineError>;

/// Represent the state during line editing.
/// Implement rendering.
struct State<'out, 'prompt> {
    out: &'out mut Renderer,
    prompt: &'prompt str,  // Prompt to display
    prompt_size: Position, // Prompt Unicode/visible width and height
    line: LineBuffer,      // Edited line buffer
    cursor: Position,      /* Cursor position (relative to the start of the prompt
                            * for `row`) */
    old_rows: usize, // Number of rows used so far (from start of prompt to end of input)
    history_index: usize, // The history index we are currently editing
    saved_line_for_history: LineBuffer, // Current edited line before history browsing
    byte_buffer: [u8; 4],
    edit_state: EditState,
    changes: Rc<RefCell<Changeset>>,
    hinter: Option<&'out Hinter>,
}

impl<'out, 'prompt> State<'out, 'prompt> {
    fn new(
        out: &'out mut Renderer,
        config: &Config,
        prompt: &'prompt str,
        history_index: usize,
        custom_bindings: Rc<RefCell<HashMap<KeyPress, Cmd>>>,
        hinter: Option<&'out Hinter>,
    ) -> State<'out, 'prompt> {
        let capacity = MAX_LINE;
        let prompt_size = out.calculate_position(prompt, Position::default());
        State {
            out: out,
            prompt: prompt,
            prompt_size: prompt_size,
            line: LineBuffer::with_capacity(capacity),
            cursor: prompt_size,
            old_rows: prompt_size.row,
            history_index: history_index,
            saved_line_for_history: LineBuffer::with_capacity(capacity),
            byte_buffer: [0; 4],
            edit_state: EditState::new(config, custom_bindings),
            changes: Rc::new(RefCell::new(Changeset::new())),
            hinter: hinter,
        }
    }

    fn next_cmd<R: RawReader>(&mut self, rdr: &mut R) -> Result<Cmd> {
        loop {
            let rc = self.edit_state.next_cmd(rdr);
            if rc.is_err() && self.out.sigwinch() {
                self.out.update_size();
                try!(self.refresh_line());
                continue;
            }
            return rc;
        }
    }

    fn backup(&mut self) {
        self.saved_line_for_history
            .update(self.line.as_str(), self.line.pos());
    }
    fn restore(&mut self) {
        self.line.update(
            self.saved_line_for_history.as_str(),
            self.saved_line_for_history.pos(),
        );
    }

    fn move_cursor(&mut self) -> Result<()> {
        // calculate the desired position of the cursor
        let cursor = self.out
            .calculate_position(&self.line[..self.line.pos()], self.prompt_size);
        if self.cursor == cursor {
            return Ok(());
        }
        try!(self.out.move_cursor(self.cursor, cursor));
        self.cursor = cursor;
        Ok(())
    }

    /// Rewrite the currently edited line accordingly to the buffer content,
    /// cursor position, and number of columns of the terminal.
    fn refresh_line(&mut self) -> Result<()> {
        let prompt_size = self.prompt_size;
        let hint = self.hint();
        self.refresh(self.prompt, prompt_size, hint)
    }

    fn refresh_prompt_and_line(&mut self, prompt: &str) -> Result<()> {
        let prompt_size = self.out.calculate_position(prompt, Position::default());
        let hint = self.hint();
        self.refresh(prompt, prompt_size, hint)
    }

    fn refresh(&mut self, prompt: &str, prompt_size: Position, hint: Option<String>) -> Result<()> {
        let (cursor, end_pos) = try!(self.out.refresh_line(
            prompt,
            prompt_size,
            &self.line,
            hint,
            self.cursor.row,
            self.old_rows,
        ));

        self.cursor = cursor;
        self.old_rows = end_pos.row;
        Ok(())
    }

    fn hint(&self) -> Option<String> {
        if let Some(hinter) = self.hinter {
            hinter.hint(self.line.as_str(), self.line.pos())
        } else {
            None
        }
    }
}

impl<'out, 'prompt> fmt::Debug for State<'out, 'prompt> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("State")
            .field("prompt", &self.prompt)
            .field("prompt_size", &self.prompt_size)
            .field("buf", &self.line)
            .field("cursor", &self.cursor)
            .field("cols", &self.out.get_columns())
            .field("old_rows", &self.old_rows)
            .field("history_index", &self.history_index)
            .field("saved_line_for_history", &self.saved_line_for_history)
            .finish()
    }
}

/// Insert the character `ch` at cursor current position.
fn edit_insert(s: &mut State, ch: char, n: RepeatCount) -> Result<()> {
    if let Some(push) = s.line.insert(ch, n) {
        if push {
            let prompt_size = s.prompt_size;
            let hint = s.hint();
            if n == 1 && s.cursor.col + ch.width().unwrap_or(0) < s.out.get_columns()
                && hint.is_none()
            {
                // Avoid a full update of the line in the trivial case.
                let cursor = s.out
                    .calculate_position(&s.line[..s.line.pos()], s.prompt_size);
                s.cursor = cursor;
                let bits = ch.encode_utf8(&mut s.byte_buffer);
                let bits = bits.as_bytes();
                s.out.write_and_flush(bits)
            } else {
                s.refresh(s.prompt, prompt_size, hint)
            }
        } else {
            s.refresh_line()
        }
    } else {
        Ok(())
    }
}

/// Replace a single (or n) character(s) under the cursor (Vi mode)
fn edit_replace_char(s: &mut State, ch: char, n: RepeatCount) -> Result<()> {
    s.changes.borrow_mut().begin();
    let succeed = if let Some(chars) = s.line.delete(n) {
        let count = chars.graphemes(true).count();
        s.line.insert(ch, count);
        s.line.move_backward(1);
        true
    } else {
        false
    };
    s.changes.borrow_mut().end();
    if succeed {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Overwrite the character under the cursor (Vi mode)
fn edit_overwrite_char(s: &mut State, ch: char) -> Result<()> {
    if let Some(end) = s.line.next_pos(1) {
        {
            let text = ch.encode_utf8(&mut s.byte_buffer);
            let start = s.line.pos();
            s.line.replace(start..end, text);
        }
        s.refresh_line()
    } else {
        Ok(())
    }
}

// Yank/paste `text` at current position.
fn edit_yank(s: &mut State, text: &str, anchor: Anchor, n: RepeatCount) -> Result<()> {
    if let Anchor::After = anchor {
        s.line.move_forward(1);
    }
    if s.line.yank(text, n).is_some() {
        if !s.edit_state.is_emacs_mode() {
            s.line.move_backward(1);
        }
        s.refresh_line()
    } else {
        Ok(())
    }
}

// Delete previously yanked text and yank/paste `text` at current position.
fn edit_yank_pop(s: &mut State, yank_size: usize, text: &str) -> Result<()> {
    s.changes.borrow_mut().begin();
    s.line.yank_pop(yank_size, text);
    let result = edit_yank(s, text, Anchor::Before, 1);
    s.changes.borrow_mut().end();
    result
}

/// Move cursor on the left.
fn edit_move_backward(s: &mut State, n: RepeatCount) -> Result<()> {
    if s.line.move_backward(n) {
        s.move_cursor()
    } else {
        Ok(())
    }
}

/// Move cursor on the right.
fn edit_move_forward(s: &mut State, n: RepeatCount) -> Result<()> {
    if s.line.move_forward(n) {
        s.move_cursor()
    } else {
        Ok(())
    }
}

/// Move cursor to the start of the line.
fn edit_move_home(s: &mut State) -> Result<()> {
    if s.line.move_home() {
        s.move_cursor()
    } else {
        Ok(())
    }
}

/// Move cursor to the end of the line.
fn edit_move_end(s: &mut State) -> Result<()> {
    if s.line.move_end() {
        s.move_cursor()
    } else {
        Ok(())
    }
}

/// Delete the character at the right of the cursor without altering the cursor
/// position. Basically this is what happens with the "Delete" keyboard key.
fn edit_delete(s: &mut State, n: RepeatCount) -> Result<()> {
    if s.line.delete(n).is_some() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Backspace implementation.
fn edit_backspace(s: &mut State, n: RepeatCount) -> Result<()> {
    if s.line.backspace(n) {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Kill the text from point to the end of the line.
fn edit_kill_line(s: &mut State) -> Result<()> {
    if s.line.kill_line() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Kill backward from point to the beginning of the line.
fn edit_discard_line(s: &mut State) -> Result<()> {
    if s.line.discard_line() {
        s.refresh_line()
    } else {
        Ok(())
    }
}

/// Exchange the char before cursor with the character at cursor.
fn edit_transpose_chars(s: &mut State) -> Result<()> {
    s.changes.borrow_mut().begin();
    let succeed = s.line.transpose_chars();
    s.changes.borrow_mut().end();
    if succeed {
        s.refresh_line()
    } else {
        Ok(())
    }
}

fn edit_move_to_prev_word(s: &mut State, word_def: Word, n: RepeatCount) -> Result<()> {
    if s.line.move_to_prev_word(word_def, n) {
        s.move_cursor()
    } else {
        Ok(())
    }
}

/// Delete the previous word, maintaining the cursor at the start of the
/// current word.
fn edit_delete_prev_word(s: &mut State, word_def: Word, n: RepeatCount) -> Result<()> {
    if s.line.delete_prev_word(word_def, n) {
        s.refresh_line()
    } else {
        Ok(())
    }
}

fn edit_move_to_next_word(s: &mut State, at: At, word_def: Word, n: RepeatCount) -> Result<()> {
    if s.line.move_to_next_word(at, word_def, n) {
        s.move_cursor()
    } else {
        Ok(())
    }
}

fn edit_move_to(s: &mut State, cs: CharSearch, n: RepeatCount) -> Result<()> {
    if s.line.move_to(cs, n) {
        s.move_cursor()
    } else {
        Ok(())
    }
}

/// Kill from the cursor to the end of the current word, or, if between words,
/// to the end of the next word.
fn edit_delete_word(s: &mut State, at: At, word_def: Word, n: RepeatCount) -> Result<()> {
    if s.line.delete_word(at, word_def, n) {
        s.refresh_line()
    } else {
        Ok(())
    }
}

fn edit_delete_to(s: &mut State, cs: CharSearch, n: RepeatCount) -> Result<()> {
    if s.line.delete_to(cs, n) {
        s.refresh_line()
    } else {
        Ok(())
    }
}

fn edit_word(s: &mut State, a: WordAction) -> Result<()> {
    s.changes.borrow_mut().begin();
    let succeed = s.line.edit_word(a);
    s.changes.borrow_mut().end();
    if succeed {
        s.refresh_line()
    } else {
        Ok(())
    }
}

fn edit_transpose_words(s: &mut State, n: RepeatCount) -> Result<()> {
    s.changes.borrow_mut().begin();
    let succeed = s.line.transpose_words(n);
    s.changes.borrow_mut().end();
    if succeed {
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
            // Save the current edited line before overwriting it
            s.backup();
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
        s.changes.borrow_mut().begin();
        s.line.update(buf, buf.len());
        s.changes.borrow_mut().end();
    } else {
        // Restore current edited line
        s.restore();
    }
    s.refresh_line()
}

// Non-incremental, anchored search
fn edit_history_search(s: &mut State, history: &History, dir: Direction) -> Result<()> {
    if history.is_empty() {
        return s.out.beep();
    }
    if s.history_index == history.len() && dir == Direction::Forward {
        return s.out.beep();
    } else if s.history_index == 0 && dir == Direction::Reverse {
        return s.out.beep();
    }
    if dir == Direction::Reverse {
        s.history_index -= 1;
    } else {
        s.history_index += 1;
    }
    if let Some(history_index) =
        history.starts_with(&s.line.as_str()[..s.line.pos()], s.history_index, dir)
    {
        s.history_index = history_index;
        let buf = history.get(history_index).unwrap();
        s.changes.borrow_mut().begin();
        s.line.update(buf, buf.len());
        s.changes.borrow_mut().end();
        s.refresh_line()
    } else {
        s.out.beep()
    }
}

/// Substitute the currently edited line with the first/last history entry.
fn edit_history(s: &mut State, history: &History, first: bool) -> Result<()> {
    if history.is_empty() {
        return Ok(());
    }
    if s.history_index == history.len() {
        if first {
            // Save the current edited line before overwriting it
            s.backup();
        } else {
            return Ok(());
        }
    } else if s.history_index == 0 && first {
        return Ok(());
    }
    if first {
        s.history_index = 0;
        let buf = history.get(s.history_index).unwrap();
        s.changes.borrow_mut().begin();
        s.line.update(buf, buf.len());
        s.changes.borrow_mut().end();
    } else {
        s.history_index = history.len();
        // Restore current edited line
        s.restore();
    }
    s.refresh_line()
}

/// Completes the line/word
fn complete_line<R: RawReader, C: Completer>(
    rdr: &mut R,
    s: &mut State,
    completer: &C,
    config: &Config,
) -> Result<Option<Cmd>> {
    // get a list of completions
    let (start, candidates) = try!(completer.complete(&s.line, s.line.pos()));
    // if no completions, we are done
    if candidates.is_empty() {
        try!(s.out.beep());
        Ok(None)
    } else if CompletionType::Circular == config.completion_type() {
        let mark = s.changes.borrow_mut().begin();
        // Save the current edited line before overwriting it
        let backup = s.line.as_str().to_owned();
        let backup_pos = s.line.pos();
        let mut cmd;
        let mut i = 0;
        loop {
            // Show completion or original buffer
            if i < candidates.len() {
                completer.update(&mut s.line, start, &candidates[i]);
                try!(s.refresh_line());
            } else {
                // Restore current edited line
                s.line.update(&backup, backup_pos);
                try!(s.refresh_line());
            }

            cmd = try!(s.next_cmd(rdr));
            match cmd {
                Cmd::Complete => {
                    i = (i + 1) % (candidates.len() + 1); // Circular
                    if i == candidates.len() {
                        try!(s.out.beep());
                    }
                }
                Cmd::Abort => {
                    // Re-show original buffer
                    if i < candidates.len() {
                        s.line.update(&backup, backup_pos);
                        try!(s.refresh_line());
                    }
                    s.changes.borrow_mut().truncate(mark);
                    return Ok(None);
                }
                _ => {
                    s.changes.borrow_mut().end();
                    break;
                }
            }
        }
        Ok(Some(cmd))
    } else if CompletionType::List == config.completion_type() {
        // beep if ambiguous
        if candidates.len() > 1 {
            try!(s.out.beep());
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
        let mut cmd = try!(s.next_cmd(rdr));
        // if any character other than tab, pass it to the main loop
        if cmd != Cmd::Complete {
            return Ok(Some(cmd));
        }
        // move cursor to EOL to avoid overwriting the command line
        let save_pos = s.line.pos();
        try!(edit_move_end(s));
        s.line.set_pos(save_pos);
        // we got a second tab, maybe show list of possible completions
        let show_completions = if candidates.len() > config.completion_prompt_limit() {
            let msg = format!("\nDisplay all {} possibilities? (y or n)", candidates.len());
            try!(s.out.write_and_flush(msg.as_bytes()));
            s.old_rows += 1;
            while cmd != Cmd::SelfInsert(1, 'y') && cmd != Cmd::SelfInsert(1, 'Y')
                && cmd != Cmd::SelfInsert(1, 'n')
                && cmd != Cmd::SelfInsert(1, 'N')
                && cmd != Cmd::Kill(Movement::BackwardChar(1))
            {
                cmd = try!(s.next_cmd(rdr));
            }
            match cmd {
                Cmd::SelfInsert(1, 'y') | Cmd::SelfInsert(1, 'Y') => true,
                _ => false,
            }
        } else {
            true
        };
        if show_completions {
            page_completions(rdr, s, &candidates)
        } else {
            try!(s.refresh_line());
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn page_completions<R: RawReader>(
    rdr: &mut R,
    s: &mut State,
    candidates: &[String],
) -> Result<Option<Cmd>> {
    use std::cmp;

    let min_col_pad = 2;
    let cols = s.out.get_columns();
    let max_width = cmp::min(
        cols,
        candidates
            .into_iter()
            .map(|s| s.as_str().width())
            .max()
            .unwrap() + min_col_pad,
    );
    let num_cols = cols / max_width;

    let mut pause_row = s.out.get_rows() - 1;
    let num_rows = (candidates.len() + num_cols - 1) / num_cols;
    let mut ab = String::new();
    for row in 0..num_rows {
        if row == pause_row {
            try!(s.out.write_and_flush(b"\n--More--"));
            let mut cmd = Cmd::Noop;
            while cmd != Cmd::SelfInsert(1, 'y') && cmd != Cmd::SelfInsert(1, 'Y')
                && cmd != Cmd::SelfInsert(1, 'n')
                && cmd != Cmd::SelfInsert(1, 'N')
                && cmd != Cmd::SelfInsert(1, 'q')
                && cmd != Cmd::SelfInsert(1, 'Q')
                && cmd != Cmd::SelfInsert(1, ' ')
                && cmd != Cmd::Kill(Movement::BackwardChar(1))
                && cmd != Cmd::AcceptLine
            {
                cmd = try!(s.next_cmd(rdr));
            }
            match cmd {
                Cmd::SelfInsert(1, 'y') | Cmd::SelfInsert(1, 'Y') | Cmd::SelfInsert(1, ' ') => {
                    pause_row += s.out.get_rows() - 1;
                }
                Cmd::AcceptLine => {
                    pause_row += 1;
                }
                _ => break,
            }
            try!(s.out.write_and_flush(b"\n"));
        } else {
            try!(s.out.write_and_flush(b"\n"));
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
        try!(s.out.write_and_flush(ab.as_bytes()));
    }
    try!(s.out.write_and_flush(b"\n"));
    try!(s.refresh_line());
    Ok(None)
}

/// Incremental search
fn reverse_incremental_search<R: RawReader>(
    rdr: &mut R,
    s: &mut State,
    history: &History,
) -> Result<Option<Cmd>> {
    if history.is_empty() {
        return Ok(None);
    }
    let mark = s.changes.borrow_mut().begin();
    // Save the current edited line (and cursor position) before overwriting it
    let backup = s.line.as_str().to_owned();
    let backup_pos = s.line.pos();

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

        cmd = try!(s.next_cmd(rdr));
        if let Cmd::SelfInsert(_, c) = cmd {
            search_buf.push(c);
        } else {
            match cmd {
                Cmd::Kill(Movement::BackwardChar(_)) => {
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
                    s.line.update(&backup, backup_pos);
                    try!(s.refresh_line());
                    s.changes.borrow_mut().truncate(mark);
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
    s.changes.borrow_mut().end();
    Ok(Some(cmd))
}

/// Handles reading and editting the readline buffer.
/// It will also handle special inputs in an appropriate fashion
/// (e.g., C-c will exit readline)
#[allow(let_unit_value)]
fn readline_edit<H: Helper>(
    prompt: &str,
    initial: Option<(&str, &str)>,
    editor: &mut Editor<H>,
    original_mode: &tty::Mode,
) -> Result<String> {
    let completer = editor.helper.as_ref().map(|h| h.completer());
    let hinter = editor.helper.as_ref().map(|h| h.hinter() as &Hinter);

    let mut stdout = editor.term.create_writer();

    editor.reset_kill_ring();
    let mut s = State::new(
        &mut stdout,
        &editor.config,
        prompt,
        editor.history.len(),
        Rc::clone(&editor.custom_bindings),
        hinter,
    );

    s.line.set_delete_listener(editor.kill_ring.clone());
    s.line.set_change_listener(s.changes.clone());

    if let Some((left, right)) = initial {
        s.line
            .update((left.to_owned() + right).as_ref(), left.len());
    }

    try!(s.refresh_line());

    let mut rdr = try!(editor.term.create_reader(&editor.config));

    loop {
        let rc = s.next_cmd(&mut rdr);
        let mut cmd = try!(rc);

        if cmd.should_reset_kill_ring() {
            editor.reset_kill_ring();
        }

        // autocomplete
        if cmd == Cmd::Complete && completer.is_some() {
            let next = try!(complete_line(
                &mut rdr,
                &mut s,
                completer.unwrap(),
                &editor.config,
            ));
            if next.is_some() {
                cmd = next.unwrap();
            } else {
                continue;
            }
        }

        if let Cmd::SelfInsert(n, c) = cmd {
            try!(edit_insert(&mut s, c, n));
            continue;
        } else if let Cmd::Insert(n, text) = cmd {
            try!(edit_yank(&mut s, &text, Anchor::Before, n));
            continue;
        }

        if cmd == Cmd::ReverseSearchHistory {
            // Search history backward
            let next = try!(reverse_incremental_search(
                &mut rdr,
                &mut s,
                &editor.history,
            ));
            if next.is_some() {
                cmd = next.unwrap();
            } else {
                continue;
            }
        }

        match cmd {
            Cmd::Move(Movement::BeginningOfLine) => {
                // Move to the beginning of line.
                try!(edit_move_home(&mut s))
            }
            Cmd::Move(Movement::ViFirstPrint) => {
                try!(edit_move_home(&mut s));
                try!(edit_move_to_next_word(&mut s, At::Start, Word::Big, 1))
            }
            Cmd::Move(Movement::BackwardChar(n)) => {
                // Move back a character.
                try!(edit_move_backward(&mut s, n))
            }
            Cmd::Kill(Movement::ForwardChar(n)) => {
                // Delete (forward) one character at point.
                try!(edit_delete(&mut s, n))
            }
            Cmd::Replace(n, c) => {
                try!(edit_replace_char(&mut s, c, n));
            }
            Cmd::Overwrite(c) => {
                try!(edit_overwrite_char(&mut s, c));
            }
            Cmd::EndOfFile => if !s.edit_state.is_emacs_mode() && !s.line.is_empty() {
                try!(edit_move_end(&mut s));
                break;
            } else if s.line.is_empty() {
                return Err(error::ReadlineError::Eof);
            } else {
                try!(edit_delete(&mut s, 1))
            },
            Cmd::Move(Movement::EndOfLine) => {
                // Move to the end of line.
                try!(edit_move_end(&mut s))
            }
            Cmd::Move(Movement::ForwardChar(n)) => {
                // Move forward a character.
                try!(edit_move_forward(&mut s, n))
            }
            Cmd::Kill(Movement::BackwardChar(n)) => {
                // Delete one character backward.
                try!(edit_backspace(&mut s, n))
            }
            Cmd::Kill(Movement::EndOfLine) => {
                // Kill the text from point to the end of the line.
                editor.kill_ring.borrow_mut().start_killing();
                try!(edit_kill_line(&mut s));
                editor.kill_ring.borrow_mut().stop_killing();
            }
            Cmd::Kill(Movement::WholeLine) => {
                try!(edit_move_home(&mut s));
                editor.kill_ring.borrow_mut().start_killing();
                try!(edit_kill_line(&mut s));
                editor.kill_ring.borrow_mut().stop_killing();
            }
            Cmd::ClearScreen => {
                // Clear the screen leaving the current line at the top of the screen.
                try!(s.out.clear_screen());
                try!(s.refresh_line())
            }
            Cmd::NextHistory => {
                // Fetch the next command from the history list.
                try!(edit_history_next(&mut s, &editor.history, false))
            }
            Cmd::PreviousHistory => {
                // Fetch the previous command from the history list.
                try!(edit_history_next(&mut s, &editor.history, true))
            }
            Cmd::HistorySearchBackward => try!(edit_history_search(
                &mut s,
                &editor.history,
                Direction::Reverse,
            )),
            Cmd::HistorySearchForward => try!(edit_history_search(
                &mut s,
                &editor.history,
                Direction::Forward,
            )),
            Cmd::TransposeChars => {
                // Exchange the char before cursor with the character at cursor.
                try!(edit_transpose_chars(&mut s))
            }
            Cmd::Kill(Movement::BeginningOfLine) => {
                // Kill backward from point to the beginning of the line.
                editor.kill_ring.borrow_mut().start_killing();
                try!(edit_discard_line(&mut s));
                editor.kill_ring.borrow_mut().stop_killing();
            }
            #[cfg(unix)]
            Cmd::QuotedInsert => {
                // Quoted insert
                let c = try!(rdr.next_char());
                try!(edit_insert(&mut s, c, 1)) // FIXME
            }
            Cmd::Yank(n, anchor) => {
                // retrieve (yank) last item killed
                if let Some(text) = editor.kill_ring.borrow_mut().yank() {
                    try!(edit_yank(&mut s, text, anchor, n))
                }
            }
            Cmd::ViYankTo(mvt) => if let Some(text) = s.line.copy(mvt) {
                editor.kill_ring.borrow_mut().kill(&text, Mode::Append)
            },
            // TODO CTRL-_ // undo
            Cmd::AcceptLine => {
                // Accept the line regardless of where the cursor is.
                try!(edit_move_end(&mut s));
                if s.hinter.is_some() {
                    // Force a refresh without hints to leave the previous
                    // line as the user typed it after a newline.
                    s.hinter = None;
                    try!(s.refresh_line());
                }
                break;
            }
            Cmd::Kill(Movement::BackwardWord(n, word_def)) => {
                // kill one word backward (until start of word)
                editor.kill_ring.borrow_mut().start_killing();
                try!(edit_delete_prev_word(&mut s, word_def, n));
                editor.kill_ring.borrow_mut().stop_killing();
            }
            Cmd::BeginningOfHistory => {
                // move to first entry in history
                try!(edit_history(&mut s, &editor.history, true))
            }
            Cmd::EndOfHistory => {
                // move to last entry in history
                try!(edit_history(&mut s, &editor.history, false))
            }
            Cmd::Move(Movement::BackwardWord(n, word_def)) => {
                // move backwards one word
                try!(edit_move_to_prev_word(&mut s, word_def, n))
            }
            Cmd::CapitalizeWord => {
                // capitalize word after point
                try!(edit_word(&mut s, WordAction::CAPITALIZE))
            }
            Cmd::Kill(Movement::ForwardWord(n, at, word_def)) => {
                // kill one word forward (until start/end of word)
                editor.kill_ring.borrow_mut().start_killing();
                try!(edit_delete_word(&mut s, at, word_def, n));
                editor.kill_ring.borrow_mut().stop_killing();
            }
            Cmd::Move(Movement::ForwardWord(n, at, word_def)) => {
                // move forwards one word
                try!(edit_move_to_next_word(&mut s, at, word_def, n))
            }
            Cmd::DowncaseWord => {
                // lowercase word after point
                try!(edit_word(&mut s, WordAction::LOWERCASE))
            }
            Cmd::TransposeWords(n) => {
                // transpose words
                try!(edit_transpose_words(&mut s, n))
            }
            Cmd::UpcaseWord => {
                // uppercase word after point
                try!(edit_word(&mut s, WordAction::UPPERCASE))
            }
            Cmd::YankPop => {
                // yank-pop
                if let Some((yank_size, text)) = editor.kill_ring.borrow_mut().yank_pop() {
                    try!(edit_yank_pop(&mut s, yank_size, text))
                }
            }
            Cmd::Move(Movement::ViCharSearch(n, cs)) => try!(edit_move_to(&mut s, cs, n)),
            Cmd::Kill(Movement::ViCharSearch(n, cs)) => {
                editor.kill_ring.borrow_mut().start_killing();
                try!(edit_delete_to(&mut s, cs, n));
                editor.kill_ring.borrow_mut().stop_killing();
            }
            Cmd::Undo => {
                s.line.remove_change_listener();
                if s.changes.borrow_mut().undo(&mut s.line) {
                    try!(s.refresh_line());
                }
                s.line.set_change_listener(s.changes.clone());
            }
            Cmd::Interrupt => {
                return Err(error::ReadlineError::Interrupted);
            }
            #[cfg(unix)]
            Cmd::Suspend => {
                try!(original_mode.disable_raw_mode());
                try!(tty::suspend());
                try!(editor.term.enable_raw_mode()); // TODO original_mode may have changed
                try!(s.refresh_line());
                continue;
            }
            Cmd::Noop => {}
            _ => {
                // Ignore the character typed.
            }
        }
    }
    Ok(s.line.into_string())
}

struct Guard<'m>(&'m tty::Mode);

#[allow(unused_must_use)]
impl<'m> Drop for Guard<'m> {
    fn drop(&mut self) {
        let Guard(mode) = *self;
        mode.disable_raw_mode();
    }
}

/// Readline method that will enable RAW mode, call the `readline_edit()`
/// method and disable raw mode
fn readline_raw<H: Helper>(
    prompt: &str,
    initial: Option<(&str, &str)>,
    editor: &mut Editor<H>,
) -> Result<String> {
    let original_mode = try!(editor.term.enable_raw_mode());
    let guard = Guard(&original_mode);
    let user_input = readline_edit(prompt, initial, editor, &original_mode);
    if editor.config.auto_add_history() {
        if let Ok(ref line) = user_input {
            editor.add_history_entry(line.as_ref());
        }
    }
    drop(guard); // try!(disable_raw_mode(original_mode));
    println!();
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

pub trait Helper {
    type Completer: Completer;
    type Hinter: Hinter;

    fn completer(&self) -> &Self::Completer;
    fn hinter(&self) -> &Self::Hinter;
}

impl<C: Completer, H: Hinter> Helper for (C, H) {
    type Completer = C;
    type Hinter = H;

    fn completer(&self) -> &C {
        &self.0
    }
    fn hinter(&self) -> &H {
        &self.1
    }
}
impl<C: Completer> Helper for C {
    type Completer = C;
    type Hinter = ();

    fn completer(&self) -> &C {
        self
    }
    fn hinter(&self) -> &() {
        &()
    }
}

/// Line editor
pub struct Editor<H: Helper> {
    term: Terminal,
    history: History,
    helper: Option<H>,
    kill_ring: Rc<RefCell<KillRing>>,
    config: Config,
    custom_bindings: Rc<RefCell<HashMap<KeyPress, Cmd>>>,
}

impl<H: Helper> Editor<H> {
    /// Create an editor with the default configuration
    pub fn new() -> Editor<H> {
        Self::with_config(Config::default())
    }

    /// Create an editor with a specific configuration.
    pub fn with_config(config: Config) -> Editor<H> {
        let term = Terminal::new();
        Editor {
            term: term,
            history: History::with_config(config),
            helper: None,
            kill_ring: Rc::new(RefCell::new(KillRing::new(60))),
            config: config,
            custom_bindings: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// This method will read a line from STDIN and will display a `prompt`.
    ///
    /// It uses terminal-style interaction if `stdin` is connected to a
    /// terminal.
    /// Otherwise (e.g., if `stdin` is a pipe or the terminal is not supported),
    /// it uses file-style interaction.
    pub fn readline(&mut self, prompt: &str) -> Result<String> {
        self.readline_with(prompt, None)
    }
    /// This function behaves in the exact same manner as `readline`, except
    /// that it pre-populates the input area.
    ///
    /// The text that resides in the input area is given as a 2-tuple.
    /// The string on the left of the tuple what will appear to the left of the
    /// cursor
    /// and the string on the right is what will appear to the right of the
    /// cursor.
    pub fn readline_with_initial(&mut self, prompt: &str, initial: (&str, &str)) -> Result<String> {
        self.readline_with(prompt, Some(initial))
    }

    fn readline_with(&mut self, prompt: &str, initial: Option<(&str, &str)>) -> Result<String> {
        if self.term.is_unsupported() {
            debug!(target: "rustyline", "unsupported terminal");
            // Write prompt and flush it to stdout
            let mut stdout = io::stdout();
            try!(stdout.write_all(prompt.as_bytes()));
            try!(stdout.flush());

            readline_direct()
        } else if !self.term.is_stdin_tty() {
            debug!(target: "rustyline", "stdin is not a tty");
            // Not a tty: read from file / pipe.
            readline_direct()
        } else {
            readline_raw(prompt, initial, self)
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
    /// Return a mutable reference to the history object.
    pub fn get_history(&mut self) -> &mut History {
        &mut self.history
    }
    /// Return an immutable reference to the history object.
    pub fn get_history_const(&self) -> &History {
        &self.history
    }

    /// Register a callback function to be called for tab-completion
    /// or to show hints to the user at the right of the prompt.
    pub fn set_helper(&mut self, helper: Option<H>) {
        self.helper = helper;
    }

    /// Bind a sequence to a command.
    pub fn bind_sequence(&mut self, key_seq: KeyPress, cmd: Cmd) -> Option<Cmd> {
        self.custom_bindings.borrow_mut().insert(key_seq, cmd)
    }
    /// Remove a binding for the given sequence.
    pub fn unbind_sequence(&mut self, key_seq: KeyPress) -> Option<Cmd> {
        self.custom_bindings.borrow_mut().remove(&key_seq)
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
    pub fn iter<'a>(&'a mut self, prompt: &'a str) -> Iter<H> {
        Iter {
            editor: self,
            prompt: prompt,
        }
    }

    fn reset_kill_ring(&self) {
        self.kill_ring.borrow_mut().reset();
    }
}

impl<H: Helper> fmt::Debug for Editor<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Editor")
            .field("term", &self.term)
            .field("config", &self.config)
            .finish()
    }
}

/// Edited lines iterator
pub struct Iter<'a, H: Helper>
where
    H: 'a,
{
    editor: &'a mut Editor<H>,
    prompt: &'a str,
}

impl<'a, H: Helper> Iterator for Iter<'a, H> {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Result<String>> {
        let readline = self.editor.readline(self.prompt);
        match readline {
            Ok(l) => Some(Ok(l)),
            Err(error::ReadlineError::Eof) => None,
            e @ Err(_) => Some(e),
        }
    }
}

#[cfg(test)]
mod test {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;
    use line_buffer::LineBuffer;
    use history::History;
    use completion::Completer;
    use config::Config;
    use consts::KeyPress;
    use keymap::{Cmd, EditState};
    use super::{Editor, Position, Result, State};
    use tty::Renderer;
    use undo::Changeset;

    fn init_state<'out>(out: &'out mut Renderer, line: &str, pos: usize) -> State<'out, 'static> {
        let config = Config::default();
        State {
            out: out,
            prompt: "",
            prompt_size: Position::default(),
            line: LineBuffer::init(line, pos, None),
            cursor: Position::default(),
            old_rows: 0,
            history_index: 0,
            saved_line_for_history: LineBuffer::with_capacity(100),
            byte_buffer: [0; 4],
            edit_state: EditState::new(&config, Rc::new(RefCell::new(HashMap::new()))),
            changes: Rc::new(RefCell::new(Changeset::new())),
            hinter: None,
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
        let mut s = init_state(&mut out, line, 6);
        let mut history = History::new();
        history.add("line0");
        history.add("line1");
        s.history_index = history.len();

        for _ in 0..2 {
            super::edit_history_next(&mut s, &history, false).unwrap();
            assert_eq!(line, s.line.as_str());
        }

        super::edit_history_next(&mut s, &history, true).unwrap();
        assert_eq!(line, s.saved_line_for_history.as_str());
        assert_eq!(1, s.history_index);
        assert_eq!("line1", s.line.as_str());

        for _ in 0..2 {
            super::edit_history_next(&mut s, &history, true).unwrap();
            assert_eq!(line, s.saved_line_for_history.as_str());
            assert_eq!(0, s.history_index);
            assert_eq!("line0", s.line.as_str());
        }

        super::edit_history_next(&mut s, &history, false).unwrap();
        assert_eq!(line, s.saved_line_for_history.as_str());
        assert_eq!(1, s.history_index);
        assert_eq!("line1", s.line.as_str());

        super::edit_history_next(&mut s, &history, false).unwrap();
        // assert_eq!(line, s.saved_line_for_history);
        assert_eq!(2, s.history_index);
        assert_eq!(line, s.line.as_str());
    }

    struct SimpleCompleter;
    impl Completer for SimpleCompleter {
        fn complete(&self, line: &str, _pos: usize) -> Result<(usize, Vec<String>)> {
            Ok((0, vec![line.to_owned() + "t"]))
        }
    }

    #[test]
    fn complete_line() {
        let mut out = ::std::io::sink();
        let mut s = init_state(&mut out, "rus", 3);
        let keys = &[KeyPress::Enter];
        let mut rdr = keys.iter();
        let completer = SimpleCompleter;
        let cmd = super::complete_line(&mut rdr, &mut s, &completer, &Config::default()).unwrap();
        assert_eq!(Some(Cmd::AcceptLine), cmd);
        assert_eq!("rust", s.line.as_str());
        assert_eq!(4, s.line.pos());
    }

    fn assert_line(keys: &[KeyPress], expected_line: &str) {
        let mut editor = init_editor(keys);
        let actual_line = editor.readline(&">>").unwrap();
        assert_eq!(expected_line, actual_line);
    }

    #[test]
    fn delete_key() {
        assert_line(
            &[KeyPress::Char('a'), KeyPress::Delete, KeyPress::Enter],
            "a",
        );
        assert_line(
            &[
                KeyPress::Char('a'),
                KeyPress::Left,
                KeyPress::Delete,
                KeyPress::Enter,
            ],
            "",
        );
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

    #[test]
    fn unknown_esc_key() {
        assert_line(&[KeyPress::UnknownEscSeq, KeyPress::Enter], "");
    }
}
