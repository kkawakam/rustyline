use std::sync::{Arc, Mutex};

use crate::complete_hint_line;
use crate::config::Config;
use crate::edit::State;
use crate::error;
use crate::history::Direction;
use crate::keymap::{Anchor, At, Cmd, Movement, Word};
use crate::keymap::{InputState, Refresher, Invoke};
use crate::kill_ring::{KillRing, Mode};
use crate::line_buffer::WordAction;
use crate::{Helper, Result};
use crate::validate::{ValidationContext, ValidationResult};

pub enum Status {
    Proceed,
    Submit,
}

pub struct InvokeContext<'a, 'b, H: Helper> {
    pub state: &'a mut State<'b, 'b, H>,
    pub input_state: &'a InputState,
    pub kill_ring: &'a Arc<Mutex<KillRing>>,
    pub config: &'a Config,
}

pub fn execute<H: Helper>(cmd: Cmd, ctx: &mut InvokeContext<H>)
    -> Result<Status>
{
    use Status::*;
    let InvokeContext { state: s, input_state, kill_ring, config } = ctx;

    match cmd {
        Cmd::CompleteHint => {
            complete_hint_line(s)?;
        }
        Cmd::SelfInsert(n, c) => {
            s.edit_insert(c, n)?;
        }
        Cmd::Insert(n, text) => {
            s.edit_yank(&input_state, &text, Anchor::Before, n)?;
        }
        Cmd::Move(Movement::BeginningOfLine) => {
            // Move to the beginning of line.
            s.edit_move_home()?
        }
        Cmd::Move(Movement::ViFirstPrint) => {
            s.edit_move_home()?;
            s.edit_move_to_next_word(At::Start, Word::Big, 1)?
        }
        Cmd::Move(Movement::BackwardChar(n)) => {
            // Move back a character.
            s.edit_move_backward(n)?
        }
        Cmd::ReplaceChar(n, c) => s.edit_replace_char(c, n)?,
        Cmd::Replace(mvt, text) => {
            s.edit_kill(&mvt)?;
            if let Some(text) = text {
                s.edit_insert_text(&text)?
            }
        }
        Cmd::Overwrite(c) => {
            s.edit_overwrite_char(c)?;
        }
        Cmd::EndOfFile => {
            if input_state.is_emacs_mode() && !s.line.is_empty() {
                s.edit_delete(1)?
            } else {
                if s.has_hint() || !s.is_default_prompt() {
                    // Force a refresh without hints to leave the previous
                    // line as the user typed it after a newline.
                    s.refresh_line_with_msg(None)?;
                }
                if s.line.is_empty() {
                    return Err(error::ReadlineError::Eof);
                } else if !input_state.is_emacs_mode() {
                    return Ok(Submit);
                }
            }
        }
        Cmd::Move(Movement::EndOfLine) => {
            // Move to the end of line.
            s.edit_move_end()?
        }
        Cmd::Move(Movement::ForwardChar(n)) => {
            // Move forward a character.
            s.edit_move_forward(n)?
        }
        Cmd::ClearScreen => {
            // Clear the screen leaving the current line at the top of the screen.
            s.clear_screen()?;
            s.refresh_line()?
        }
        Cmd::NextHistory => {
            // Fetch the next command from the history list.
            s.edit_history_next(false)?
        }
        Cmd::PreviousHistory => {
            // Fetch the previous command from the history list.
            s.edit_history_next(true)?
        }
        Cmd::LineUpOrPreviousHistory(n) => {
            if !s.edit_move_line_up(n)? {
                s.edit_history_next(true)?
            }
        }
        Cmd::LineDownOrNextHistory(n) => {
            if !s.edit_move_line_down(n)? {
                s.edit_history_next(false)?
            }
        }
        Cmd::HistorySearchBackward => s.edit_history_search(Direction::Reverse)?,
        Cmd::HistorySearchForward => s.edit_history_search(Direction::Forward)?,
        Cmd::TransposeChars => {
            // Exchange the char before cursor with the character at cursor.
            s.edit_transpose_chars()?
        }
        Cmd::Yank(n, anchor) => {
            // retrieve (yank) last item killed
            let mut kill_ring = kill_ring.lock().unwrap();
            if let Some(text) = kill_ring.yank() {
                s.edit_yank(&input_state, text, anchor, n)?
            }
        }
        Cmd::ViYankTo(ref mvt) => {
            if let Some(text) = s.line.copy(mvt) {
                let mut kill_ring = kill_ring.lock().unwrap();
                kill_ring.kill(&text, Mode::Append)
            }
        }
        Cmd::AcceptLine | Cmd::AcceptOrInsertLine { .. } | Cmd::Newline => {
            if s.has_hint() || !s.is_default_prompt() {
                // Force a refresh without hints to leave the previous
                // line as the user typed it after a newline.
                s.refresh_line_with_msg(None)?;
            }
            let validation_result = validate(ctx)?;
            let valid = validation_result.is_valid();
            let end = ctx.state.line.is_end_of_input();
            match (cmd, valid, end) {
                (Cmd::AcceptLine, ..)
                | (Cmd::AcceptOrInsertLine { .. }, true, true)
                | (
                    Cmd::AcceptOrInsertLine {
                        accept_in_the_middle: true,
                    },
                    true,
                    _,
                ) => {
                    return Ok(Submit);
                }
                (Cmd::Newline, ..)
                | (Cmd::AcceptOrInsertLine { .. }, false, _)
                | (Cmd::AcceptOrInsertLine { .. }, true, false) => {
                    if valid || !validation_result.has_message() {
                        ctx.state.edit_insert('\n', 1)?;
                    }
                }
                _ => unreachable!(),
            }
        }
        Cmd::BeginningOfHistory => {
            // move to first entry in history
            s.edit_history(true)?
        }
        Cmd::EndOfHistory => {
            // move to last entry in history
            s.edit_history(false)?
        }
        Cmd::Move(Movement::BackwardWord(n, word_def)) => {
            // move backwards one word
            s.edit_move_to_prev_word(word_def, n)?
        }
        Cmd::CapitalizeWord => {
            // capitalize word after point
            s.edit_word(WordAction::Capitalize)?
        }
        Cmd::Kill(ref mvt) => {
            s.edit_kill(mvt)?;
        }
        Cmd::Move(Movement::ForwardWord(n, at, word_def)) => {
            // move forwards one word
            s.edit_move_to_next_word(at, word_def, n)?
        }
        Cmd::Move(Movement::LineUp(n)) => {
            s.edit_move_line_up(n)?;
        }
        Cmd::Move(Movement::LineDown(n)) => {
            s.edit_move_line_down(n)?;
        }
        Cmd::Move(Movement::BeginningOfBuffer) => {
            // Move to the start of the buffer.
            s.edit_move_buffer_start()?
        }
        Cmd::Move(Movement::EndOfBuffer) => {
            // Move to the end of the buffer.
            s.edit_move_buffer_end()?
        }
        Cmd::DowncaseWord => {
            // lowercase word after point
            s.edit_word(WordAction::Lowercase)?
        }
        Cmd::TransposeWords(n) => {
            // transpose words
            s.edit_transpose_words(n)?
        }
        Cmd::UpcaseWord => {
            // uppercase word after point
            s.edit_word(WordAction::Uppercase)?
        }
        Cmd::YankPop => {
            // yank-pop
            let mut kill_ring = kill_ring.lock().unwrap();
            if let Some((yank_size, text)) = kill_ring.yank_pop() {
                s.edit_yank_pop(yank_size, text)?
            }
        }
        Cmd::Move(Movement::ViCharSearch(n, cs)) => s.edit_move_to(cs, n)?,
        Cmd::Undo(n) => {
            if s.changes.borrow_mut().undo(&mut s.line, n) {
                s.refresh_line()?;
            }
        }
        Cmd::Dedent(mvt) => {
            s.edit_indent(&mvt, config.indent_size(), true)?;
        }
        Cmd::Indent(mvt) => {
            s.edit_indent(&mvt, config.indent_size(), false)?;
        }
        Cmd::Interrupt => {
            // Move to end, in case cursor was in the middle of the
            // line, so that next thing application prints goes after
            // the input
            s.edit_move_buffer_end()?;
            return Err(error::ReadlineError::Interrupted);
        }
        _ => {
            // Ignore the character typed.
        }
    }
    Ok(Proceed)
}

pub fn validate<H: Helper>(ctx: &mut InvokeContext<H>)
    -> Result<ValidationResult>
{
    if let Some(validator) = ctx.state.helper {
        ctx.state.changes.borrow_mut().begin();
        let result = validator.validate(&mut ValidationContext::new(ctx))?;
        let corrected = ctx.state.changes.borrow_mut().end();
        match result {
            ValidationResult::Incomplete => {}
            ValidationResult::Valid(ref msg) => {
                // Accept the line regardless of where the cursor is.
                if corrected || ctx.state.has_hint() || msg.is_some() {
                    // Force a refresh without hints to leave the previous
                    // line as the user typed it after a newline.
                    ctx.state.refresh_line_with_msg(msg.as_deref())?;
                }
            }
            ValidationResult::Invalid(ref msg) => {
                if corrected || ctx.state.has_hint() || msg.is_some() {
                    ctx.state.refresh_line_with_msg(msg.as_deref())?;
                }
            }
        }
        Ok(result)
    } else {
        Ok(ValidationResult::Valid(None))
    }
}

impl<H: Helper> Invoke for InvokeContext<'_, '_, H> {
    fn input(&self) -> &str {
        self.state.line.as_str()
    }
    fn invoke(&mut self, cmd: Cmd) -> Result<Status> {
        execute(cmd, self)
    }
}

