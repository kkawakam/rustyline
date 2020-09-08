//! Bindings from keys to command for Emacs and Vi modes
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use log::debug;

use super::Result;
use crate::config::Config;
use crate::config::EditMode;
use crate::keys::{KeyCode as K, KeyEvent, Modifiers as M};
use crate::tty::{RawReader, Term, Terminal};

/// The number of times one command should be repeated.
pub type RepeatCount = usize;

/// Commands
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
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
    /// Insert text
    Insert(RepeatCount, String),
    /// Interrupt signal (Ctrl-C)
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
    /// No action
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
    /// Suspend signal (Ctrl-Z on unix platform)
    Suspend,
    /// transpose-chars
    TransposeChars,
    /// transpose-words
    TransposeWords(RepeatCount),
    /// undo
    Undo(RepeatCount),
    /// Unsupported / unexpected
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
    LineUpOrPreviousHistory(RepeatCount),
    /// moves cursor to the line below or switches to next history entry if
    /// the cursor is already on the last line
    LineDownOrNextHistory(RepeatCount),
    /// accepts the line when cursor is at the end of the text (non including
    /// trailing whitespace), inserts newline character otherwise
    AcceptOrInsertLine,
}

impl Cmd {
    /// Tells if current command should reset kill ring.
    pub fn should_reset_kill_ring(&self) -> bool {
        #[allow(clippy::match_same_arms)]
        match *self {
            Cmd::Kill(Movement::BackwardChar(_)) | Cmd::Kill(Movement::ForwardChar(_)) => true,
            Cmd::ClearScreen
            | Cmd::Kill(_)
            | Cmd::Replace(..)
            | Cmd::Noop
            | Cmd::Suspend
            | Cmd::Yank(..)
            | Cmd::YankPop => false,
            _ => true,
        }
    }

    fn is_repeatable_change(&self) -> bool {
        match *self {
            Cmd::Insert(..)
            | Cmd::Kill(_)
            | Cmd::ReplaceChar(..)
            | Cmd::Replace(..)
            | Cmd::SelfInsert(..)
            | Cmd::ViYankTo(_)
            | Cmd::Yank(..) => true,
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
    /// Start of word.
    Start,
    /// Before end of word.
    BeforeEnd,
    /// After end of word.
    AfterEnd,
}

/// Where to paste (relative to cursor position)
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum Anchor {
    /// After cursor
    After,
    /// Before cursor
    Before,
}

/// Vi character search
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum CharSearch {
    /// Forward search
    Forward(char),
    /// Forward search until
    ForwardBefore(char),
    /// Backward search
    Backward(char),
    /// Backward search until
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
#[non_exhaustive]
pub enum Movement {
    /// Whole current line (not really a movement but a range)
    WholeLine,
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
    /// Whole user input (not really a movement but a range)
    WholeBuffer,
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
    custom_bindings: Arc<RwLock<HashMap<KeyEvent, Cmd>>>,
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
    pub fn new(config: &Config, custom_bindings: Arc<RwLock<HashMap<KeyEvent, Cmd>>>) -> Self {
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
            EditMode::Emacs => {
                let key = rdr.next_key(single_esc_abort)?;
                self.emacs(rdr, wrt, key)
            }
            EditMode::Vi if self.input_mode != InputMode::Command => {
                let key = rdr.next_key(false)?;
                self.vi_insert(rdr, wrt, key)
            }
            EditMode::Vi => {
                let key = rdr.next_key(false)?;
                self.vi_command(rdr, wrt, key)
            }
        }
    }

    fn emacs_digit_argument<R: RawReader>(
        &mut self,
        rdr: &mut R,
        wrt: &mut dyn Refresher,
        digit: char,
    ) -> Result<KeyEvent> {
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
                (K::Char(digit @ '0'..='9'), m) if m == M::NONE || m == M::ALT => {
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
                (K::Char('-'), m) if m == M::NONE || m == M::ALT => {}
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
        mut key: KeyEvent,
    ) -> Result<Cmd> {
        if let (K::Char(digit @ '-'), M::ALT) = key {
            key = self.emacs_digit_argument(rdr, wrt, digit)?;
        } else if let (K::Char(digit @ '0'..='9'), M::ALT) = key {
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
            (K::Char(c), M::NONE) => {
                if positive {
                    Cmd::SelfInsert(n, c)
                } else {
                    Cmd::Unknown
                }
            }
            (K::Char('A'), M::CTRL) => Cmd::Move(Movement::BeginningOfLine),
            (K::Char('B'), M::CTRL) => {
                if positive {
                    Cmd::Move(Movement::BackwardChar(n))
                } else {
                    Cmd::Move(Movement::ForwardChar(n))
                }
            }
            (K::Char('E'), M::CTRL) => Cmd::Move(Movement::EndOfLine),
            (K::Char('F'), M::CTRL) => {
                if positive {
                    Cmd::Move(Movement::ForwardChar(n))
                } else {
                    Cmd::Move(Movement::BackwardChar(n))
                }
            }
            (K::Char('G'), M::CTRL) | (K::Esc, M::NONE) | (K::Char('\x07'), M::ALT) => Cmd::Abort,
            (K::Char('H'), M::CTRL) | (K::Backspace, M::NONE) => {
                if positive {
                    Cmd::Kill(Movement::BackwardChar(n))
                } else {
                    Cmd::Kill(Movement::ForwardChar(n))
                }
            }
            (K::BackTab, M::NONE) => Cmd::CompleteBackward,
            (K::Tab, M::NONE) => {
                if positive {
                    Cmd::Complete
                } else {
                    Cmd::CompleteBackward
                }
            }
            // Don't complete hints when the cursor is not at the end of a line
            (K::Right, M::NONE) if wrt.has_hint() && wrt.is_cursor_at_end() => Cmd::CompleteHint,
            (K::Char('K'), M::CTRL) => {
                if positive {
                    Cmd::Kill(Movement::EndOfLine)
                } else {
                    Cmd::Kill(Movement::BeginningOfLine)
                }
            }
            (K::Char('L'), M::CTRL) => Cmd::ClearScreen,
            (K::Char('N'), M::CTRL) => Cmd::NextHistory,
            (K::Char('P'), M::CTRL) => Cmd::PreviousHistory,
            (K::Char('X'), M::CTRL) => {
                let snd_key = rdr.next_key(true)?;
                match snd_key {
                    (K::Char('G'), M::CTRL) | (K::Esc, M::NONE) => Cmd::Abort,
                    (K::Char('U'), M::CTRL) => Cmd::Undo(n),
                    _ => Cmd::Unknown,
                }
            }
            (K::Char('\x08'), M::ALT) | (K::Char('\x7f'), M::ALT) => {
                if positive {
                    Cmd::Kill(Movement::BackwardWord(n, Word::Emacs))
                } else {
                    Cmd::Kill(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
                }
            }
            (K::Char('<'), M::ALT) => Cmd::BeginningOfHistory,
            (K::Char('>'), M::ALT) => Cmd::EndOfHistory,
            (K::Char('B'), M::ALT) | (K::Char('b'), M::ALT) => {
                if positive {
                    Cmd::Move(Movement::BackwardWord(n, Word::Emacs))
                } else {
                    Cmd::Move(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
                }
            }
            (K::Char('C'), M::ALT) | (K::Char('c'), M::ALT) => Cmd::CapitalizeWord,
            (K::Char('D'), M::ALT) | (K::Char('d'), M::ALT) => {
                if positive {
                    Cmd::Kill(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
                } else {
                    Cmd::Kill(Movement::BackwardWord(n, Word::Emacs))
                }
            }
            (K::Char('F'), M::ALT) | (K::Char('f'), M::ALT) => {
                if positive {
                    Cmd::Move(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
                } else {
                    Cmd::Move(Movement::BackwardWord(n, Word::Emacs))
                }
            }
            (K::Char('L'), M::ALT) | (K::Char('l'), M::ALT) => Cmd::DowncaseWord,
            (K::Char('T'), M::ALT) | (K::Char('t'), M::ALT) => Cmd::TransposeWords(n),
            (K::Char('U'), M::ALT) | (K::Char('u'), M::ALT) => Cmd::UpcaseWord,
            (K::Char('Y'), M::ALT) | (K::Char('y'), M::ALT) => Cmd::YankPop,
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
    ) -> Result<KeyEvent> {
        self.num_args = digit.to_digit(10).unwrap() as i16;
        loop {
            wrt.refresh_prompt_and_line(&format!("(arg: {}) ", self.num_args))?;
            let key = rdr.next_key(false)?;
            if let (K::Char(digit @ '0'..='9'), M::NONE) = key {
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

    fn vi_command<R: RawReader>(
        &mut self,
        rdr: &mut R,
        wrt: &mut dyn Refresher,
        mut key: KeyEvent,
    ) -> Result<Cmd> {
        if let (K::Char(digit @ '1'..='9'), M::NONE) = key {
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
            (K::Char('$'), M::NONE) | (K::End, M::NONE) => Cmd::Move(Movement::EndOfLine),
            (K::Char('.'), M::NONE) => {
                // vi-redo (repeat last command)
                if no_num_args {
                    self.last_cmd.redo(None, wrt)
                } else {
                    self.last_cmd.redo(Some(n), wrt)
                }
            }
            // TODO (K::Char('%'), M::NONE) => Cmd::???, Move to the corresponding opening/closing
            // bracket
            (K::Char('0'), M::NONE) => Cmd::Move(Movement::BeginningOfLine),
            (K::Char('^'), M::NONE) => Cmd::Move(Movement::ViFirstPrint),
            (K::Char('a'), M::NONE) => {
                // vi-append-mode
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Move(Movement::ForwardChar(n))
            }
            (K::Char('A'), M::NONE) => {
                // vi-append-eol
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Move(Movement::EndOfLine)
            }
            (K::Char('b'), M::NONE) => Cmd::Move(Movement::BackwardWord(n, Word::Vi)), /* vi-prev-word */
            (K::Char('B'), M::NONE) => Cmd::Move(Movement::BackwardWord(n, Word::Big)),
            (K::Char('c'), M::NONE) => {
                self.input_mode = InputMode::Insert;
                match self.vi_cmd_motion(rdr, wrt, key, n)? {
                    Some(mvt) => Cmd::Replace(mvt, None),
                    None => Cmd::Unknown,
                }
            }
            (K::Char('C'), M::NONE) => {
                self.input_mode = InputMode::Insert;
                Cmd::Replace(Movement::EndOfLine, None)
            }
            (K::Char('d'), M::NONE) => match self.vi_cmd_motion(rdr, wrt, key, n)? {
                Some(mvt) => Cmd::Kill(mvt),
                None => Cmd::Unknown,
            },
            (K::Char('D'), M::NONE) | (K::Char('K'), M::CTRL) => Cmd::Kill(Movement::EndOfLine),
            (K::Char('e'), M::NONE) => Cmd::Move(Movement::ForwardWord(n, At::BeforeEnd, Word::Vi)),
            (K::Char('E'), M::NONE) => {
                Cmd::Move(Movement::ForwardWord(n, At::BeforeEnd, Word::Big))
            }
            (K::Char('i'), M::NONE) => {
                // vi-insertion-mode
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Noop
            }
            (K::Char('I'), M::NONE) => {
                // vi-insert-beg
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Move(Movement::BeginningOfLine)
            }
            (K::Char(c), M::NONE) if c == 'f' || c == 'F' || c == 't' || c == 'T' => {
                // vi-char-search
                let cs = self.vi_char_search(rdr, c)?;
                match cs {
                    Some(cs) => Cmd::Move(Movement::ViCharSearch(n, cs)),
                    None => Cmd::Unknown,
                }
            }
            (K::Char(';'), M::NONE) => match self.last_char_search {
                Some(cs) => Cmd::Move(Movement::ViCharSearch(n, cs)),
                None => Cmd::Noop,
            },
            (K::Char(','), M::NONE) => match self.last_char_search {
                Some(ref cs) => Cmd::Move(Movement::ViCharSearch(n, cs.opposite())),
                None => Cmd::Noop,
            },
            // TODO (K::Char('G'), M::NONE) => Cmd::???, Move to the history line n
            (K::Char('p'), M::NONE) => Cmd::Yank(n, Anchor::After), // vi-put
            (K::Char('P'), M::NONE) => Cmd::Yank(n, Anchor::Before), // vi-put
            (K::Char('r'), M::NONE) => {
                // vi-replace-char:
                let ch = rdr.next_key(false)?;
                match ch {
                    (K::Char(c), M::NONE) => Cmd::ReplaceChar(n, c),
                    (K::Esc, M::NONE) => Cmd::Noop,
                    _ => Cmd::Unknown,
                }
            }
            (K::Char('R'), M::NONE) => {
                //  vi-replace-mode (overwrite-mode)
                self.input_mode = InputMode::Replace;
                Cmd::Replace(Movement::ForwardChar(0), None)
            }
            (K::Char('s'), M::NONE) => {
                // vi-substitute-char:
                self.input_mode = InputMode::Insert;
                Cmd::Replace(Movement::ForwardChar(n), None)
            }
            (K::Char('S'), M::NONE) => {
                // vi-substitute-line:
                self.input_mode = InputMode::Insert;
                Cmd::Replace(Movement::WholeLine, None)
            }
            (K::Char('u'), M::NONE) => Cmd::Undo(n),
            // (K::Char('U'), M::NONE) => Cmd::???, // revert-line
            (K::Char('w'), M::NONE) => Cmd::Move(Movement::ForwardWord(n, At::Start, Word::Vi)), /* vi-next-word */
            (K::Char('W'), M::NONE) => Cmd::Move(Movement::ForwardWord(n, At::Start, Word::Big)), /* vi-next-word */
            // TODO move backward if eol
            (K::Char('x'), M::NONE) => Cmd::Kill(Movement::ForwardChar(n)), // vi-delete
            (K::Char('X'), M::NONE) => Cmd::Kill(Movement::BackwardChar(n)), // vi-rubout
            (K::Char('y'), M::NONE) => match self.vi_cmd_motion(rdr, wrt, key, n)? {
                Some(mvt) => Cmd::ViYankTo(mvt),
                None => Cmd::Unknown,
            },
            // (K::Char('Y'), M::NONE) => Cmd::???, // vi-yank-to
            (K::Char('h'), M::NONE) | (K::Char('H'), M::CTRL) | (K::Backspace, M::NONE) => {
                Cmd::Move(Movement::BackwardChar(n))
            }
            (K::Char('G'), M::CTRL) => Cmd::Abort,
            (K::Char('l'), M::NONE) | (K::Char(' '), M::NONE) => {
                Cmd::Move(Movement::ForwardChar(n))
            }
            (K::Char('L'), M::CTRL) => Cmd::ClearScreen,
            (K::Char('+'), M::NONE) | (K::Char('j'), M::NONE) => Cmd::LineDownOrNextHistory(n),
            // TODO: move to the start of the line.
            (K::Char('N'), M::CTRL) => Cmd::NextHistory,
            (K::Char('-'), M::NONE) | (K::Char('k'), M::NONE) => Cmd::LineUpOrPreviousHistory(n),
            // TODO: move to the start of the line.
            (K::Char('P'), M::CTRL) => Cmd::PreviousHistory,
            (K::Char('R'), M::CTRL) => {
                self.input_mode = InputMode::Insert; // TODO Validate
                Cmd::ReverseSearchHistory
            }
            (K::Char('S'), M::CTRL) => {
                self.input_mode = InputMode::Insert; // TODO Validate
                Cmd::ForwardSearchHistory
            }
            (K::Esc, M::NONE) => Cmd::Noop,
            _ => self.common(rdr, key, n, true)?,
        };
        debug!(target: "rustyline", "Vi command: {:?}", cmd);
        if cmd.is_repeatable_change() {
            self.last_cmd = cmd.clone();
        }
        Ok(cmd)
    }

    fn vi_insert<R: RawReader>(
        &mut self,
        rdr: &mut R,
        wrt: &mut dyn Refresher,
        key: KeyEvent,
    ) -> Result<Cmd> {
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
            (K::Char(c), M::NONE) => {
                if self.input_mode == InputMode::Replace {
                    Cmd::Overwrite(c)
                } else {
                    Cmd::SelfInsert(1, c)
                }
            }
            (K::Char('H'), M::CTRL) | (K::Backspace, M::NONE) => {
                Cmd::Kill(Movement::BackwardChar(1))
            }
            (K::BackTab, M::NONE) => Cmd::CompleteBackward,
            (K::Tab, M::NONE) => Cmd::Complete,
            // Don't complete hints when the cursor is not at the end of a line
            (K::Right, M::NONE) if wrt.has_hint() && wrt.is_cursor_at_end() => Cmd::CompleteHint,
            (K::Char(k), M::ALT) => {
                debug!(target: "rustyline", "Vi fast command mode: {}", k);
                self.input_mode = InputMode::Command;
                wrt.done_inserting();

                self.vi_command(rdr, wrt, (K::Char(k), M::NONE))?
            }
            (K::Esc, M::NONE) => {
                // vi-movement-mode/vi-command-mode
                self.input_mode = InputMode::Command;
                wrt.done_inserting();
                Cmd::Move(Movement::BackwardChar(1))
            }
            _ => self.common(rdr, key, 1, true)?,
        };
        debug!(target: "rustyline", "Vi insert: {:?}", cmd);
        if cmd.is_repeatable_change() {
            if let (Cmd::Replace(..), Cmd::SelfInsert(..)) = (&self.last_cmd, &cmd) {
                // replacing...
            } else if let (Cmd::SelfInsert(..), Cmd::SelfInsert(..)) = (&self.last_cmd, &cmd) {
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
        key: KeyEvent,
        n: RepeatCount,
    ) -> Result<Option<Movement>> {
        let mut mvt = rdr.next_key(false)?;
        if mvt == key {
            return Ok(Some(Movement::WholeLine));
        }
        let mut n = n;
        if let (K::Char(digit @ '1'..='9'), M::NONE) = mvt {
            // vi-arg-digit
            mvt = self.vi_arg_digit(rdr, wrt, digit)?;
            n = self.vi_num_args().saturating_mul(n);
        }
        Ok(match mvt {
            (K::Char('$'), M::NONE) => Some(Movement::EndOfLine),
            (K::Char('0'), M::NONE) => Some(Movement::BeginningOfLine),
            (K::Char('^'), M::NONE) => Some(Movement::ViFirstPrint),
            (K::Char('b'), M::NONE) => Some(Movement::BackwardWord(n, Word::Vi)),
            (K::Char('B'), M::NONE) => Some(Movement::BackwardWord(n, Word::Big)),
            (K::Char('e'), M::NONE) => Some(Movement::ForwardWord(n, At::AfterEnd, Word::Vi)),
            (K::Char('E'), M::NONE) => Some(Movement::ForwardWord(n, At::AfterEnd, Word::Big)),
            (K::Char(c), M::NONE) if c == 'f' || c == 'F' || c == 't' || c == 'T' => {
                let cs = self.vi_char_search(rdr, c)?;
                match cs {
                    Some(cs) => Some(Movement::ViCharSearch(n, cs)),
                    None => None,
                }
            }
            (K::Char(';'), M::NONE) => match self.last_char_search {
                Some(cs) => Some(Movement::ViCharSearch(n, cs)),
                None => None,
            },
            (K::Char(','), M::NONE) => match self.last_char_search {
                Some(ref cs) => Some(Movement::ViCharSearch(n, cs.opposite())),
                None => None,
            },
            (K::Char('h'), M::NONE) | (K::Char('H'), M::CTRL) | (K::Backspace, M::NONE) => {
                Some(Movement::BackwardChar(n))
            }
            (K::Char('l'), M::NONE) | (K::Char(' '), M::NONE) => Some(Movement::ForwardChar(n)),
            (K::Char('j'), M::NONE) | (K::Char('+'), M::NONE) => Some(Movement::LineDown(n)),
            (K::Char('k'), M::NONE) | (K::Char('-'), M::NONE) => Some(Movement::LineUp(n)),
            (K::Char('w'), M::NONE) => {
                // 'cw' is 'ce'
                if key == (K::Char('c'), M::NONE) {
                    Some(Movement::ForwardWord(n, At::AfterEnd, Word::Vi))
                } else {
                    Some(Movement::ForwardWord(n, At::Start, Word::Vi))
                }
            }
            (K::Char('W'), M::NONE) => {
                // 'cW' is 'cE'
                if key == (K::Char('c'), M::NONE) {
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
            (K::Char(ch), M::NONE) => {
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
        key: KeyEvent,
        n: RepeatCount,
        positive: bool,
    ) -> Result<Cmd> {
        Ok(match key {
            (K::Home, M::NONE) => Cmd::Move(Movement::BeginningOfLine),
            (K::Left, M::NONE) => {
                if positive {
                    Cmd::Move(Movement::BackwardChar(n))
                } else {
                    Cmd::Move(Movement::ForwardChar(n))
                }
            }
            (K::Char('C'), M::CTRL) => Cmd::Interrupt,
            (K::Char('D'), M::CTRL) => Cmd::EndOfFile,
            (K::Delete, M::NONE) => {
                if positive {
                    Cmd::Kill(Movement::ForwardChar(n))
                } else {
                    Cmd::Kill(Movement::BackwardChar(n))
                }
            }
            (K::End, M::NONE) => Cmd::Move(Movement::EndOfLine),
            (K::Right, M::NONE) => {
                if positive {
                    Cmd::Move(Movement::ForwardChar(n))
                } else {
                    Cmd::Move(Movement::BackwardChar(n))
                }
            }
            (K::Char('J'), M::CTRL) |
            (K::Enter, M::NONE) => Cmd::AcceptLine,
            (K::Down, M::NONE) => Cmd::LineDownOrNextHistory(1),
            (K::Up, M::NONE) => Cmd::LineUpOrPreviousHistory(1),
            (K::Char('R'), M::CTRL) => Cmd::ReverseSearchHistory,
            (K::Char('S'), M::CTRL) => Cmd::ForwardSearchHistory, // most terminals override Ctrl+S to suspend execution
            (K::Char('T'), M::CTRL) => Cmd::TransposeChars,
            (K::Char('U'), M::CTRL) => {
                if positive {
                    Cmd::Kill(Movement::BeginningOfLine)
                } else {
                    Cmd::Kill(Movement::EndOfLine)
                }
            },
            (K::Char('Q'), M::CTRL) | // most terminals override Ctrl+Q to resume execution
            (K::Char('V'), M::CTRL) => Cmd::QuotedInsert,
            (K::Char('W'), M::CTRL) => {
                if positive {
                    Cmd::Kill(Movement::BackwardWord(n, Word::Big))
                } else {
                    Cmd::Kill(Movement::ForwardWord(n, At::AfterEnd, Word::Big))
                }
            }
            (K::Char('Y'), M::CTRL) => {
                if positive {
                    Cmd::Yank(n, Anchor::Before)
                } else {
                    Cmd::Unknown // TODO Validate
                }
            }
            (K::Char('Z'), M::CTRL) => Cmd::Suspend,
            (K::Char('_'), M::CTRL) => Cmd::Undo(n),
            (K::UnknownEscSeq, M::NONE) => Cmd::Noop,
            (K::BracketedPasteStart, M::NONE) => {
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
