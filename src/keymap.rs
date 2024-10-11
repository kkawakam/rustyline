//! Bindings from keys to command for Emacs and Vi modes
use log::debug;

use super::Result;
use crate::highlight::CmdKind;
use crate::keys::{KeyCode as K, KeyEvent, KeyEvent as E, Modifiers as M};
use crate::tty::{self, RawReader, Term, Terminal};
use crate::{Config, EditMode};
#[cfg(feature = "custom-bindings")]
use crate::{Event, EventContext, EventHandler};

/// The number of times one command should be repeated.
pub type RepeatCount = usize;

/// Commands
#[derive(Debug, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum Cmd {
    /// abort
    Abort, // Miscellaneous Command
    /// accept-line
    ///
    /// See also `AcceptOrInsertLine`
    AcceptLine,
    /// beginning-of-history
    BeginningOfHistory,
    /// capitalize-word
    CapitalizeWord,
    /// clear-screen
    ClearScreen,
    /// Paste from the clipboard
    #[cfg(windows)]
    PasteFromClipboard,
    /// complete
    Complete,
    /// complete-backward
    CompleteBackward,
    /// complete-hint
    CompleteHint,
    /// Dedent current line
    Dedent(Movement),
    /// downcase-word
    DowncaseWord,
    /// vi-eof-maybe
    EndOfFile,
    /// end-of-history
    EndOfHistory,
    /// forward-search-history (incremental search)
    ForwardSearchHistory,
    /// history-search-backward (common prefix search)
    HistorySearchBackward,
    /// history-search-forward (common prefix search)
    HistorySearchForward,
    /// Indent current line
    Indent(Movement),
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
    /// repaint
    Repaint,
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
    /// reverse-search-history (incremental search)
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
    /// Inserts a newline
    Newline,
    /// Either accepts or inserts a newline
    ///
    /// Always inserts newline if input is non-valid. Can also insert newline
    /// if cursor is in the middle of the text
    ///
    /// If you support multi-line input:
    /// * Use `accept_in_the_middle: true` for mostly single-line cases, for
    ///   example command-line.
    /// * Use `accept_in_the_middle: false` for mostly multi-line cases, for
    ///   example SQL or JSON input.
    AcceptOrInsertLine {
        /// Whether this commands accepts input if the cursor not at the end
        /// of the current input
        accept_in_the_middle: bool,
    },
}

impl Cmd {
    /// Tells if current command should reset kill ring.
    #[must_use]
    pub const fn should_reset_kill_ring(&self) -> bool {
        match *self {
            Self::Kill(Movement::BackwardChar(_) | Movement::ForwardChar(_)) => true,
            Self::ClearScreen
            | Self::Kill(_)
            | Self::Replace(..)
            | Self::Noop
            | Self::Suspend
            | Self::Yank(..)
            | Self::YankPop => false,
            _ => true,
        }
    }

    const fn is_repeatable_change(&self) -> bool {
        matches!(
            *self,
            Self::Dedent(..)
                | Self::Indent(..)
                | Self::Insert(..)
                | Self::Kill(_)
                | Self::ReplaceChar(..)
                | Self::Replace(..)
                | Self::SelfInsert(..)
                | Self::ViYankTo(_)
                | Self::Yank(..) // Cmd::TransposeChars | TODO Validate
        )
    }

    const fn is_repeatable(&self) -> bool {
        match *self {
            Self::Move(_) => true,
            _ => self.is_repeatable_change(),
        }
    }

    // Replay this command with a possible different `RepeatCount`.
    fn redo(&self, new: Option<RepeatCount>, wrt: &dyn Refresher) -> Self {
        match *self {
            Self::Dedent(ref mvt) => Self::Dedent(mvt.redo(new)),
            Self::Indent(ref mvt) => Self::Indent(mvt.redo(new)),
            Self::Insert(previous, ref text) => {
                Self::Insert(repeat_count(previous, new), text.clone())
            }
            Self::Kill(ref mvt) => Self::Kill(mvt.redo(new)),
            Self::Move(ref mvt) => Self::Move(mvt.redo(new)),
            Self::ReplaceChar(previous, c) => Self::ReplaceChar(repeat_count(previous, new), c),
            Self::Replace(ref mvt, ref text) => {
                if text.is_none() {
                    let last_insert = wrt.last_insert();
                    if let Movement::ForwardChar(0) = mvt {
                        Self::Replace(
                            Movement::ForwardChar(last_insert.as_ref().map_or(0, String::len)),
                            last_insert,
                        )
                    } else {
                        Self::Replace(mvt.redo(new), last_insert)
                    }
                } else {
                    Self::Replace(mvt.redo(new), text.clone())
                }
            }
            Self::SelfInsert(previous, c) => {
                // consecutive char inserts are repeatable not only the last one...
                if let Some(text) = wrt.last_insert() {
                    Self::Insert(repeat_count(previous, new), text)
                } else {
                    Self::SelfInsert(repeat_count(previous, new), c)
                }
            }
            // Cmd::TransposeChars => Cmd::TransposeChars,
            Self::ViYankTo(ref mvt) => Self::ViYankTo(mvt.redo(new)),
            Self::Yank(previous, anchor) => Self::Yank(repeat_count(previous, new), anchor),
            _ => unreachable!(),
        }
    }
}

const fn repeat_count(previous: RepeatCount, new: Option<RepeatCount>) -> RepeatCount {
    match new {
        Some(n) => n,
        None => previous,
    }
}

/// Different word definitions
#[derive(Debug, Clone, Eq, PartialEq, Copy)]
pub enum Word {
    /// non-blanks characters
    Big,
    /// alphanumeric characters
    Emacs,
    /// alphanumeric (and '_') characters
    Vi,
}

/// Where to move with respect to word boundary
#[derive(Debug, Clone, Eq, PartialEq, Copy)]
pub enum At {
    /// Start of word.
    Start,
    /// Before end of word.
    BeforeEnd,
    /// After end of word.
    AfterEnd,
}

/// Where to paste (relative to cursor position)
#[derive(Debug, Clone, Eq, PartialEq, Copy)]
pub enum Anchor {
    /// After cursor
    After,
    /// Before cursor
    Before,
}

/// character search
#[derive(Debug, Clone, Eq, PartialEq, Copy)]
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
    const fn opposite(self) -> Self {
        match self {
            Self::Forward(c) => Self::Backward(c),
            Self::ForwardBefore(c) => Self::BackwardAfter(c),
            Self::Backward(c) => Self::Forward(c),
            Self::BackwardAfter(c) => Self::ForwardBefore(c),
        }
    }
}

/// Where to move
#[derive(Debug, Clone, Eq, PartialEq)]
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
    /// character-search, character-search-backward, vi-char-search
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
    const fn redo(&self, new: Option<RepeatCount>) -> Self {
        match *self {
            Self::WholeLine => Self::WholeLine,
            Self::BeginningOfLine => Self::BeginningOfLine,
            Self::ViFirstPrint => Self::ViFirstPrint,
            Self::EndOfLine => Self::EndOfLine,
            Self::BackwardWord(previous, word) => {
                Self::BackwardWord(repeat_count(previous, new), word)
            }
            Self::ForwardWord(previous, at, word) => {
                Self::ForwardWord(repeat_count(previous, new), at, word)
            }
            Self::ViCharSearch(previous, char_search) => {
                Self::ViCharSearch(repeat_count(previous, new), char_search)
            }
            Self::BackwardChar(previous) => Self::BackwardChar(repeat_count(previous, new)),
            Self::ForwardChar(previous) => Self::ForwardChar(repeat_count(previous, new)),
            Self::LineUp(previous) => Self::LineUp(repeat_count(previous, new)),
            Self::LineDown(previous) => Self::LineDown(repeat_count(previous, new)),
            Self::WholeBuffer => Self::WholeBuffer,
            Self::BeginningOfBuffer => Self::BeginningOfBuffer,
            Self::EndOfBuffer => Self::EndOfBuffer,
        }
    }
}

/// Vi input modes
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum InputMode {
    /// Vi Command/Alternate
    Command,
    /// Insert/Input mode
    Insert,
    /// Overwrite mode
    Replace,
}

/// Transform key(s) to commands based on current input mode
pub struct InputState<'b> {
    pub(crate) mode: EditMode,
    #[cfg_attr(not(feature = "custom-bindings"), expect(dead_code))]
    custom_bindings: &'b Bindings,
    pub(crate) input_mode: InputMode, // vi only ?
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

impl Invoke for &str {
    fn input(&self) -> &str {
        self
    }
}

pub trait Refresher {
    /// Rewrite the currently edited line accordingly to the buffer content,
    /// cursor position, and number of columns of the terminal.
    fn refresh_line(&mut self) -> Result<()>;
    /// Same as [`refresh_line`] with a specific message instead of hint
    fn refresh_line_with_msg(&mut self, msg: Option<&str>, kind: CmdKind) -> Result<()>;
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
    /// Returns the hint text that is shown after the current cursor position.
    #[cfg_attr(not(feature = "custom-bindings"), expect(dead_code))]
    fn hint_text(&self) -> Option<&str>;
    /// currently edited line
    fn line(&self) -> &str;
    /// Current cursor position (byte position)
    #[cfg_attr(not(feature = "custom-bindings"), expect(dead_code))]
    fn pos(&self) -> usize;
    /// Display `msg` above currently edited line.
    fn external_print(&mut self, msg: String) -> Result<()>;
}

impl<'b> InputState<'b> {
    pub fn new(config: &Config, custom_bindings: &'b Bindings) -> Self {
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
        ignore_external_print: bool,
    ) -> Result<Cmd> {
        let single_esc_abort = self.single_esc_abort(single_esc_abort);
        let key;
        if ignore_external_print {
            key = rdr.next_key(single_esc_abort)?;
        } else {
            loop {
                let event = rdr.wait_for_input(single_esc_abort)?;
                match event {
                    tty::Event::KeyPress(k) => {
                        key = k;
                        break;
                    }
                    tty::Event::ExternalPrint(msg) => {
                        wrt.external_print(msg)?;
                    }
                }
            }
        }
        match self.mode {
            EditMode::Emacs => self.emacs(rdr, wrt, key),
            EditMode::Vi if self.input_mode != InputMode::Command => self.vi_insert(rdr, wrt, key),
            EditMode::Vi => self.vi_command(rdr, wrt, key),
        }
    }

    fn single_esc_abort(&self, single_esc_abort: bool) -> bool {
        match self.mode {
            EditMode::Emacs => single_esc_abort,
            EditMode::Vi => false,
        }
    }

    /// Terminal peculiar binding
    fn term_binding<R: RawReader>(rdr: &R, wrt: &dyn Refresher, key: &KeyEvent) -> Option<Cmd> {
        let cmd = rdr.find_binding(key);
        if cmd == Some(Cmd::EndOfFile) && !wrt.line().is_empty() {
            None // ReadlineError::Eof only if line is empty
        } else {
            cmd
        }
    }

    fn emacs_digit_argument<R: RawReader>(
        &mut self,
        rdr: &mut R,
        wrt: &mut dyn Refresher,
        digit: char,
    ) -> Result<KeyEvent> {
        #[expect(clippy::cast_possible_truncation)]
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
            #[expect(clippy::cast_possible_truncation)]
            match key {
                E(K::Char(digit @ '0'..='9'), m) if m == M::NONE || m == M::ALT => {
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
                E(K::Char('-'), m) if m == M::NONE || m == M::ALT => {}
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
        if let E(K::Char(digit @ '-'), M::ALT) = key {
            key = self.emacs_digit_argument(rdr, wrt, digit)?;
        } else if let E(K::Char(digit @ '0'..='9'), M::ALT) = key {
            key = self.emacs_digit_argument(rdr, wrt, digit)?;
        }
        let (n, positive) = self.emacs_num_args(); // consume them in all cases

        let mut evt = key.into();
        if let Some(cmd) = self.custom_binding(wrt, &evt, n, positive) {
            return Ok(if cmd.is_repeatable() {
                cmd.redo(Some(n), wrt)
            } else {
                cmd
            });
        } else if let Some(cmd) = InputState::term_binding(rdr, wrt, &key) {
            return Ok(cmd);
        }
        let cmd = match key {
            E(K::Char(c), M::NONE) => {
                if positive {
                    Cmd::SelfInsert(n, c)
                } else {
                    Cmd::Unknown
                }
            }
            E(K::Char('A'), M::CTRL) => Cmd::Move(Movement::BeginningOfLine),
            E(K::Char('B'), M::CTRL) => Cmd::Move(if positive {
                Movement::BackwardChar(n)
            } else {
                Movement::ForwardChar(n)
            }),
            E(K::Char('E'), M::CTRL) => Cmd::Move(Movement::EndOfLine),
            E(K::Char('F'), M::CTRL) => Cmd::Move(if positive {
                Movement::ForwardChar(n)
            } else {
                Movement::BackwardChar(n)
            }),
            E(K::Char('G'), M::CTRL | M::CTRL_ALT) | E::ESC => Cmd::Abort,
            E(K::Char('H'), M::CTRL) | E::BACKSPACE => Cmd::Kill(if positive {
                Movement::BackwardChar(n)
            } else {
                Movement::ForwardChar(n)
            }),
            E(K::BackTab, M::NONE) => Cmd::CompleteBackward,
            E(K::Char('I'), M::CTRL) | E(K::Tab, M::NONE) => {
                if positive {
                    Cmd::Complete
                } else {
                    Cmd::CompleteBackward
                }
            }
            // Don't complete hints when the cursor is not at the end of a line
            E(K::Right, M::NONE) if wrt.has_hint() && wrt.is_cursor_at_end() => Cmd::CompleteHint,
            E(K::Char('K'), M::CTRL) => Cmd::Kill(if positive {
                Movement::EndOfLine
            } else {
                Movement::BeginningOfLine
            }),
            E(K::Char('L'), M::CTRL) => Cmd::ClearScreen,
            E(K::Char('N'), M::CTRL) => Cmd::NextHistory,
            E(K::Char('P'), M::CTRL) => Cmd::PreviousHistory,
            E(K::Char('X'), M::CTRL) => {
                if let Some(cmd) = self.custom_seq_binding(rdr, wrt, &mut evt, n, positive)? {
                    cmd
                } else {
                    let snd_key = match evt {
                        // we may have already read the second key in custom_seq_binding
                        #[allow(clippy::out_of_bounds_indexing)]
                        Event::KeySeq(ref key_seq) if key_seq.len() > 1 => key_seq[1],
                        _ => rdr.next_key(true)?,
                    };
                    match snd_key {
                        E(K::Char('G'), M::CTRL) | E::ESC => Cmd::Abort,
                        E(K::Char('U'), M::CTRL) => Cmd::Undo(n),
                        E(K::Backspace, M::NONE) => Cmd::Kill(if positive {
                            Movement::BeginningOfLine
                        } else {
                            Movement::EndOfLine
                        }),
                        _ => Cmd::Unknown,
                    }
                }
            }
            // character-search, character-search-backward
            E(K::Char(']'), m @ (M::CTRL | M::CTRL_ALT)) => {
                let ch = rdr.next_key(false)?;
                match ch {
                    E(K::Char(ch), M::NONE) => Cmd::Move(Movement::ViCharSearch(
                        n,
                        if positive {
                            if m.contains(M::ALT) {
                                CharSearch::Backward(ch)
                            } else {
                                CharSearch::ForwardBefore(ch)
                            }
                        } else if m.contains(M::ALT) {
                            CharSearch::ForwardBefore(ch)
                        } else {
                            CharSearch::Backward(ch)
                        },
                    )),
                    _ => Cmd::Unknown,
                }
            }
            E(K::Backspace, M::ALT) => Cmd::Kill(if positive {
                Movement::BackwardWord(n, Word::Emacs)
            } else {
                Movement::ForwardWord(n, At::AfterEnd, Word::Emacs)
            }),
            E(K::Char('<'), M::ALT) => Cmd::BeginningOfHistory,
            E(K::Char('>'), M::ALT) => Cmd::EndOfHistory,
            E(K::Char('B' | 'b') | K::Left, M::ALT) | E(K::Left, M::CTRL) => {
                Cmd::Move(if positive {
                    Movement::BackwardWord(n, Word::Emacs)
                } else {
                    Movement::ForwardWord(n, At::AfterEnd, Word::Emacs)
                })
            }
            E(K::Char('C' | 'c'), M::ALT) => Cmd::CapitalizeWord,
            E(K::Char('D' | 'd'), M::ALT) => Cmd::Kill(if positive {
                Movement::ForwardWord(n, At::AfterEnd, Word::Emacs)
            } else {
                Movement::BackwardWord(n, Word::Emacs)
            }),
            E(K::Char('F' | 'f') | K::Right, M::ALT) | E(K::Right, M::CTRL) => {
                Cmd::Move(if positive {
                    Movement::ForwardWord(n, At::AfterEnd, Word::Emacs)
                } else {
                    Movement::BackwardWord(n, Word::Emacs)
                })
            }
            E(K::Char('L' | 'l'), M::ALT) => Cmd::DowncaseWord,
            E(K::Char('T' | 't'), M::ALT) => Cmd::TransposeWords(n),
            // TODO ESC-R (r): Undo all changes made to this line.
            E(K::Char('U' | 'u'), M::ALT) => Cmd::UpcaseWord,
            E(K::Char('Y' | 'y'), M::ALT) => Cmd::YankPop,
            _ => self.common(rdr, wrt, evt, key, n, positive)?,
        };
        debug!(target: "rustyline", "Emacs command: {:?}", cmd);
        Ok(cmd)
    }

    #[expect(clippy::cast_possible_truncation)]
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
            if let E(K::Char(digit @ '0'..='9'), M::NONE) = key {
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
        if let E(K::Char(digit @ '1'..='9'), M::NONE) = key {
            key = self.vi_arg_digit(rdr, wrt, digit)?;
        }
        let no_num_args = self.num_args == 0;
        let n = self.vi_num_args(); // consume them in all cases
        let evt = key.into();
        if let Some(cmd) = self.custom_binding(wrt, &evt, n, true) {
            return Ok(if cmd.is_repeatable() {
                if no_num_args {
                    cmd.redo(None, wrt)
                } else {
                    cmd.redo(Some(n), wrt)
                }
            } else {
                cmd
            });
        } else if let Some(cmd) = InputState::term_binding(rdr, wrt, &key) {
            return Ok(cmd);
        }
        let cmd = match key {
            E(K::Char('$') | K::End, M::NONE) => Cmd::Move(Movement::EndOfLine),
            E(K::Char('.'), M::NONE) => {
                // vi-redo (repeat last command)
                if !self.last_cmd.is_repeatable() {
                    Cmd::Noop
                } else if no_num_args {
                    self.last_cmd.redo(None, wrt)
                } else {
                    self.last_cmd.redo(Some(n), wrt)
                }
            }
            // TODO E(K::Char('%'), M::NONE) => Cmd::???, Move to the corresponding opening/closing
            // bracket
            E(K::Char('0'), M::NONE) => Cmd::Move(Movement::BeginningOfLine),
            E(K::Char('^'), M::NONE) => Cmd::Move(Movement::ViFirstPrint),
            E(K::Char('a'), M::NONE) => {
                // vi-append-mode
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Move(Movement::ForwardChar(n))
            }
            E(K::Char('A'), M::NONE) => {
                // vi-append-eol
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Move(Movement::EndOfLine)
            }
            E(K::Char('b'), M::NONE) => Cmd::Move(Movement::BackwardWord(n, Word::Vi)), /* vi-prev-word */
            E(K::Char('B'), M::NONE) => Cmd::Move(Movement::BackwardWord(n, Word::Big)),
            E(K::Char('c'), M::NONE) => {
                self.input_mode = InputMode::Insert;
                match self.vi_cmd_motion(rdr, wrt, key, n)? {
                    Some(mvt) => Cmd::Replace(mvt, None),
                    None => Cmd::Unknown,
                }
            }
            E(K::Char('C'), M::NONE) => {
                self.input_mode = InputMode::Insert;
                Cmd::Replace(Movement::EndOfLine, None)
            }
            E(K::Char('d'), M::NONE) => match self.vi_cmd_motion(rdr, wrt, key, n)? {
                Some(mvt) => Cmd::Kill(mvt),
                None => Cmd::Unknown,
            },
            E(K::Char('D'), M::NONE) | E(K::Char('K'), M::CTRL) => Cmd::Kill(Movement::EndOfLine),
            E(K::Char('e'), M::NONE) => {
                Cmd::Move(Movement::ForwardWord(n, At::BeforeEnd, Word::Vi))
            }
            E(K::Char('E'), M::NONE) => {
                Cmd::Move(Movement::ForwardWord(n, At::BeforeEnd, Word::Big))
            }
            E(K::Char('i'), M::NONE) => {
                // vi-insertion-mode
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Noop
            }
            E(K::Char('I'), M::NONE) => {
                // vi-insert-beg
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Move(Movement::BeginningOfLine)
            }
            E(K::Char(c), M::NONE) if c == 'f' || c == 'F' || c == 't' || c == 'T' => {
                // vi-char-search
                let cs = self.vi_char_search(rdr, c)?;
                match cs {
                    Some(cs) => Cmd::Move(Movement::ViCharSearch(n, cs)),
                    None => Cmd::Unknown,
                }
            }
            E(K::Char(';'), M::NONE) => match self.last_char_search {
                Some(cs) => Cmd::Move(Movement::ViCharSearch(n, cs)),
                None => Cmd::Noop,
            },
            E(K::Char(','), M::NONE) => match self.last_char_search {
                Some(ref cs) => Cmd::Move(Movement::ViCharSearch(n, cs.opposite())),
                None => Cmd::Noop,
            },
            // TODO E(K::Char('G'), M::NONE) => Cmd::???, Move to the history line n
            E(K::Char('p'), M::NONE) => Cmd::Yank(n, Anchor::After), // vi-put
            E(K::Char('P'), M::NONE) => Cmd::Yank(n, Anchor::Before), // vi-put
            E(K::Char('r'), M::NONE) => {
                // vi-replace-char:
                let ch = rdr.next_key(false)?;
                match ch {
                    E(K::Char(c), M::NONE) => Cmd::ReplaceChar(n, c),
                    E::ESC => Cmd::Noop,
                    _ => Cmd::Unknown,
                }
            }
            E(K::Char('R'), M::NONE) => {
                //  vi-replace-mode (overwrite-mode)
                self.input_mode = InputMode::Replace;
                Cmd::Replace(Movement::ForwardChar(0), None)
            }
            E(K::Char('s'), M::NONE) => {
                // vi-substitute-char:
                self.input_mode = InputMode::Insert;
                Cmd::Replace(Movement::ForwardChar(n), None)
            }
            E(K::Char('S'), M::NONE) => {
                // vi-substitute-line:
                self.input_mode = InputMode::Insert;
                Cmd::Replace(Movement::WholeLine, None)
            }
            E(K::Char('u'), M::NONE) => Cmd::Undo(n),
            // E(K::Char('U'), M::NONE) => Cmd::???, // revert-line
            E(K::Char('w'), M::NONE) => Cmd::Move(Movement::ForwardWord(n, At::Start, Word::Vi)), /* vi-next-word */
            E(K::Char('W'), M::NONE) => Cmd::Move(Movement::ForwardWord(n, At::Start, Word::Big)), /* vi-next-word */
            // TODO move backward if eol
            E(K::Char('x'), M::NONE) => Cmd::Kill(Movement::ForwardChar(n)), // vi-delete
            E(K::Char('X'), M::NONE) => Cmd::Kill(Movement::BackwardChar(n)), // vi-rubout
            E(K::Char('y'), M::NONE) => match self.vi_cmd_motion(rdr, wrt, key, n)? {
                Some(mvt) => Cmd::ViYankTo(mvt),
                None => Cmd::Unknown,
            },
            // E(K::Char('Y'), M::NONE) => Cmd::???, // vi-yank-to
            E(K::Char('h'), M::NONE) | E(K::Char('H'), M::CTRL) | E::BACKSPACE => {
                Cmd::Move(Movement::BackwardChar(n))
            }
            E(K::Char('G'), M::CTRL) => Cmd::Abort,
            E(K::Char('l' | ' '), M::NONE) => Cmd::Move(Movement::ForwardChar(n)),
            E(K::Char('L'), M::CTRL) => Cmd::ClearScreen,
            E(K::Char('+' | 'j'), M::NONE) => Cmd::LineDownOrNextHistory(n),
            // TODO: move to the start of the line.
            E(K::Char('N'), M::CTRL) => Cmd::NextHistory,
            E(K::Char('-' | 'k'), M::NONE) => Cmd::LineUpOrPreviousHistory(n),
            // TODO: move to the start of the line.
            E(K::Char('P'), M::CTRL) => Cmd::PreviousHistory,
            E(K::Char('R'), M::CTRL) => {
                self.input_mode = InputMode::Insert; // TODO Validate
                Cmd::ReverseSearchHistory
            }
            E(K::Char('S'), M::CTRL) => {
                self.input_mode = InputMode::Insert; // TODO Validate
                Cmd::ForwardSearchHistory
            }
            E(K::Char('<'), M::NONE) => match self.vi_cmd_motion(rdr, wrt, key, n)? {
                Some(mvt) => Cmd::Dedent(mvt),
                None => Cmd::Unknown,
            },
            E(K::Char('>'), M::NONE) => match self.vi_cmd_motion(rdr, wrt, key, n)? {
                Some(mvt) => Cmd::Indent(mvt),
                None => Cmd::Unknown,
            },
            E::ESC => Cmd::Noop,
            _ => self.common(rdr, wrt, evt, key, n, true)?,
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
        let evt = key.into();
        if let Some(cmd) = self.custom_binding(wrt, &evt, 0, true) {
            return Ok(if cmd.is_repeatable() {
                cmd.redo(None, wrt)
            } else {
                cmd
            });
        } else if let Some(cmd) = InputState::term_binding(rdr, wrt, &key) {
            return Ok(cmd);
        }
        let cmd = match key {
            E(K::Char(c), M::NONE) => {
                if self.input_mode == InputMode::Replace {
                    Cmd::Overwrite(c)
                } else {
                    Cmd::SelfInsert(1, c)
                }
            }
            E(K::Char('H'), M::CTRL) | E::BACKSPACE => Cmd::Kill(Movement::BackwardChar(1)),
            E(K::BackTab, M::NONE) => Cmd::CompleteBackward,
            E(K::Char('I'), M::CTRL) | E(K::Tab, M::NONE) => Cmd::Complete,
            // Don't complete hints when the cursor is not at the end of a line
            E(K::Right, M::NONE) if wrt.has_hint() && wrt.is_cursor_at_end() => Cmd::CompleteHint,
            E(K::Char(k), M::ALT) => {
                debug!(target: "rustyline", "Vi fast command mode: {}", k);
                self.input_mode = InputMode::Command;
                wrt.done_inserting();

                self.vi_command(rdr, wrt, E(K::Char(k), M::NONE))?
            }
            E::ESC => {
                // vi-movement-mode/vi-command-mode
                self.input_mode = InputMode::Command;
                wrt.done_inserting();
                Cmd::Move(Movement::BackwardChar(1))
            }
            _ => self.common(rdr, wrt, evt, key, 1, true)?,
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
        if let E(K::Char(digit @ '1'..='9'), M::NONE) = mvt {
            // vi-arg-digit
            mvt = self.vi_arg_digit(rdr, wrt, digit)?;
            n = self.vi_num_args().saturating_mul(n);
        }
        Ok(match mvt {
            E(K::Char('$'), M::NONE) => Some(Movement::EndOfLine),
            E(K::Char('0'), M::NONE) => Some(Movement::BeginningOfLine),
            E(K::Char('^'), M::NONE) => Some(Movement::ViFirstPrint),
            E(K::Char('b'), M::NONE) => Some(Movement::BackwardWord(n, Word::Vi)),
            E(K::Char('B'), M::NONE) => Some(Movement::BackwardWord(n, Word::Big)),
            E(K::Char('e'), M::NONE) => Some(Movement::ForwardWord(n, At::AfterEnd, Word::Vi)),
            E(K::Char('E'), M::NONE) => Some(Movement::ForwardWord(n, At::AfterEnd, Word::Big)),
            E(K::Char(c), M::NONE) if c == 'f' || c == 'F' || c == 't' || c == 'T' => {
                let cs = self.vi_char_search(rdr, c)?;
                cs.map(|cs| Movement::ViCharSearch(n, cs))
            }
            E(K::Char(';'), M::NONE) => self
                .last_char_search
                .map(|cs| Movement::ViCharSearch(n, cs)),
            E(K::Char(','), M::NONE) => self
                .last_char_search
                .map(|cs| Movement::ViCharSearch(n, cs.opposite())),
            E(K::Char('h'), M::NONE) | E(K::Char('H'), M::CTRL) | E::BACKSPACE => {
                Some(Movement::BackwardChar(n))
            }
            E(K::Char('l' | ' '), M::NONE) => Some(Movement::ForwardChar(n)),
            E(K::Char('j' | '+'), M::NONE) => Some(Movement::LineDown(n)),
            E(K::Char('k' | '-'), M::NONE) => Some(Movement::LineUp(n)),
            E(K::Char('w'), M::NONE) => {
                // 'cw' is 'ce'
                if key == E(K::Char('c'), M::NONE) {
                    Some(Movement::ForwardWord(n, At::AfterEnd, Word::Vi))
                } else {
                    Some(Movement::ForwardWord(n, At::Start, Word::Vi))
                }
            }
            E(K::Char('W'), M::NONE) => {
                // 'cW' is 'cE'
                if key == E(K::Char('c'), M::NONE) {
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
            E(K::Char(ch), M::NONE) => {
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
        wrt: &dyn Refresher,
        mut evt: Event,
        key: KeyEvent,
        n: RepeatCount,
        positive: bool,
    ) -> Result<Cmd> {
        Ok(match key {
            E(K::Home, M::NONE) => Cmd::Move(Movement::BeginningOfLine),
            E(K::Left, M::NONE) => Cmd::Move(if positive {
                Movement::BackwardChar(n)
            } else {
                Movement::ForwardChar(n)
            }),
            #[cfg(any(windows, test))]
            E(K::Char('C'), M::CTRL) => Cmd::Interrupt,
            E(K::Char('D'), M::CTRL) => {
                if self.is_emacs_mode() && !wrt.line().is_empty() {
                    Cmd::Kill(if positive {
                        Movement::ForwardChar(n)
                    } else {
                        Movement::BackwardChar(n)
                    })
                } else if cfg!(windows) || cfg!(test) || !wrt.line().is_empty() {
                    Cmd::EndOfFile
                } else {
                    Cmd::Unknown
                }
            }
            E(K::Delete, M::NONE) => Cmd::Kill(if positive {
                Movement::ForwardChar(n)
            } else {
                Movement::BackwardChar(n)
            }),
            E(K::End, M::NONE) => Cmd::Move(Movement::EndOfLine),
            E(K::Right, M::NONE) => Cmd::Move(if positive {
                Movement::ForwardChar(n)
            } else {
                Movement::BackwardChar(n)
            }),
            E(K::Char('J' | 'M'), M::CTRL) | E::ENTER => Cmd::AcceptOrInsertLine {
                accept_in_the_middle: true,
            },
            E(K::Down, M::NONE) => Cmd::LineDownOrNextHistory(1),
            E(K::Up, M::NONE) => Cmd::LineUpOrPreviousHistory(1),
            E(K::Char('R'), M::CTRL) => Cmd::ReverseSearchHistory,
            // most terminals override Ctrl+S to suspend execution
            E(K::Char('S'), M::CTRL) => Cmd::ForwardSearchHistory,
            E(K::Char('T'), M::CTRL) => Cmd::TransposeChars,
            E(K::Char('U'), M::CTRL) => Cmd::Kill(if positive {
                Movement::BeginningOfLine
            } else {
                Movement::EndOfLine
            }),
            // most terminals override Ctrl+Q to resume execution
            E(K::Char('Q'), M::CTRL) => Cmd::QuotedInsert,
            #[cfg(not(windows))]
            E(K::Char('V'), M::CTRL) => Cmd::QuotedInsert,
            #[cfg(windows)]
            E(K::Char('V'), M::CTRL) => Cmd::PasteFromClipboard,
            E(K::Char('W'), M::CTRL) => Cmd::Kill(if positive {
                Movement::BackwardWord(n, Word::Big)
            } else {
                Movement::ForwardWord(n, At::AfterEnd, Word::Big)
            }),
            E(K::Char('Y'), M::CTRL) => {
                if positive {
                    Cmd::Yank(n, Anchor::Before)
                } else {
                    Cmd::Unknown // TODO Validate
                }
            }
            E(K::Char('_'), M::CTRL) => Cmd::Undo(n),
            E(K::UnknownEscSeq, M::NONE) => Cmd::Noop,
            E(K::BracketedPasteStart, M::NONE) => {
                let paste = rdr.read_pasted_text()?;
                Cmd::Insert(1, paste)
            }
            _ => self
                .custom_seq_binding(rdr, wrt, &mut evt, n, positive)?
                .unwrap_or(Cmd::Unknown),
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

    #[expect(clippy::cast_sign_loss)]
    fn emacs_num_args(&mut self) -> (RepeatCount, bool) {
        let num_args = self.num_args();
        if num_args < 0 {
            if let (n, false) = num_args.overflowing_abs() {
                (n as RepeatCount, false)
            } else {
                (RepeatCount::MAX, false)
            }
        } else {
            (num_args as RepeatCount, true)
        }
    }

    fn vi_num_args(&mut self) -> RepeatCount {
        let num_args = self.num_args();
        if num_args < 0 {
            unreachable!()
        } else {
            num_args.unsigned_abs() as RepeatCount
        }
    }
}

#[cfg(feature = "custom-bindings")]
impl InputState<'_> {
    /// Application customized binding
    fn custom_binding(
        &self,
        wrt: &dyn Refresher,
        evt: &Event,
        n: RepeatCount,
        positive: bool,
    ) -> Option<Cmd> {
        let bindings = self.custom_bindings;
        let handler = bindings.get(evt).or_else(|| bindings.get(&Event::Any));
        if let Some(handler) = handler {
            match handler {
                EventHandler::Simple(cmd) => Some(cmd.clone()),
                EventHandler::Conditional(handler) => {
                    let ctx = EventContext::new(self, wrt);
                    handler.handle(evt, n, positive, &ctx)
                }
            }
        } else {
            None
        }
    }

    fn custom_seq_binding<R: RawReader>(
        &self,
        rdr: &mut R,
        wrt: &dyn Refresher,
        evt: &mut Event,
        n: RepeatCount,
        positive: bool,
    ) -> Result<Option<Cmd>> {
        while let Some(subtrie) = self.custom_bindings.get_raw_descendant(evt) {
            let snd_key = rdr.next_key(true)?;
            if let Event::KeySeq(ref mut key_seq) = evt {
                key_seq.push(snd_key);
            } else {
                break;
            }
            let handler = subtrie.get(evt).unwrap();
            if let Some(handler) = handler {
                let cmd = match handler {
                    EventHandler::Simple(cmd) => Some(cmd.clone()),
                    EventHandler::Conditional(handler) => {
                        let ctx = EventContext::new(self, wrt);
                        handler.handle(evt, n, positive, &ctx)
                    }
                };
                if cmd.is_some() {
                    return Ok(cmd);
                }
            }
        }
        Ok(None)
    }
}

#[cfg(not(feature = "custom-bindings"))]
impl<'b> InputState<'b> {
    fn custom_binding(&self, _: &dyn Refresher, _: &Event, _: RepeatCount, _: bool) -> Option<Cmd> {
        None
    }

    fn custom_seq_binding<R: RawReader>(
        &self,
        _: &mut R,
        _: &dyn Refresher,
        _: &mut Event,
        _: RepeatCount,
        _: bool,
    ) -> Result<Option<Cmd>> {
        Ok(None)
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "custom-bindings")] {
pub type Bindings = radix_trie::Trie<Event, EventHandler>;
    } else {
enum Event {
   KeySeq([KeyEvent; 1]),
}
impl From<KeyEvent> for Event {
    fn from(k: KeyEvent) -> Self {
        Self::KeySeq([k])
    }
}
pub struct Bindings {}
impl Bindings {
    pub fn new() -> Self {
        Self {}
    }
}
    }
}
