//! Command processor

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;

use super::{Context, Helper, Result};
use crate::highlight::Highlighter;
use crate::history::Direction;
use crate::keymap::{Anchor, At, CharSearch, Cmd, Movement, RepeatCount, Word};
use crate::keymap::{InputState, Refresher};
use crate::line_buffer::{LineBuffer, WordAction, MAX_LINE};
use crate::tty::{Position, RawReader, Renderer};
use crate::undo::Changeset;

/// Represent the state during line editing.
/// Implement rendering.
pub struct State<'out, 'prompt, H: Helper> {
    pub out: &'out mut dyn Renderer,
    prompt: &'prompt str,  // Prompt to display (rl_prompt)
    prompt_size: Position, // Prompt Unicode/visible width and height
    pub line: LineBuffer,  // Edited line buffer
    pub cursor: Position,  /* Cursor position (relative to the start of the prompt
                            * for `row`) */
    pub old_rows: usize, // Number of rows used so far (from start of prompt to end of input)
    saved_line_for_history: LineBuffer, // Current edited line before history browsing
    byte_buffer: [u8; 4],
    pub changes: Rc<RefCell<Changeset>>, // changes to line, for undo/redo
    pub helper: Option<&'out H>,
    pub ctx: Context<'out>,   // Give access to history for `hinter`
    pub hint: Option<String>, // last hint displayed
    highlight_char: bool,     // `true` if a char has been highlighted
}

enum Info<'m> {
    NoHint,
    Hint,
    Msg(Option<&'m str>),
}

impl<'out, 'prompt, H: Helper> State<'out, 'prompt, H> {
    pub fn new(
        out: &'out mut dyn Renderer,
        prompt: &'prompt str,
        helper: Option<&'out H>,
        ctx: Context<'out>,
    ) -> State<'out, 'prompt, H> {
        let capacity = MAX_LINE;
        let prompt_size = out.calculate_position(prompt, Position::default());
        State {
            out,
            prompt,
            prompt_size,
            line: LineBuffer::with_capacity(capacity),
            cursor: prompt_size,
            old_rows: 0,
            saved_line_for_history: LineBuffer::with_capacity(capacity),
            byte_buffer: [0; 4],
            changes: Rc::new(RefCell::new(Changeset::new())),
            helper,
            ctx,
            hint: None,
            highlight_char: false,
        }
    }

    pub fn highlighter(&self) -> Option<&dyn Highlighter> {
        if self.out.colors_enabled() {
            self.helper.map(|h| h as &dyn Highlighter)
        } else {
            None
        }
    }

    pub fn next_cmd<R: RawReader>(
        &mut self,
        input_state: &mut InputState,
        rdr: &mut R,
        single_esc_abort: bool,
    ) -> Result<Cmd> {
        loop {
            let rc = input_state.next_cmd(rdr, self, single_esc_abort);
            if rc.is_err() && self.out.sigwinch() {
                self.out.update_size();
                self.refresh_line()?;
                continue;
            }
            if let Ok(Cmd::Replace(_, _)) = rc {
                self.changes.borrow_mut().begin();
            }
            return rc;
        }
    }

    pub fn backup(&mut self) {
        self.saved_line_for_history
            .update(self.line.as_str(), self.line.pos());
    }

    pub fn restore(&mut self) {
        self.line.update(
            self.saved_line_for_history.as_str(),
            self.saved_line_for_history.pos(),
        );
    }

    pub fn move_cursor(&mut self) -> Result<()> {
        // calculate the desired position of the cursor
        let cursor = self
            .out
            .calculate_position(&self.line[..self.line.pos()], self.prompt_size);
        if self.cursor == cursor {
            return Ok(());
        }
        if self.highlight_char() {
            let prompt_size = self.prompt_size;
            self.refresh(self.prompt, prompt_size, Info::NoHint)?;
        } else {
            self.out.move_cursor(self.cursor, cursor)?;
        }
        self.cursor = cursor;
        Ok(())
    }

    fn refresh(&mut self, prompt: &str, prompt_size: Position, info: Info<'_>) -> Result<()> {
        let info = match info {
            Info::NoHint => None,
            Info::Hint => self.hint.as_ref().map(String::as_str),
            Info::Msg(msg) => msg,
        };
        let highlighter = if self.out.colors_enabled() {
            self.helper.map(|h| h as &dyn Highlighter)
        } else {
            None
        };
        let (cursor, end_pos) = self.out.refresh_line(
            prompt,
            prompt_size,
            &self.line,
            info,
            self.cursor.row,
            self.old_rows,
            highlighter,
        )?;

        self.cursor = cursor;
        self.old_rows = end_pos.row;
        Ok(())
    }

    pub fn hint(&mut self) {
        if let Some(hinter) = self.helper {
            let hint = hinter.hint(self.line.as_str(), self.line.pos(), &self.ctx);
            self.hint = hint;
        } else {
            self.hint = None
        }
    }

    fn highlight_char(&mut self) -> bool {
        if let Some(highlighter) = self.highlighter() {
            let highlight_char = highlighter.highlight_char(&self.line, self.line.pos());
            if highlight_char {
                self.highlight_char = true;
                true
            } else if self.highlight_char {
                // previously highlighted => force a full refresh
                self.highlight_char = false;
                true
            } else {
                false
            }
        } else {
            false
        }
    }
}

impl<'out, 'prompt, H: Helper> Refresher for State<'out, 'prompt, H> {
    fn refresh_line(&mut self) -> Result<()> {
        let prompt_size = self.prompt_size;
        self.hint();
        self.highlight_char();
        self.refresh(self.prompt, prompt_size, Info::Hint)
    }

    fn refresh_line_with_msg(&mut self, msg: Option<String>) -> Result<()> {
        let prompt_size = self.prompt_size;
        self.hint = None;
        self.highlight_char();
        self.refresh(
            self.prompt,
            prompt_size,
            Info::Msg(msg.as_ref().map(String::as_str)),
        )
    }

    fn refresh_prompt_and_line(&mut self, prompt: &str) -> Result<()> {
        let prompt_size = self.out.calculate_position(prompt, Position::default());
        self.hint();
        self.highlight_char();
        self.refresh(prompt, prompt_size, Info::Hint)
    }

    fn doing_insert(&mut self) {
        self.changes.borrow_mut().begin();
    }

    fn done_inserting(&mut self) {
        self.changes.borrow_mut().end();
    }

    fn last_insert(&self) -> Option<String> {
        self.changes.borrow().last_insert()
    }

    fn is_cursor_at_end(&self) -> bool {
        self.line.pos() == self.line.len()
    }

    fn has_hint(&self) -> bool {
        self.hint.is_some()
    }
}

impl<'out, 'prompt, H: Helper> fmt::Debug for State<'out, 'prompt, H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("prompt", &self.prompt)
            .field("prompt_size", &self.prompt_size)
            .field("buf", &self.line)
            .field("cursor", &self.cursor)
            .field("cols", &self.out.get_columns())
            .field("old_rows", &self.old_rows)
            .field("saved_line_for_history", &self.saved_line_for_history)
            .finish()
    }
}

impl<'out, 'prompt, H: Helper> State<'out, 'prompt, H> {
    /// Insert the character `ch` at cursor current position.
    pub fn edit_insert(&mut self, ch: char, n: RepeatCount) -> Result<()> {
        if let Some(push) = self.line.insert(ch, n) {
            if push {
                let prompt_size = self.prompt_size;
                let no_previous_hint = self.hint.is_none();
                self.hint();
                if n == 1
                    && self.cursor.col + ch.width().unwrap_or(0) < self.out.get_columns()
                    && (self.hint.is_none() && no_previous_hint) // TODO refresh only current line
                    && !self.highlight_char()
                {
                    // Avoid a full update of the line in the trivial case.
                    let cursor = self
                        .out
                        .calculate_position(&self.line[..self.line.pos()], self.prompt_size);
                    self.cursor = cursor;
                    let bits = ch.encode_utf8(&mut self.byte_buffer);
                    let bits = bits.as_bytes();
                    self.out.write_and_flush(bits)
                } else {
                    self.refresh(self.prompt, prompt_size, Info::Hint)
                }
            } else {
                self.refresh_line()
            }
        } else {
            Ok(())
        }
    }

    /// Replace a single (or n) character(s) under the cursor (Vi mode)
    pub fn edit_replace_char(&mut self, ch: char, n: RepeatCount) -> Result<()> {
        self.changes.borrow_mut().begin();
        let succeed = if let Some(chars) = self.line.delete(n) {
            let count = chars.graphemes(true).count();
            self.line.insert(ch, count);
            self.line.move_backward(1);
            true
        } else {
            false
        };
        self.changes.borrow_mut().end();
        if succeed {
            self.refresh_line()
        } else {
            Ok(())
        }
    }

    /// Overwrite the character under the cursor (Vi mode)
    pub fn edit_overwrite_char(&mut self, ch: char) -> Result<()> {
        if let Some(end) = self.line.next_pos(1) {
            {
                let text = ch.encode_utf8(&mut self.byte_buffer);
                let start = self.line.pos();
                self.line.replace(start..end, text);
            }
            self.refresh_line()
        } else {
            Ok(())
        }
    }

    // Yank/paste `text` at current position.
    pub fn edit_yank(
        &mut self,
        input_state: &InputState,
        text: &str,
        anchor: Anchor,
        n: RepeatCount,
    ) -> Result<()> {
        if let Anchor::After = anchor {
            self.line.move_forward(1);
        }
        if self.line.yank(text, n).is_some() {
            if !input_state.is_emacs_mode() {
                self.line.move_backward(1);
            }
            self.refresh_line()
        } else {
            Ok(())
        }
    }

    // Delete previously yanked text and yank/paste `text` at current position.
    pub fn edit_yank_pop(&mut self, yank_size: usize, text: &str) -> Result<()> {
        self.changes.borrow_mut().begin();
        let result = if self.line.yank_pop(yank_size, text).is_some() {
            self.refresh_line()
        } else {
            Ok(())
        };
        self.changes.borrow_mut().end();
        result
    }

    /// Move cursor on the left.
    pub fn edit_move_backward(&mut self, n: RepeatCount) -> Result<()> {
        if self.line.move_backward(n) {
            self.move_cursor()
        } else {
            Ok(())
        }
    }

    /// Move cursor on the right.
    pub fn edit_move_forward(&mut self, n: RepeatCount) -> Result<()> {
        if self.line.move_forward(n) {
            self.move_cursor()
        } else {
            Ok(())
        }
    }

    /// Move cursor to the start of the line.
    pub fn edit_move_home(&mut self) -> Result<()> {
        if self.line.move_home() {
            self.move_cursor()
        } else {
            Ok(())
        }
    }

    /// Move cursor to the end of the line.
    pub fn edit_move_end(&mut self) -> Result<()> {
        if self.line.move_end() {
            self.move_cursor()
        } else {
            Ok(())
        }
    }

    pub fn edit_kill(&mut self, mvt: &Movement) -> Result<()> {
        if self.line.kill(mvt) {
            self.refresh_line()
        } else {
            Ok(())
        }
    }

    pub fn edit_insert_text(&mut self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        let cursor = self.line.pos();
        self.line.insert_str(cursor, text);
        self.refresh_line()
    }

    pub fn edit_delete(&mut self, n: RepeatCount) -> Result<()> {
        if self.line.delete(n).is_some() {
            self.refresh_line()
        } else {
            Ok(())
        }
    }

    /// Exchange the char before cursor with the character at cursor.
    pub fn edit_transpose_chars(&mut self) -> Result<()> {
        self.changes.borrow_mut().begin();
        let succeed = self.line.transpose_chars();
        self.changes.borrow_mut().end();
        if succeed {
            self.refresh_line()
        } else {
            Ok(())
        }
    }

    pub fn edit_move_to_prev_word(&mut self, word_def: Word, n: RepeatCount) -> Result<()> {
        if self.line.move_to_prev_word(word_def, n) {
            self.move_cursor()
        } else {
            Ok(())
        }
    }

    pub fn edit_move_to_next_word(&mut self, at: At, word_def: Word, n: RepeatCount) -> Result<()> {
        if self.line.move_to_next_word(at, word_def, n) {
            self.move_cursor()
        } else {
            Ok(())
        }
    }

    pub fn edit_move_to(&mut self, cs: CharSearch, n: RepeatCount) -> Result<()> {
        if self.line.move_to(cs, n) {
            self.move_cursor()
        } else {
            Ok(())
        }
    }

    pub fn edit_word(&mut self, a: WordAction) -> Result<()> {
        self.changes.borrow_mut().begin();
        let succeed = self.line.edit_word(a);
        self.changes.borrow_mut().end();
        if succeed {
            self.refresh_line()
        } else {
            Ok(())
        }
    }

    pub fn edit_transpose_words(&mut self, n: RepeatCount) -> Result<()> {
        self.changes.borrow_mut().begin();
        let succeed = self.line.transpose_words(n);
        self.changes.borrow_mut().end();
        if succeed {
            self.refresh_line()
        } else {
            Ok(())
        }
    }

    /// Substitute the currently edited line with the next or previous history
    /// entry.
    pub fn edit_history_next(&mut self, prev: bool) -> Result<()> {
        let history = self.ctx.history;
        if history.is_empty() {
            return Ok(());
        }
        if self.ctx.history_index == history.len() {
            if prev {
                // Save the current edited line before overwriting it
                self.backup();
            } else {
                return Ok(());
            }
        } else if self.ctx.history_index == 0 && prev {
            return Ok(());
        }
        if prev {
            self.ctx.history_index -= 1;
        } else {
            self.ctx.history_index += 1;
        }
        if self.ctx.history_index < history.len() {
            let buf = history.get(self.ctx.history_index).unwrap();
            self.changes.borrow_mut().begin();
            self.line.update(buf, buf.len());
            self.changes.borrow_mut().end();
        } else {
            // Restore current edited line
            self.restore();
        }
        self.refresh_line()
    }

    // Non-incremental, anchored search
    pub fn edit_history_search(&mut self, dir: Direction) -> Result<()> {
        let history = self.ctx.history;
        if history.is_empty() {
            return self.out.beep();
        }
        if self.ctx.history_index == history.len() && dir == Direction::Forward
            || self.ctx.history_index == 0 && dir == Direction::Reverse
        {
            return self.out.beep();
        }
        if dir == Direction::Reverse {
            self.ctx.history_index -= 1;
        } else {
            self.ctx.history_index += 1;
        }
        if let Some(history_index) = history.starts_with(
            &self.line.as_str()[..self.line.pos()],
            self.ctx.history_index,
            dir,
        ) {
            self.ctx.history_index = history_index;
            let buf = history.get(history_index).unwrap();
            self.changes.borrow_mut().begin();
            self.line.update(buf, buf.len());
            self.changes.borrow_mut().end();
            self.refresh_line()
        } else {
            self.out.beep()
        }
    }

    /// Substitute the currently edited line with the first/last history entry.
    pub fn edit_history(&mut self, first: bool) -> Result<()> {
        let history = self.ctx.history;
        if history.is_empty() {
            return Ok(());
        }
        if self.ctx.history_index == history.len() {
            if first {
                // Save the current edited line before overwriting it
                self.backup();
            } else {
                return Ok(());
            }
        } else if self.ctx.history_index == 0 && first {
            return Ok(());
        }
        if first {
            self.ctx.history_index = 0;
            let buf = history.get(self.ctx.history_index).unwrap();
            self.changes.borrow_mut().begin();
            self.line.update(buf, buf.len());
            self.changes.borrow_mut().end();
        } else {
            self.ctx.history_index = history.len();
            // Restore current edited line
            self.restore();
        }
        self.refresh_line()
    }
}

#[cfg(test)]
pub fn init_state<'out, H: Helper>(
    out: &'out mut dyn Renderer,
    line: &str,
    pos: usize,
    helper: Option<&'out H>,
    history: &'out crate::history::History,
) -> State<'out, 'static, H> {
    State {
        out,
        prompt: "",
        prompt_size: Position::default(),
        line: LineBuffer::init(line, pos, None),
        cursor: Position::default(),
        old_rows: 0,
        saved_line_for_history: LineBuffer::with_capacity(100),
        byte_buffer: [0; 4],
        changes: Rc::new(RefCell::new(Changeset::new())),
        helper,
        ctx: Context {
            history,
            history_index: 0,
        },
        hint: Some("hint".to_owned()),
        highlight_char: false,
    }
}

#[cfg(test)]
mod test {
    use super::init_state;
    use crate::history::History;
    use crate::tty::Sink;

    #[test]
    fn edit_history_next() {
        let mut out = Sink::new();
        let mut history = History::new();
        history.add("line0");
        history.add("line1");
        let line = "current edited line";
        let helper: Option<()> = None;
        let mut s = init_state(&mut out, line, 6, helper.as_ref(), &history);
        s.ctx.history_index = history.len();

        for _ in 0..2 {
            s.edit_history_next(false).unwrap();
            assert_eq!(line, s.line.as_str());
        }

        s.edit_history_next(true).unwrap();
        assert_eq!(line, s.saved_line_for_history.as_str());
        assert_eq!(1, s.ctx.history_index);
        assert_eq!("line1", s.line.as_str());

        for _ in 0..2 {
            s.edit_history_next(true).unwrap();
            assert_eq!(line, s.saved_line_for_history.as_str());
            assert_eq!(0, s.ctx.history_index);
            assert_eq!("line0", s.line.as_str());
        }

        s.edit_history_next(false).unwrap();
        assert_eq!(line, s.saved_line_for_history.as_str());
        assert_eq!(1, s.ctx.history_index);
        assert_eq!("line1", s.line.as_str());

        s.edit_history_next(false).unwrap();
        // assert_eq!(line, s.saved_line_for_history);
        assert_eq!(2, s.ctx.history_index);
        assert_eq!(line, s.line.as_str());
    }
}
