//! Bindings from keys to command for Emacs and Vi modes
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::Result;
use config::Config;
use config::EditMode;
use keys::KeyPress;
use tty::RawReader;

/// The number of times one command should be repeated.
pub type RepeatCount = usize;

/// Commands
/// #[non_exhaustive]
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
}

impl Cmd {
    pub fn should_reset_kill_ring(&self) -> bool {
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
            Cmd::TransposeChars => false, // TODO Validate
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
    fn redo(&self, new: Option<RepeatCount>, wrt: &Refresher) -> Cmd {
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
                            Movement::ForwardChar(
                                last_insert.as_ref().map_or(0, |text| text.len()),
                            ),
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
    fn opposite(self) -> CharSearch {
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
}

impl Movement {
    // Replay this movement with a possible different `RepeatCount`.
    fn redo(&self, new: Option<RepeatCount>) -> Movement {
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

/// Tranform key(s) to commands based on current input mode
pub struct InputState {
    mode: EditMode,
    custom_bindings: Arc<RwLock<HashMap<KeyPress, Cmd>>>,
    input_mode: InputMode, // vi only ?
    // numeric arguments: http://web.mit.edu/gnu/doc/html/rlman_1.html#SEC7
    num_args: i16,
    last_cmd: Cmd,                        // vi only
    last_char_search: Option<CharSearch>, // vi only
}

pub trait Refresher {
    /// Rewrite the currently edited line accordingly to the buffer content,
    /// cursor position, and number of columns of the terminal.
    fn refresh_line(&mut self) -> Result<()>;
    /// Same as `refresh_line` but with a dynamic prompt.
    fn refresh_prompt_and_line(&mut self, prompt: &str) -> Result<()>;
    /// Vi only, switch to insert mode.
    fn doing_insert(&mut self);
    /// Vi only, switch to command mode.
    fn done_inserting(&mut self);
    /// Vi only, last text inserted.
    fn last_insert(&self) -> Option<String>;
}

impl InputState {
    pub fn new(
        config: &Config,
        custom_bindings: Arc<RwLock<HashMap<KeyPress, Cmd>>>,
    ) -> InputState {
        InputState {
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
    pub fn next_cmd<R: RawReader>(
        &mut self,
        rdr: &mut R,
        wrt: &mut Refresher,
        single_esc_abort: bool,
    ) -> Result<Cmd> {
        match self.mode {
            EditMode::Emacs => self.emacs(rdr, wrt, single_esc_abort),
            EditMode::Vi if self.input_mode != InputMode::Command => self.vi_insert(rdr, wrt),
            EditMode::Vi => self.vi_command(rdr, wrt),
        }
    }

    // TODO dynamic prompt (arg: ?)
    fn emacs_digit_argument<R: RawReader>(
        &mut self,
        rdr: &mut R,
        wrt: &mut Refresher,
        digit: char,
    ) -> Result<KeyPress> {
        match digit {
            '0'...'9' => {
                self.num_args = digit.to_digit(10).unwrap() as i16;
            }
            '-' => {
                self.num_args = -1;
            }
            _ => unreachable!(),
        }
        loop {
            try!(wrt.refresh_prompt_and_line(&format!("(arg: {}) ", self.num_args)));
            let key = try!(rdr.next_key(true));
            match key {
                KeyPress::Char(digit @ '0'...'9') | KeyPress::Meta(digit @ '0'...'9') => {
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
                KeyPress::Char('-') | KeyPress::Meta('-') => {}
                _ => {
                    try!(wrt.refresh_line());
                    return Ok(key);
                }
            };
        }
    }

    fn emacs<R: RawReader>(
        &mut self,
        rdr: &mut R,
        wrt: &mut Refresher,
        single_esc_abort: bool,
    ) -> Result<Cmd> {
        let mut key = try!(rdr.next_key(single_esc_abort));
        if let KeyPress::Meta(digit @ '-') = key {
            key = try!(self.emacs_digit_argument(rdr, wrt, digit));
        } else if let KeyPress::Meta(digit @ '0'...'9') = key {
            key = try!(self.emacs_digit_argument(rdr, wrt, digit));
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
            KeyPress::Char(c) => if positive {
                Cmd::SelfInsert(n, c)
            } else {
                Cmd::Unknown
            },
            KeyPress::Ctrl('A') => Cmd::Move(Movement::BeginningOfLine),
            KeyPress::Ctrl('B') => if positive {
                Cmd::Move(Movement::BackwardChar(n))
            } else {
                Cmd::Move(Movement::ForwardChar(n))
            },
            KeyPress::Ctrl('E') => Cmd::Move(Movement::EndOfLine),
            KeyPress::Ctrl('F') => if positive {
                Cmd::Move(Movement::ForwardChar(n))
            } else {
                Cmd::Move(Movement::BackwardChar(n))
            },
            KeyPress::Ctrl('G') | KeyPress::Esc | KeyPress::Meta('\x07') => Cmd::Abort,
            KeyPress::Ctrl('H') | KeyPress::Backspace => if positive {
                Cmd::Kill(Movement::BackwardChar(n))
            } else {
                Cmd::Kill(Movement::ForwardChar(n))
            },
            KeyPress::Tab => Cmd::Complete,
            KeyPress::Ctrl('K') => if positive {
                Cmd::Kill(Movement::EndOfLine)
            } else {
                Cmd::Kill(Movement::BeginningOfLine)
            },
            KeyPress::Ctrl('L') => Cmd::ClearScreen,
            KeyPress::Ctrl('N') => Cmd::NextHistory,
            KeyPress::Ctrl('P') => Cmd::PreviousHistory,
            KeyPress::Ctrl('X') => {
                let snd_key = try!(rdr.next_key(true));
                match snd_key {
                    KeyPress::Ctrl('G') | KeyPress::Esc => Cmd::Abort,
                    KeyPress::Ctrl('U') => Cmd::Undo(n),
                    _ => Cmd::Unknown,
                }
            }
            KeyPress::Meta('\x08') | KeyPress::Meta('\x7f') => if positive {
                Cmd::Kill(Movement::BackwardWord(n, Word::Emacs))
            } else {
                Cmd::Kill(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
            },
            KeyPress::Meta('<') => Cmd::BeginningOfHistory,
            KeyPress::Meta('>') => Cmd::EndOfHistory,
            KeyPress::Meta('B') | KeyPress::Meta('b') => if positive {
                Cmd::Move(Movement::BackwardWord(n, Word::Emacs))
            } else {
                Cmd::Move(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
            },
            KeyPress::Meta('C') | KeyPress::Meta('c') => Cmd::CapitalizeWord,
            KeyPress::Meta('D') | KeyPress::Meta('d') => if positive {
                Cmd::Kill(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
            } else {
                Cmd::Kill(Movement::BackwardWord(n, Word::Emacs))
            },
            KeyPress::Meta('F') | KeyPress::Meta('f') => if positive {
                Cmd::Move(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
            } else {
                Cmd::Move(Movement::BackwardWord(n, Word::Emacs))
            },
            KeyPress::Meta('L') | KeyPress::Meta('l') => Cmd::DowncaseWord,
            KeyPress::Meta('T') | KeyPress::Meta('t') => Cmd::TransposeWords(n),
            KeyPress::Meta('U') | KeyPress::Meta('u') => Cmd::UpcaseWord,
            KeyPress::Meta('Y') | KeyPress::Meta('y') => Cmd::YankPop,
            _ => self.common(key, n, positive),
        };
        debug!(target: "rustyline", "Emacs command: {:?}", cmd);
        Ok(cmd)
    }

    fn vi_arg_digit<R: RawReader>(
        &mut self,
        rdr: &mut R,
        wrt: &mut Refresher,
        digit: char,
    ) -> Result<KeyPress> {
        self.num_args = digit.to_digit(10).unwrap() as i16;
        loop {
            try!(wrt.refresh_prompt_and_line(&format!("(arg: {}) ", self.num_args)));
            let key = try!(rdr.next_key(false));
            if let KeyPress::Char(digit @ '0'...'9') = key {
                if self.num_args.abs() < 1000 {
                    // shouldn't ever need more than 4 digits
                    self.num_args = self
                        .num_args
                        .saturating_mul(10)
                        .saturating_add(digit.to_digit(10).unwrap() as i16);
                }
            } else {
                try!(wrt.refresh_line());
                return Ok(key);
            };
        }
    }

    fn vi_command<R: RawReader>(&mut self, rdr: &mut R, wrt: &mut Refresher) -> Result<Cmd> {
        let mut key = try!(rdr.next_key(false));
        if let KeyPress::Char(digit @ '1'...'9') = key {
            key = try!(self.vi_arg_digit(rdr, wrt, digit));
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
            KeyPress::Char('$') |
            KeyPress::End => Cmd::Move(Movement::EndOfLine),
            KeyPress::Char('.') => { // vi-redo (repeat last command)
                if no_num_args {
                    self.last_cmd.redo(None, wrt)
                } else {
                    self.last_cmd.redo(Some(n), wrt)
                }
            },
            // TODO KeyPress::Char('%') => Cmd::???, Move to the corresponding opening/closing bracket
            KeyPress::Char('0') => Cmd::Move(Movement::BeginningOfLine),
            KeyPress::Char('^') => Cmd::Move(Movement::ViFirstPrint),
            KeyPress::Char('a') => {
                // vi-append-mode
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Move(Movement::ForwardChar(n))
            }
            KeyPress::Char('A') => {
                // vi-append-eol
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Move(Movement::EndOfLine)
            }
            KeyPress::Char('b') => Cmd::Move(Movement::BackwardWord(n, Word::Vi)), // vi-prev-word
            KeyPress::Char('B') => Cmd::Move(Movement::BackwardWord(n, Word::Big)),
            KeyPress::Char('c') => {
                self.input_mode = InputMode::Insert;
                match try!(self.vi_cmd_motion(rdr, wrt, key, n)) {
                    Some(mvt) => Cmd::Replace(mvt, None),
                    None => Cmd::Unknown,
                }
            }
            KeyPress::Char('C') => {
                self.input_mode = InputMode::Insert;
                Cmd::Replace(Movement::EndOfLine, None)
            }
            KeyPress::Char('d') => {
                match try!(self.vi_cmd_motion(rdr, wrt, key, n)) {
                    Some(mvt) => Cmd::Kill(mvt),
                    None => Cmd::Unknown,
                }
            }
            KeyPress::Char('D') |
            KeyPress::Ctrl('K') => Cmd::Kill(Movement::EndOfLine),
            KeyPress::Char('e') => Cmd::Move(Movement::ForwardWord(n, At::BeforeEnd, Word::Vi)),
            KeyPress::Char('E') => Cmd::Move(Movement::ForwardWord(n, At::BeforeEnd, Word::Big)),
            KeyPress::Char('i') => {
                // vi-insertion-mode
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Noop
            }
            KeyPress::Char('I') => {
                // vi-insert-beg
                self.input_mode = InputMode::Insert;
                wrt.doing_insert();
                Cmd::Move(Movement::BeginningOfLine)
            }
            KeyPress::Char(c) if c == 'f' || c == 'F' || c == 't' || c == 'T' => {
                // vi-char-search
                let cs = try!(self.vi_char_search(rdr, c));
                match cs {
                    Some(cs) => Cmd::Move(Movement::ViCharSearch(n, cs)),
                    None => Cmd::Unknown,
                }
            }
            KeyPress::Char(';') => {
                match self.last_char_search {
                    Some(cs) => Cmd::Move(Movement::ViCharSearch(n, cs)),
                    None => Cmd::Noop,
                }
            }
            KeyPress::Char(',') => {
                match self.last_char_search {
                    Some(ref cs) => Cmd::Move(Movement::ViCharSearch(n, cs.opposite())),
                    None => Cmd::Noop,
                }
            }
            // TODO KeyPress::Char('G') => Cmd::???, Move to the history line n
            KeyPress::Char('p') => Cmd::Yank(n, Anchor::After), // vi-put
            KeyPress::Char('P') => Cmd::Yank(n, Anchor::Before), // vi-put
            KeyPress::Char('r') => {
                // vi-replace-char:
                let ch = try!(rdr.next_key(false));
                match ch {
                    KeyPress::Char(c) => Cmd::ReplaceChar(n, c),
                    KeyPress::Esc => Cmd::Noop,
                    _ => Cmd::Unknown,
                }
            }
            KeyPress::Char('R') => {
                //  vi-replace-mode (overwrite-mode)
                self.input_mode = InputMode::Replace;
                Cmd::Replace(Movement::ForwardChar(0), None)
            }
            KeyPress::Char('s') => {
                // vi-substitute-char:
                self.input_mode = InputMode::Insert;
                Cmd::Replace(Movement::ForwardChar(n), None)
            }
            KeyPress::Char('S') => {
                // vi-substitute-line:
                self.input_mode = InputMode::Insert;
                Cmd::Replace(Movement::WholeLine, None)
            }
            KeyPress::Char('u') => Cmd::Undo(n),
            // KeyPress::Char('U') => Cmd::???, // revert-line
            KeyPress::Char('w') => Cmd::Move(Movement::ForwardWord(n, At::Start, Word::Vi)), // vi-next-word
            KeyPress::Char('W') => Cmd::Move(Movement::ForwardWord(n, At::Start, Word::Big)), // vi-next-word
            KeyPress::Char('x') => Cmd::Kill(Movement::ForwardChar(n)), // vi-delete: TODO move backward if eol
            KeyPress::Char('X') => Cmd::Kill(Movement::BackwardChar(n)), // vi-rubout
            KeyPress::Char('y') => {
                match try!(self.vi_cmd_motion(rdr, wrt, key, n)) {
                    Some(mvt) => Cmd::ViYankTo(mvt),
                    None => Cmd::Unknown,
                }
            }
            // KeyPress::Char('Y') => Cmd::???, // vi-yank-to
            KeyPress::Char('h') |
            KeyPress::Ctrl('H') |
            KeyPress::Backspace => Cmd::Move(Movement::BackwardChar(n)),
            KeyPress::Ctrl('G') => Cmd::Abort,
            KeyPress::Char('l') |
            KeyPress::Char(' ') => Cmd::Move(Movement::ForwardChar(n)),
            KeyPress::Ctrl('L') => Cmd::ClearScreen,
            KeyPress::Char('+') |
            KeyPress::Char('j') | // TODO: move to the start of the line.
            KeyPress::Ctrl('N') => Cmd::NextHistory,
            KeyPress::Char('-') |
            KeyPress::Char('k') | // TODO: move to the start of the line.
            KeyPress::Ctrl('P') => Cmd::PreviousHistory,
            KeyPress::Ctrl('R') => {
                self.input_mode = InputMode::Insert; // TODO Validate
                Cmd::ReverseSearchHistory
            }
            KeyPress::Ctrl('S') => {
                self.input_mode = InputMode::Insert; // TODO Validate
                Cmd::ForwardSearchHistory
            }
            KeyPress::Esc => Cmd::Noop,
            _ => self.common(key, n, true),
        };
        debug!(target: "rustyline", "Vi command: {:?}", cmd);
        if cmd.is_repeatable_change() {
            self.last_cmd = cmd.clone();
        }
        Ok(cmd)
    }

    fn vi_insert<R: RawReader>(&mut self, rdr: &mut R, wrt: &mut Refresher) -> Result<Cmd> {
        let key = try!(rdr.next_key(false));
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
            KeyPress::Char(c) => if self.input_mode == InputMode::Replace {
                Cmd::Overwrite(c)
            } else {
                Cmd::SelfInsert(1, c)
            },
            KeyPress::Ctrl('H') | KeyPress::Backspace => Cmd::Kill(Movement::BackwardChar(1)),
            KeyPress::Tab => Cmd::Complete,
            KeyPress::Esc => {
                // vi-movement-mode/vi-command-mode
                self.input_mode = InputMode::Command;
                wrt.done_inserting();
                Cmd::Move(Movement::BackwardChar(1))
            }
            _ => self.common(key, 1, true),
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
        wrt: &mut Refresher,
        key: KeyPress,
        n: RepeatCount,
    ) -> Result<Option<Movement>> {
        let mut mvt = try!(rdr.next_key(false));
        if mvt == key {
            return Ok(Some(Movement::WholeLine));
        }
        let mut n = n;
        if let KeyPress::Char(digit @ '1'...'9') = mvt {
            // vi-arg-digit
            mvt = try!(self.vi_arg_digit(rdr, wrt, digit));
            n = self.vi_num_args().saturating_mul(n);
        }
        Ok(match mvt {
            KeyPress::Char('$') => Some(Movement::EndOfLine),
            KeyPress::Char('0') => Some(Movement::BeginningOfLine),
            KeyPress::Char('^') => Some(Movement::ViFirstPrint),
            KeyPress::Char('b') => Some(Movement::BackwardWord(n, Word::Vi)),
            KeyPress::Char('B') => Some(Movement::BackwardWord(n, Word::Big)),
            KeyPress::Char('e') => Some(Movement::ForwardWord(n, At::AfterEnd, Word::Vi)),
            KeyPress::Char('E') => Some(Movement::ForwardWord(n, At::AfterEnd, Word::Big)),
            KeyPress::Char(c) if c == 'f' || c == 'F' || c == 't' || c == 'T' => {
                let cs = try!(self.vi_char_search(rdr, c));
                match cs {
                    Some(cs) => Some(Movement::ViCharSearch(n, cs)),
                    None => None,
                }
            }
            KeyPress::Char(';') => match self.last_char_search {
                Some(cs) => Some(Movement::ViCharSearch(n, cs)),
                None => None,
            },
            KeyPress::Char(',') => match self.last_char_search {
                Some(ref cs) => Some(Movement::ViCharSearch(n, cs.opposite())),
                None => None,
            },
            KeyPress::Char('h') | KeyPress::Ctrl('H') | KeyPress::Backspace => {
                Some(Movement::BackwardChar(n))
            }
            KeyPress::Char('l') | KeyPress::Char(' ') => Some(Movement::ForwardChar(n)),
            KeyPress::Char('w') => {
                // 'cw' is 'ce'
                if key == KeyPress::Char('c') {
                    Some(Movement::ForwardWord(n, At::AfterEnd, Word::Vi))
                } else {
                    Some(Movement::ForwardWord(n, At::Start, Word::Vi))
                }
            }
            KeyPress::Char('W') => {
                // 'cW' is 'cE'
                if key == KeyPress::Char('c') {
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
        let ch = try!(rdr.next_key(false));
        Ok(match ch {
            KeyPress::Char(ch) => {
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

    fn common(&mut self, key: KeyPress, n: RepeatCount, positive: bool) -> Cmd {
        match key {
            KeyPress::Home => Cmd::Move(Movement::BeginningOfLine),
            KeyPress::Left => {
                if positive {
                    Cmd::Move(Movement::BackwardChar(n))
                } else {
                    Cmd::Move(Movement::ForwardChar(n))
                }
            }
            KeyPress::Ctrl('C') => Cmd::Interrupt,
            KeyPress::Ctrl('D') => Cmd::EndOfFile,
            KeyPress::Delete => {
                if positive {
                    Cmd::Kill(Movement::ForwardChar(n))
                } else {
                    Cmd::Kill(Movement::BackwardChar(n))
                }
            }
            KeyPress::End => Cmd::Move(Movement::EndOfLine),
            KeyPress::Right => {
                if positive {
                    Cmd::Move(Movement::ForwardChar(n))
                } else {
                    Cmd::Move(Movement::BackwardChar(n))
                }
            }
            KeyPress::Ctrl('J') |
            KeyPress::Enter => Cmd::AcceptLine,
            KeyPress::Down => Cmd::NextHistory,
            KeyPress::Up => Cmd::PreviousHistory,
            KeyPress::Ctrl('R') => Cmd::ReverseSearchHistory,
            KeyPress::Ctrl('S') => Cmd::ForwardSearchHistory, // most terminals override Ctrl+S to suspend execution
            KeyPress::Ctrl('T') => Cmd::TransposeChars,
            KeyPress::Ctrl('U') => {
                if positive {
                    Cmd::Kill(Movement::BeginningOfLine)
                } else {
                    Cmd::Kill(Movement::EndOfLine)
                }
            },
            KeyPress::Ctrl('Q') | // most terminals override Ctrl+Q to resume execution
            KeyPress::Ctrl('V') => Cmd::QuotedInsert,
            KeyPress::Ctrl('W') => {
                if positive {
                    Cmd::Kill(Movement::BackwardWord(n, Word::Big))
                } else {
                    Cmd::Kill(Movement::ForwardWord(n, At::AfterEnd, Word::Big))
                }
            }
            KeyPress::Ctrl('Y') => {
                if positive {
                    Cmd::Yank(n, Anchor::Before)
                } else {
                    Cmd::Unknown // TODO Validate
                }
            }
            KeyPress::Ctrl('Z') => Cmd::Suspend,
            KeyPress::Ctrl('_') => Cmd::Undo(n),
            KeyPress::UnknownEscSeq => Cmd::Noop,
            _ => Cmd::Unknown,
        }
    }

    fn num_args(&mut self) -> i16 {
        let num_args = match self.num_args {
            0 => 1,
            _ => self.num_args,
        };
        self.num_args = 0;
        num_args
    }

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

    fn vi_num_args(&mut self) -> RepeatCount {
        let num_args = self.num_args();
        if num_args < 0 {
            unreachable!()
        } else {
            num_args.abs() as RepeatCount
        }
    }
}
