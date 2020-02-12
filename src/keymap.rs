//! Bindings from keys to command for Emacs and Vi modes
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use log::debug;

use super::Result;
use crate::config::Config;
use crate::config::EditMode;
use crate::keys::KeyPress;
use crate::tty::{RawReader, Term, Terminal};

/// The number of times one command should be repeated.
pub type RepeatCount = usize;

/// Commands
// #[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    /// abort
    Abort, // Miscellaneous Command
    /// accept-line
    AcceptLine,
    /// beginning-of-history
    BeginningOfHistory,
    /// capitalize-word
    CapitalizeWord,
    /// clear-screen
    ClearScreen,
    /// complete
    Complete,
    /// complete-backward
    CompleteBackward,
    /// complete-hint
    CompleteHint,
    /// downcase-word
    DowncaseWord,
    /// vi-eof-maybe
    EndOfFile,
    /// end-of-history
    EndOfHistory,
    /// forward-search-history
    ForwardSearchHistory,
    /// history-search-backward
    HistorySearchBackward,
    /// history-search-forward
    HistorySearchForward,
    Insert(RepeatCount, String),
    Interrupt,
    /// backward-delete-char, backward-kill-line, backward-kill-word
    /// delete-char, kill-line, kill-word, unix-line-discard, unix-word-rubout,
    /// vi-delete, vi-delete-to, vi-rubout
    Kill(Movement),
    /// backward-char, backward-word, beginning-of-line, end-of-line,
    /// forward-char, forward-word, vi-char-search, vi-end-word, vi-next-word,
    /// vi-prev-word
    Move(Movement),
    /// next-history
    NextHistory,
    Noop,
    /// vi-replace
    Overwrite(char),
    /// previous-history
    PreviousHistory,
    /// quoted-insert
    QuotedInsert,
    /// vi-change-char
    ReplaceChar(RepeatCount, char),
    /// vi-change-to, vi-substitute
    Replace(Movement, Option<String>),
    /// reverse-search-history
    ReverseSearchHistory,
    /// self-insert
    SelfInsert(RepeatCount, char),
    Suspend,
    /// transpose-chars
    TransposeChars,
    /// transpose-words
    TransposeWords(RepeatCount),
    /// undo
    Undo(RepeatCount),
    Unknown,
    /// upcase-word
    UpcaseWord,
    /// vi-yank-to
    ViYankTo(Movement),
    /// yank, vi-put
    Yank(RepeatCount, Anchor),
    /// yank-pop
    YankPop,
    /// moves cursor to the line above or switches to prev history entry if
    /// the cursor is already on the first line
    LineUpOrPreviousHistory,
    /// moves cursor to the line below or switches to next history entry if
    /// the cursor is already on the last line
    LineDownOrNextHistory,
    /// accepts the line when cursor is at the end of the text (non including
    /// trailing whitespace), inserts newline character otherwise
    AcceptOrInsertLine,
}

impl Cmd {
    pub fn should_reset_kill_ring(&self) -> bool {
        #[allow(clippy::match_same_arms)]
        match *self {
            Cmd::Kill(Movement::BackwardChar(_)) | Cmd::Kill(Movement::ForwardChar(_)) => true,
            Cmd::ClearScreen
            | Cmd::Kill(_)
            | Cmd::Replace(_, _)
            | Cmd::Noop
            | Cmd::Suspend
            | Cmd::Yank(_, _)
            | Cmd::YankPop => false,
            _ => true,
        }
    }

    fn is_repeatable_change(&self) -> bool {
        match *self {
            Cmd::Insert(_, _)
            | Cmd::Kill(_)
            | Cmd::ReplaceChar(_, _)
            | Cmd::Replace(_, _)
            | Cmd::SelfInsert(_, _)
            | Cmd::ViYankTo(_)
            | Cmd::Yank(_, _) => true,
            // Cmd::TransposeChars | TODO Validate
            _ => false,
        }
    }

    fn is_repeatable(&self) -> bool {
        match *self {
            Cmd::Move(_) => true,
            _ => self.is_repeatable_change(),
        }
    }

    // Replay this command with a possible different `RepeatCount`.
    fn redo(&self, new: Option<RepeatCount>, wrt: &dyn Refresher) -> Self {
        match *self {
            Cmd::Insert(previous, ref text) => {
                Cmd::Insert(repeat_count(previous, new), text.clone())
            }
            Cmd::Kill(ref mvt) => Cmd::Kill(mvt.redo(new)),
            Cmd::Move(ref mvt) => Cmd::Move(mvt.redo(new)),
            Cmd::ReplaceChar(previous, c) => Cmd::ReplaceChar(repeat_count(previous, new), c),
            Cmd::Replace(ref mvt, ref text) => {
                if text.is_none() {
                    let last_insert = wrt.last_insert();
                    if let Movement::ForwardChar(0) = mvt {
                        Cmd::Replace(
                            Movement::ForwardChar(last_insert.as_ref().map_or(0, String::len)),
                            last_insert,
                        )
                    } else {
                        Cmd::Replace(mvt.redo(new), last_insert)
                    }
                } else {
                    Cmd::Replace(mvt.redo(new), text.clone())
                }
            }
            Cmd::SelfInsert(previous, c) => {
                // consecutive char inserts are repeatable not only the last one...
                if let Some(text) = wrt.last_insert() {
                    Cmd::Insert(repeat_count(previous, new), text)
                } else {
                    Cmd::SelfInsert(repeat_count(previous, new), c)
                }
            }
            // Cmd::TransposeChars => Cmd::TransposeChars,
            Cmd::ViYankTo(ref mvt) => Cmd::ViYankTo(mvt.redo(new)),
            Cmd::Yank(previous, anchor) => Cmd::Yank(repeat_count(previous, new), anchor),
            _ => unreachable!(),
        }
    }
}

fn repeat_count(previous: RepeatCount, new: Option<RepeatCount>) -> RepeatCount {
    match new {
        Some(n) => n,
        None => previous,
    }
}

/// Different word definitions
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum Word {
    /// non-blanks characters
    Big,
    /// alphanumeric characters
    Emacs,
    /// alphanumeric (and '_') characters
    Vi,
}

/// Where to move with respect to word boundary
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum At {
    Start,
    BeforeEnd,
    AfterEnd,
}

/// Where to paste (relative to cursor position)
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum Anchor {
    After,
    Before,
}

/// Vi character search
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum CharSearch {
    Forward(char),
    // until
    ForwardBefore(char),
    Backward(char),
    // until
    BackwardAfter(char),
}

impl CharSearch {
    fn opposite(self) -> Self {
        match self {
            CharSearch::Forward(c) => CharSearch::Backward(c),
            CharSearch::ForwardBefore(c) => CharSearch::BackwardAfter(c),
            CharSearch::Backward(c) => CharSearch::Forward(c),
            CharSearch::BackwardAfter(c) => CharSearch::ForwardBefore(c),
        }
    }
}

/// Where to move
#[derive(Debug, Clone, PartialEq)]
pub enum Movement {
    WholeLine, // not really a movement
    /// beginning-of-line
    BeginningOfLine,
    /// end-of-line
    EndOfLine,
    /// backward-word, vi-prev-word
    BackwardWord(RepeatCount, Word), // Backward until start of word
    /// forward-word, vi-end-word, vi-next-word
    ForwardWord(RepeatCount, At, Word), // Forward until start/end of word
    /// vi-char-search
    ViCharSearch(RepeatCount, CharSearch),
    /// vi-first-print
    ViFirstPrint,
    /// backward-char
    BackwardChar(RepeatCount),
    /// forward-char
    ForwardChar(RepeatCount),
    /// move to the same column on the previous line
    LineUp(RepeatCount),
    /// move to the same column on the next line
    LineDown(RepeatCount),
    WholeBuffer, // not really a movement
    /// beginning-of-buffer
    BeginningOfBuffer,
    /// end-of-buffer
    EndOfBuffer,
}

impl Movement {
    // Replay this movement with a possible different `RepeatCount`.
    fn redo(&self, new: Option<RepeatCount>) -> Self {
        match *self {
            Movement::WholeLine => Movement::WholeLine,
            Movement::BeginningOfLine => Movement::BeginningOfLine,
            Movement::ViFirstPrint => Movement::ViFirstPrint,
            Movement::EndOfLine => Movement::EndOfLine,
            Movement::BackwardWord(previous, word) => {
                Movement::BackwardWord(repeat_count(previous, new), word)
            }
            Movement::ForwardWord(previous, at, word) => {
                Movement::ForwardWord(repeat_count(previous, new), at, word)
            }
            Movement::ViCharSearch(previous, char_search) => {
                Movement::ViCharSearch(repeat_count(previous, new), char_search)
            }
            Movement::BackwardChar(previous) => Movement::BackwardChar(repeat_count(previous, new)),
            Movement::ForwardChar(previous) => Movement::ForwardChar(repeat_count(previous, new)),
            Movement::LineUp(previous) => Movement::LineUp(repeat_count(previous, new)),
            Movement::LineDown(previous) => Movement::LineDown(repeat_count(previous, new)),
            Movement::WholeBuffer => Movement::WholeBuffer,
            Movement::BeginningOfBuffer => Movement::BeginningOfBuffer,
            Movement::EndOfBuffer => Movement::EndOfBuffer,
        }
    }
}

#[derive(PartialEq)]
enum InputMode {
    /// Vi Command/Alternate
    Command,
    /// Insert/Input mode
    Insert,
    /// Overwrite mode
    Replace,
}

/// Transform key(s) to commands based on current input mode
pub struct InputState {
    mode: EditMode,
    custom_bindings: Arc<RwLock<HashMap<KeyPress, Cmd>>>,
    input_mode: InputMode, // vi only ?
    // numeric arguments: http://web.mit.edu/gnu/doc/html/rlman_1.html#SEC7
    num_args: i16,
    last_cmd: Cmd,                        // vi only
    last_char_search: Option<CharSearch>, // vi only
}

/// Provide indirect mutation to user input.
pub trait Invoke {
    /// currently edited line
    fn input(&self) -> &str;
    // TODO
    //fn invoke(&mut self, cmd: Cmd) -> Result<?>;
}

pub trait Refresher {
    /// Rewrite the currently edited line accordingly to the buffer content,
    /// cursor position, and number of columns of the terminal.
    fn refresh_line(&mut self) -> Result<()>;
    /// Same as [`refresh_line`] with a specific message instead of hint
    fn refresh_line_with_msg(&mut self, msg: Option<String>) -> Result<()>;
    /// Same as `refresh_line` but with a dynamic prompt.
    fn refresh_prompt_and_line(&mut self, prompt: &str) -> Result<()>;
    /// Vi only, switch to insert mode.
    fn doing_insert(&mut self);
    /// Vi only, switch to command mode.
    fn done_inserting(&mut self);
    /// Vi only, last text inserted.
    fn last_insert(&self) -> Option<String>;
    /// Returns `true` if the cursor is currently at the end of the line.
    fn is_cursor_at_end(&self) -> bool;
    /// Returns `true` if there is a hint displayed.
    fn has_hint(&self) -> bool;
}

impl InputState {
    pub fn new(config: &Config, custom_bindings: Arc<RwLock<HashMap<KeyPress, Cmd>>>) -> Self {
        Self {
            mode: config.edit_mode(),
            custom_bindings,
            input_mode: InputMode::Insert,
            num_args: 0,
            last_cmd: Cmd::Noop,
            last_char_search: None,
        }
    }

    pub fn is_emacs_mode(&self) -> bool {
        self.mode == EditMode::Emacs
    }

    /// Parse user input into one command
    /// `single_esc_abort` is used in emacs mode on unix platform when a single
    /// esc key is expected to abort current action.
    pub fn next_cmd(
        &mut self,
        rdr: &mut <Terminal as Term>::Reader,
        wrt: &mut dyn Refresher,
        single_esc_abort: bool,
    ) -> Result<Cmd> {
        match self.mode {
            EditMode::Emacs => self.emacs(rdr, wrt, single_esc_abort),
            EditMode::Vi if self.input_mode != InputMode::Command => self.vi_insert(rdr, wrt),
            EditMode::Vi => self.vi_command(rdr, wrt),
        }
    }

    fn emacs_digit_argument<R: RawReader>(
        &mut self,
        rdr: &mut R,
        wrt: &mut dyn Refresher,
        digit: char,
    ) -> Result<KeyPress> {
        #[allow(clippy::cast_possible_truncation)]
        match digit {
            '0'..='9' => {
                self.num_args = digit.to_digit(10).unwrap() as i16;
            }
            '-' => {
                self.num_args = -1;
            }
            _ => unreachable!(),
        }
        loop {
            wrt.refresh_prompt_and_line(&format!("(arg: {}) ", self.num_args))?;
            let key = rdr.next_key(true)?;
            #[allow(clippy::cast_possible_truncation)]
            match key {
                key_press!(digit @ '0'..='9') | key_press!(META, digit @ '0'..='9') => {
                    if self.num_args == -1 {
                        self.num_args *= digit.to_digit(10).unwrap() as i16;
                    } else if self.num_args.abs() < 1000 {
                        // shouldn't ever need more than 4 digits
                        self.num_args = self
                            .num_args
                            .saturating_mul(10)
                            .saturating_add(digit.to_digit(10).unwrap() as i16);
                    }
                }
                key_press!('-') | key_press!(META, '-') => {}
                _ => {
                    wrt.refresh_line()?;
                    return Ok(key);
                }
            };
        }
    }

    fn emacs<R: RawReader>(
        &mut self,
        rdr: &mut R,
        wrt: &mut dyn Refresher,
        single_esc_abort: bool,
    ) -> Result<Cmd> {
        let mut key = rdr.next_key(single_esc_abort)?;
        if let key_press!(META, digit @ '-') = key {
            key = self.emacs_digit_argument(rdr, wrt, digit)?;
        } else if let key_press!(META, digit @ '0'..='9') = key {
            key = self.emacs_digit_argument(rdr, wrt, digit)?;
        }
        let (n, positive) = self.emacs_num_args(); // consume them in all cases
        {
            let bindings = self.custom_bindings.read().unwrap();
            if let Some(cmd) = bindings.get(&key) {
                debug!(target: "rustyline", "Custom command: {:?}", cmd);
                return Ok(if cmd.is_repeatable() {
                    cmd.redo(Some(n), wrt)
                } else {
                    cmd.clone()
                });
            }
        }
        let cmd = match key {
            key_press!(Char(c)) => {
                if positive {
                    Cmd::SelfInsert(n, c)
                } else {
                    Cmd::Unknown
                }
            }
            key_press!(CTRL, 'A') => Cmd::Move(Movement::BeginningOfLine),
            key_press!(CTRL, 'B') => {
                if positive {
                    Cmd::Move(Movement::BackwardChar(n))
                } else {
                    Cmd::Move(Movement::ForwardChar(n))
                }
            }
            key_press!(CTRL, 'E') => Cmd::Move(Movement::EndOfLine),
            key_press!(CTRL, 'F') => {
                if positive {
                    Cmd::Move(Movement::ForwardChar(n))
                } else {
                    Cmd::Move(Movement::BackwardChar(n))
                }
            }
            key_press!(CTRL, 'G') | key_press!(Esc) | key_press!(META, '\x07') => Cmd::Abort,
            key_press!(CTRL, 'H') | key_press!(Backspace) => {
                if positive {
                    Cmd::Kill(Movement::BackwardChar(n))
                } else {
                    Cmd::Kill(Movement::ForwardChar(n))
                }
            }
            key_press!(BackTab) => Cmd::CompleteBackward,
            key_press!(Tab) => {
                if positive {
                    Cmd::Complete
                } else {
                    Cmd::CompleteBackward
                }
            }
            // Don't complete hints when the cursor is not at the end of a line
            key_press!(Right) if wrt.has_hint() && wrt.is_cursor_at_end() => Cmd::CompleteHint,
            key_press!(CTRL, 'K') => {
                if positive {
                    Cmd::Kill(Movement::EndOfLine)
                } else {
                    Cmd::Kill(Movement::BeginningOfLine)
                }
            }
            key_press!(CTRL, 'L') => Cmd::ClearScreen,
            key_press!(CTRL, 'N') => Cmd::NextHistory,
            key_press!(CTRL, 'P') => Cmd::PreviousHistory,
            key_press!(CTRL, 'X') => {
                let snd_key = rdr.next_key(true)?;
                match snd_key {
                    key_press!(CTRL, 'G') | key_press!(Esc) => Cmd::Abort,
                    key_press!(CTRL, 'U') => Cmd::Undo(n),
                    _ => Cmd::Unknown,
                }
            }
            key_press!(META, '\x08') | key_press!(META, '\x7f') => {
                if positive {
                    Cmd::Kill(Movement::BackwardWord(n, Word::Emacs))
                } else {
                    Cmd::Kill(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
                }
            }
            key_press!(META, '<') => Cmd::BeginningOfHistory,
            key_press!(META, '>') => Cmd::EndOfHistory,
            key_press!(META, 'B') | key_press!(META, 'b') => {
                if positive {
                    Cmd::Move(Movement::BackwardWord(n, Word::Emacs))
                } else {
                    Cmd::Move(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
                }
            }
            key_press!(META, 'C') | key_press!(META, 'c') => Cmd::CapitalizeWord,
            key_press!(META, 'D') | key_press!(META, 'd') => {
                if positive {
                    Cmd::Kill(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
                } else {
                    Cmd::Kill(Movement::BackwardWord(n, Word::Emacs))
                }
            }
            key_press!(META, 'F') | key_press!(META, 'f') => {
                if positive {
                    Cmd::Move(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
                } else {
                    Cmd::Move(Movement::BackwardWord(n, Word::Emacs))
                }
            }
            key_press!(META, 'L') | key_press!(META, 'l') => Cmd::DowncaseWord,
            key_press!(META, 'T') | key_press!(META, 't') => Cmd::TransposeWords(n),
            key_press!(META, 'U') | key_press!(META, 'u') => Cmd::UpcaseWord,
            key_press!(META, 'Y') | key_press!(META, 'y') => Cmd::YankPop,
            _ => self.common(rdr, key, n, positive)?,
        };
        debug!(target: "rustyline", "Emacs command: {:?}", cmd);
        Ok(cmd)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn vi_arg_digit<R: RawReader>(
        &mut self,
        rdr: &mut R,
        wrt: &mut dyn Refresher,
        digit: char,
    ) -> Result<KeyPress> {
        self.num_args = digit.to_digit(10).unwrap() as i16;
        loop {
            wrt.refresh_prompt_and_line(&format!("(arg: {}) ", self.num_args))?;
            let key = rdr.next_key(false)?;
            if let key_press!(digit @ '0'..='9') = key {
                if self.num_args.abs() < 1000 {
                    // shouldn't ever need more than 4 digits
                    self.num_args = self
                        .num_args
                        .saturating_mul(10)
                        .saturating_add(digit.to_digit(10).unwrap() as i16);
                }
            } else {
                wrt.refresh_line()?;
                return Ok(key);
            };
        }
    }

    fn vi_command<R: RawReader>(&mut self, rdr: &mut R, wrt: &mut dyn Refresher) -> Result<Cmd> {
        let mut key = rdr.next_key(false)?;
        if let key_press!(Char(digit @ '1'..='9')) = key {
            key = self.vi_arg_digit(rdr, wrt, digit)?;
        }
        let no_num_args = self.num_args == 0;
        let n = self.vi_num_args(); // consume them in all cases
        {
            let bindings = self.custom_bindings.read().unwrap();
            if let Some(cmd) = bindings.get(&key) {
                debug!(target: "rustyline", "Custom command: {:?}", cmd);
                return Ok(if cmd.is_repeatable() {
                    if no_num_args {
                        cmd.redo(None, wrt)
                    } else {
                        cmd.redo(Some(n), wrt)
                    }
                } else {
                    cmd.clone()
                });
            }
        }
        let cmd = match key {
            key_press!('$') | key_press!(End) => Cmd::Move(Movement::EndOfLine),
            key_press!('.') => {
                // vi-redo (repeat last command)
                if no_num_args {
                    self.last_cmd.redo(None, wrt)
                } else {
                    self.last_cmd.redo(Some(n), wrt)
                }
            }
            // TODO key_press!('%') => Cmd::???, Move to the corresponding opening/closing
            // bracket
            key_press!('0') => Cmd::Move(Movement::BeginningOfLine),
            key_press!('^') => Cmd::Move(Movement::ViFirstPrint),
            key_press!('a') => {
                // vi-append-mode
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Move(Movement::ForwardChar(n))
            }
            key_press!('A') => {
                // vi-append-eol
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Move(Movement::EndOfLine)
            }
            key_press!('b') => Cmd::Move(Movement::BackwardWord(n, Word::Vi)), // vi-prev-word
            key_press!('B') => Cmd::Move(Movement::BackwardWord(n, Word::Big)),
            key_press!('c') => {
                self.input_mode = InputMode::Insert;
                match self.vi_cmd_motion(rdr, wrt, key, n)? {
                    Some(mvt) => Cmd::Replace(mvt, None),
                    None => Cmd::Unknown,
                }
            }
            key_press!('C') => {
                self.input_mode = InputMode::Insert;
                Cmd::Replace(Movement::EndOfLine, None)
            }
            key_press!('d') => match self.vi_cmd_motion(rdr, wrt, key, n)? {
                Some(mvt) => Cmd::Kill(mvt),
                None => Cmd::Unknown,
            },
            key_press!('D') | key_press!(CTRL, 'K') => Cmd::Kill(Movement::EndOfLine),
            key_press!('e') => Cmd::Move(Movement::ForwardWord(n, At::BeforeEnd, Word::Vi)),
            key_press!('E') => Cmd::Move(Movement::ForwardWord(n, At::BeforeEnd, Word::Big)),
            key_press!('i') => {
                // vi-insertion-mode
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Noop
            }
            key_press!('I') => {
                // vi-insert-beg
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Move(Movement::BeginningOfLine)
            }
            key_press!(Char(c)) if c == 'f' || c == 'F' || c == 't' || c == 'T' => {
                // vi-char-search
                let cs = self.vi_char_search(rdr, c)?;
                match cs {
                    Some(cs) => Cmd::Move(Movement::ViCharSearch(n, cs)),
                    None => Cmd::Unknown,
                }
            }
            key_press!(';') => match self.last_char_search {
                Some(cs) => Cmd::Move(Movement::ViCharSearch(n, cs)),
                None => Cmd::Noop,
            },
            key_press!(',') => match self.last_char_search {
                Some(ref cs) => Cmd::Move(Movement::ViCharSearch(n, cs.opposite())),
                None => Cmd::Noop,
            },
            // TODO key_press!('G') => Cmd::???, Move to the history line n
            key_press!('p') => Cmd::Yank(n, Anchor::After), // vi-put
            key_press!('P') => Cmd::Yank(n, Anchor::Before), // vi-put
            key_press!('r') => {
                // vi-replace-char:
                let ch = rdr.next_key(false)?;
                match ch {
                    key_press!(Char(c)) => Cmd::ReplaceChar(n, c),
                    key_press!(Esc) => Cmd::Noop,
                    _ => Cmd::Unknown,
                }
            }
            key_press!('R') => {
                //  vi-replace-mode (overwrite-mode)
                self.input_mode = InputMode::Replace;
                Cmd::Replace(Movement::ForwardChar(0), None)
            }
            key_press!('s') => {
                // vi-substitute-char:
                self.input_mode = InputMode::Insert;
                Cmd::Replace(Movement::ForwardChar(n), None)
            }
            key_press!('S') => {
                // vi-substitute-line:
                self.input_mode = InputMode::Insert;
                Cmd::Replace(Movement::WholeLine, None)
            }

            key_press!('u') => Cmd::Undo(n),
            // key_press!('U') => Cmd::???, // revert-line
            key_press!('w') => Cmd::Move(Movement::ForwardWord(n, At::Start, Word::Vi)), /* vi-next-word */
            key_press!('W') => Cmd::Move(Movement::ForwardWord(n, At::Start, Word::Big)), /* vi-next-word */
            // TODO move backward if eol
            key_press!('x') => Cmd::Kill(Movement::ForwardChar(n)), // vi-delete
            key_press!('X') => Cmd::Kill(Movement::BackwardChar(n)), // vi-rubout
            key_press!('y') => match self.vi_cmd_motion(rdr, wrt, key, n)? {
                Some(mvt) => Cmd::ViYankTo(mvt),
                None => Cmd::Unknown,
            },
            // key_press!('Y') => Cmd::???, // vi-yank-to
            key_press!('h') | key_press!(CTRL, 'H') | key_press!(Backspace) => {
                Cmd::Move(Movement::BackwardChar(n))
            }
            key_press!(CTRL, 'G') => Cmd::Abort,
            key_press!('l') | key_press!(' ') => Cmd::Move(Movement::ForwardChar(n)),
            key_press!(CTRL, 'L') => Cmd::ClearScreen,
            key_press!('+') | key_press!('j') => Cmd::Move(Movement::LineDown(n)),
            // TODO: move to the start of the line.
            key_press!(CTRL, 'N') => Cmd::NextHistory,
            key_press!('-') | key_press!('k') => Cmd::Move(Movement::LineUp(n)),

            // TODO: move to the start of the line.
            key_press!(CTRL, 'P') => Cmd::PreviousHistory,
            key_press!(CTRL, 'R') => {
                self.input_mode = InputMode::Insert; // TODO Validate
                Cmd::ReverseSearchHistory
            }
            key_press!(CTRL, 'S') => {
                self.input_mode = InputMode::Insert; // TODO Validate
                Cmd::ForwardSearchHistory
            }
            key_press!(Esc) => Cmd::Noop,
            _ => self.common(rdr, key, n, true)?,
        };
        debug!(target: "rustyline", "Vi command: {:?}", cmd);
        if cmd.is_repeatable_change() {
            self.last_cmd = cmd.clone();
        }
        Ok(cmd)
    }

    fn vi_insert<R: RawReader>(&mut self, rdr: &mut R, wrt: &mut dyn Refresher) -> Result<Cmd> {
        let key = rdr.next_key(false)?;
        {
            let bindings = self.custom_bindings.read().unwrap();
            if let Some(cmd) = bindings.get(&key) {
                debug!(target: "rustyline", "Custom command: {:?}", cmd);
                return Ok(if cmd.is_repeatable() {
                    cmd.redo(None, wrt)
                } else {
                    cmd.clone()
                });
            }
        }
        let cmd = match key {
            key_press!(Char(c)) => {
                if self.input_mode == InputMode::Replace {
                    Cmd::Overwrite(c)
                } else {
                    Cmd::SelfInsert(1, c)
                }
            }
            key_press!(CTRL, 'H') | key_press!(Backspace) => Cmd::Kill(Movement::BackwardChar(1)),
            key_press!(BackTab) => Cmd::CompleteBackward,
            key_press!(Tab) => Cmd::Complete,
            // Don't complete hints when the cursor is not at the end of a line
            key_press!(Right) if wrt.has_hint() && wrt.is_cursor_at_end() => Cmd::CompleteHint,
            key_press!(Esc) => {
                // vi-movement-mode/vi-command-mode
                self.input_mode = InputMode::Command;
                wrt.done_inserting();
                Cmd::Move(Movement::BackwardChar(1))
            }
            _ => self.common(rdr, key, 1, true)?,
        };
        debug!(target: "rustyline", "Vi insert: {:?}", cmd);
        if cmd.is_repeatable_change() {
            if let (Cmd::Replace(_, _), Cmd::SelfInsert(_, _)) = (&self.last_cmd, &cmd) {
                // replacing...
            } else if let (Cmd::SelfInsert(_, _), Cmd::SelfInsert(_, _)) = (&self.last_cmd, &cmd) {
                // inserting...
            } else {
                self.last_cmd = cmd.clone();
            }
        }
        Ok(cmd)
    }

    fn vi_cmd_motion<R: RawReader>(
        &mut self,
        rdr: &mut R,
        wrt: &mut dyn Refresher,
        key: KeyPress,
        n: RepeatCount,
    ) -> Result<Option<Movement>> {
        let mut mvt = rdr.next_key(false)?;
        if mvt == key {
            return Ok(Some(Movement::WholeLine));
        }
        let mut n = n;
        if let key_press!(Char(digit @ '1'..='9')) = mvt {
            // vi-arg-digit
            mvt = self.vi_arg_digit(rdr, wrt, digit)?;
            n = self.vi_num_args().saturating_mul(n);
        }
        Ok(match mvt {
            key_press!('$') => Some(Movement::EndOfLine),
            key_press!('0') => Some(Movement::BeginningOfLine),
            key_press!('^') => Some(Movement::ViFirstPrint),
            key_press!('b') => Some(Movement::BackwardWord(n, Word::Vi)),
            key_press!('B') => Some(Movement::BackwardWord(n, Word::Big)),
            key_press!('e') => Some(Movement::ForwardWord(n, At::AfterEnd, Word::Vi)),
            key_press!('E') => Some(Movement::ForwardWord(n, At::AfterEnd, Word::Big)),
            key_press!(Char(c)) if c == 'f' || c == 'F' || c == 't' || c == 'T' => {
                let cs = self.vi_char_search(rdr, c)?;
                match cs {
                    Some(cs) => Some(Movement::ViCharSearch(n, cs)),
                    None => None,
                }
            }
            key_press!(';') => match self.last_char_search {
                Some(cs) => Some(Movement::ViCharSearch(n, cs)),
                None => None,
            },
            key_press!(',') => match self.last_char_search {
                Some(ref cs) => Some(Movement::ViCharSearch(n, cs.opposite())),
                None => None,
            },
            key_press!('h') | key_press!(CTRL, 'H') | key_press!(Backspace) => {
                Some(Movement::BackwardChar(n))
            }
            key_press!('l') | key_press!(' ') => Some(Movement::ForwardChar(n)),
            key_press!('j') | key_press!('+') => Some(Movement::LineDown(n)),
            key_press!('k') | key_press!('-') => Some(Movement::LineUp(n)),
            key_press!('w') => {
                // 'cw' is 'ce'
                if key == key_press!('c') {
                    Some(Movement::ForwardWord(n, At::AfterEnd, Word::Vi))
                } else {
                    Some(Movement::ForwardWord(n, At::Start, Word::Vi))
                }
            }
            key_press!('W') => {
                // 'cW' is 'cE'
                if key == key_press!('c') {
                    Some(Movement::ForwardWord(n, At::AfterEnd, Word::Big))
                } else {
                    Some(Movement::ForwardWord(n, At::Start, Word::Big))
                }
            }
            _ => None,
        })
    }

    fn vi_char_search<R: RawReader>(
        &mut self,
        rdr: &mut R,
        cmd: char,
    ) -> Result<Option<CharSearch>> {
        let ch = rdr.next_key(false)?;
        Ok(match ch {
            key_press!(Char(ch)) => {
                let cs = match cmd {
                    'f' => CharSearch::Forward(ch),
                    't' => CharSearch::ForwardBefore(ch),
                    'F' => CharSearch::Backward(ch),
                    'T' => CharSearch::BackwardAfter(ch),
                    _ => unreachable!(),
                };
                self.last_char_search = Some(cs);
                Some(cs)
            }
            _ => None,
        })
    }

    fn common<R: RawReader>(
        &mut self,
        rdr: &mut R,
        key: KeyPress,
        n: RepeatCount,
        positive: bool,
    ) -> Result<Cmd> {
        Ok(match key {
            key_press!(Home) => Cmd::Move(Movement::BeginningOfLine),
            key_press!(Left) => {
                if positive {
                    Cmd::Move(Movement::BackwardChar(n))
                } else {
                    Cmd::Move(Movement::ForwardChar(n))
                }
            }
            key_press!(CTRL, 'C') => Cmd::Interrupt,
            key_press!(CTRL, 'D') => Cmd::EndOfFile,
            key_press!(Delete) => {
                if positive {
                    Cmd::Kill(Movement::ForwardChar(n))
                } else {
                    Cmd::Kill(Movement::BackwardChar(n))
                }
            }
            key_press!(End) => Cmd::Move(Movement::EndOfLine),
            key_press!(Right) => {
                if positive {
                    Cmd::Move(Movement::ForwardChar(n))
                } else {
                    Cmd::Move(Movement::BackwardChar(n))
                }
            }
            key_press!(CTRL, 'J') |
            key_press!(Enter) => Cmd::AcceptLine,
            key_press!(Down) => Cmd::LineDownOrNextHistory,
            key_press!(Up) => Cmd::LineUpOrPreviousHistory,
            key_press!(CTRL, 'R') => Cmd::ReverseSearchHistory,
            key_press!(CTRL, 'S') => Cmd::ForwardSearchHistory, // most terminals override Ctrl+S to suspend execution
            key_press!(CTRL, 'T') => Cmd::TransposeChars,
            key_press!(CTRL, 'U') => {
                if positive {
                    Cmd::Kill(Movement::BeginningOfLine)
                } else {
                    Cmd::Kill(Movement::EndOfLine)
                }
            },
            key_press!(CTRL, 'Q') | // most terminals override Ctrl+Q to resume execution
            key_press!(CTRL, 'V') => Cmd::QuotedInsert,
            key_press!(CTRL, 'W') => {
                if positive {
                    Cmd::Kill(Movement::BackwardWord(n, Word::Big))
                } else {
                    Cmd::Kill(Movement::ForwardWord(n, At::AfterEnd, Word::Big))
                }
            }
            key_press!(CTRL, 'Y') => {
                if positive {
                    Cmd::Yank(n, Anchor::Before)
                } else {
                    Cmd::Unknown // TODO Validate
                }
            }
            key_press!(CTRL, 'Z') => Cmd::Suspend,
            key_press!(CTRL, '_') => Cmd::Undo(n),
            key_press!(UnknownEscSeq) => Cmd::Noop,
            key_press!(BracketedPasteStart) => {
                let paste = rdr.read_pasted_text()?;
                Cmd::Insert(1, paste)
            },
            _ => Cmd::Unknown,
        })
    }

    fn num_args(&mut self) -> i16 {
        let num_args = match self.num_args {
            0 => 1,
            _ => self.num_args,
        };
        self.num_args = 0;
        num_args
    }

    #[allow(clippy::cast_sign_loss)]
    fn emacs_num_args(&mut self) -> (RepeatCount, bool) {
        let num_args = self.num_args();
        if num_args < 0 {
            if let (n, false) = num_args.overflowing_abs() {
                (n as RepeatCount, false)
            } else {
                (RepeatCount::max_value(), false)
            }
        } else {
            (num_args as RepeatCount, true)
        }
    }

    #[allow(clippy::cast_sign_loss)]
    fn vi_num_args(&mut self) -> RepeatCount {
        let num_args = self.num_args();
        if num_args < 0 {
            unreachable!()
        } else {
            num_args.abs() as RepeatCount
        }
    }
}
