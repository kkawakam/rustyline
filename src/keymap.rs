use config::Config;
use config::EditMode;
use consts::KeyPress;
use tty::RawReader;
use super::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    Abort, // Miscellaneous Command
    AcceptLine,
    BackwardChar(i32),
    BackwardDeleteChar(i32),
    BackwardKillWord(i32, Word), // Backward until start of word
    BackwardWord(i32, Word), // Backward until start of word
    BeginningOfHistory,
    BeginningOfLine,
    CapitalizeWord,
    ClearScreen,
    Complete,
    DeleteChar(i32),
    DowncaseWord,
    EndOfFile,
    EndOfHistory,
    EndOfLine,
    ForwardChar(i32),
    ForwardSearchHistory,
    ForwardWord(i32, At, Word), // Forward until start/end of word
    Interrupt,
    KillLine,
    KillWholeLine,
    KillWord(i32, At, Word), // Forward until start/end of word
    NextHistory,
    Noop,
    PreviousHistory,
    QuotedInsert,
    Replace(i32, char), // TODO DeleteChar + SelfInsert
    ReverseSearchHistory,
    SelfInsert(char),
    Suspend,
    TransposeChars,
    TransposeWords,
    Unknown,
    UnixLikeDiscard,
    // UnixWordRubout, // = BackwardKillWord(Word::Big)
    UpcaseWord,
    ViCharSearch(i32, CharSearch),
    ViDeleteTo(i32, CharSearch),
    Yank(i32),
    YankPop,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum Word {
    // non-blanks characters
    Big,
    // alphanumeric characters
    Emacs,
    // alphanumeric (and '_') characters
    Vi,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum At {
    Start,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CharSearch {
    Forward(char),
    // until
    ForwardBefore(char),
    Backward(char),
    // until
    BackwardAfter(char),
}

pub struct EditState {
    mode: EditMode,
    // Vi Command/Alternate, Insert/Input mode
    insert: bool, // vi only ?
    // numeric arguments: http://web.mit.edu/gnu/doc/html/rlman_1.html#SEC7
    num_args: i32,
}

impl EditState {
    pub fn new(config: &Config) -> EditState {
        EditState {
            mode: config.edit_mode(),
            insert: true,
            num_args: 0,
        }
    }

    pub fn is_emacs_mode(&self) -> bool {
        self.mode == EditMode::Emacs
    }

    pub fn next_cmd<R: RawReader>(&mut self, rdr: &mut R, config: &Config) -> Result<Cmd> {
        match self.mode {
            EditMode::Emacs => self.emacs(rdr, config),
            EditMode::Vi if self.insert => self.vi_insert(rdr, config),
            EditMode::Vi => self.vi_command(rdr, config),
        }
    }

    fn digit_argument<R: RawReader>(&mut self,
                                    rdr: &mut R,
                                    config: &Config,
                                    digit: char)
                                    -> Result<KeyPress> {
        match digit {
            '0'...'9' => {
                self.num_args = digit.to_digit(10).unwrap() as i32;
            }
            '-' => {
                self.num_args = -1;
            }
            _ => unreachable!(),
        }
        loop {
            let key = try!(rdr.next_key(config.keyseq_timeout()));
            match key {
                KeyPress::Char(digit @ '0'...'9') |
                KeyPress::Meta(digit @ '0'...'9') => {
                    self.num_args = self.num_args * 10 + digit.to_digit(10).unwrap() as i32;
                }
                _ => return Ok(key),
            };
        }
    }

    fn emacs<R: RawReader>(&mut self, rdr: &mut R, config: &Config) -> Result<Cmd> {
        let mut key = try!(rdr.next_key(config.keyseq_timeout()));
        if let KeyPress::Meta(digit @ '-') = key {
            key = try!(self.digit_argument(rdr, config, digit));
        } else if let KeyPress::Meta(digit @ '0'...'9') = key {
            key = try!(self.digit_argument(rdr, config, digit));
        }
        let cmd = match key {
            KeyPress::Char(c) => Cmd::SelfInsert(c),
            KeyPress::Ctrl('A') => Cmd::BeginningOfLine,
            KeyPress::Ctrl('B') => Cmd::BackwardChar(self.num_args()),
            KeyPress::Ctrl('E') => Cmd::EndOfLine,
            KeyPress::Ctrl('F') => Cmd::ForwardChar(self.num_args()),
            KeyPress::Ctrl('G') |
            KeyPress::Esc => Cmd::Abort,
            KeyPress::Ctrl('H') |
            KeyPress::Backspace => Cmd::BackwardDeleteChar(self.num_args()),
            KeyPress::Tab => Cmd::Complete,
            KeyPress::Ctrl('K') => Cmd::KillLine,
            KeyPress::Ctrl('L') => Cmd::ClearScreen,
            KeyPress::Ctrl('N') => Cmd::NextHistory,
            KeyPress::Ctrl('P') => Cmd::PreviousHistory,
            KeyPress::Meta('\x08') |
            KeyPress::Meta('\x7f') => Cmd::BackwardKillWord(self.num_args(), Word::Emacs),
            KeyPress::Meta('<') => Cmd::BeginningOfHistory,
            KeyPress::Meta('>') => Cmd::EndOfHistory,
            KeyPress::Meta('B') => Cmd::BackwardWord(self.num_args(), Word::Emacs),
            KeyPress::Meta('C') => Cmd::CapitalizeWord,
            KeyPress::Meta('D') => Cmd::KillWord(self.num_args(), At::End, Word::Emacs),
            KeyPress::Meta('F') => Cmd::ForwardWord(self.num_args(), At::End, Word::Emacs),
            KeyPress::Meta('L') => Cmd::DowncaseWord,
            KeyPress::Meta('T') => Cmd::TransposeWords,
            KeyPress::Meta('U') => Cmd::UpcaseWord,
            KeyPress::Meta('Y') => Cmd::YankPop,
            _ => self.common(key),
        };
        Ok(cmd)
    }

    fn vi_arg_digit<R: RawReader>(&mut self,
                                  rdr: &mut R,
                                  config: &Config,
                                  digit: char)
                                  -> Result<KeyPress> {
        self.num_args = digit.to_digit(10).unwrap() as i32;
        loop {
            let key = try!(rdr.next_key(config.keyseq_timeout()));
            match key {
                KeyPress::Char(digit @ '0'...'9') => {
                    self.num_args = self.num_args * 10 + digit.to_digit(10).unwrap() as i32;
                }
                _ => return Ok(key),
            };
        }
    }

    fn vi_command<R: RawReader>(&mut self, rdr: &mut R, config: &Config) -> Result<Cmd> {
        let mut key = try!(rdr.next_key(config.keyseq_timeout()));
        if let KeyPress::Char(digit @ '1'...'9') = key {
            key = try!(self.vi_arg_digit(rdr, config, digit));
        }
        let cmd = match key {
            KeyPress::Char('$') |
            KeyPress::End => Cmd::EndOfLine,
            // TODO KeyPress::Char('%') => Cmd::???, Move to the corresponding opening/closing bracket
            KeyPress::Char('0') => Cmd::BeginningOfLine, // vi-zero: Vi move to the beginning of line.
            KeyPress::Char('^') => Cmd::BeginningOfLine, // vi-first-print TODO Move to the first non-blank character of line.
            KeyPress::Char('a') => {
                // vi-append-mode: Vi enter insert mode after the cursor.
                self.insert = true;
                Cmd::ForwardChar(self.num_args())
            }
            KeyPress::Char('A') => {
                // vi-append-eol: Vi enter insert mode at end of line.
                self.insert = true;
                Cmd::EndOfLine
            }
            KeyPress::Char('b') => Cmd::BackwardWord(self.num_args(), Word::Vi), // vi-prev-word
            KeyPress::Char('B') => Cmd::BackwardWord(self.num_args(), Word::Big),
            KeyPress::Char('c') => {
                self.insert = true;
                try!(self.vi_delete_motion(rdr, config, key))
            }
            KeyPress::Char('C') => {
                self.insert = true;
                Cmd::KillLine
            }
            KeyPress::Char('d') => try!(self.vi_delete_motion(rdr, config, key)),
            KeyPress::Char('D') |
            KeyPress::Ctrl('K') => Cmd::KillLine,
            KeyPress::Char('e') => Cmd::ForwardWord(self.num_args(), At::End, Word::Vi),
            KeyPress::Char('E') => Cmd::ForwardWord(self.num_args(), At::End, Word::Big),
            KeyPress::Char('i') => {
                // vi-insertion-mode
                self.insert = true;
                Cmd::Noop
            }
            KeyPress::Char('I') => {
                // vi-insert-beg
                self.insert = true;
                Cmd::BeginningOfLine
            }
            KeyPress::Char(c) if c == 'f' || c == 'F' || c == 't' || c == 'T' => {
                // vi-char-search
                let cs = try!(self.vi_char_search(rdr, config, c));
                match cs {
                    Some(cs) => Cmd::ViCharSearch(self.num_args(), cs),
                    None => Cmd::Unknown,
                }
            }
            // TODO KeyPress::Char('G') => Cmd::???, Move to the history line n
            KeyPress::Char('p') => Cmd::Yank(self.num_args()), // vi-put FIXME cursor at end
            KeyPress::Char('P') => Cmd::Yank(self.num_args()), // vi-put TODO Insert the yanked text before the cursor.
            KeyPress::Char('r') => {
                // vi-replace-char: Vi replace character under the cursor with the next character typed.
                let ch = try!(rdr.next_key(config.keyseq_timeout()));
                match ch {
                    KeyPress::Char(c) => Cmd::Replace(self.num_args(), c),
                    KeyPress::Esc => Cmd::Noop,
                    _ => Cmd::Unknown,
                }
            }
            // TODO KeyPress::Char('R') => Cmd::???, vi-replace-mode: Vi enter replace mode. Replaces characters under the cursor. (overwrite-mode)
            KeyPress::Char('s') => {
                // vi-substitute-char: Vi replace character under the cursor and enter insert mode.
                self.insert = true;
                Cmd::DeleteChar(self.num_args())
            }
            KeyPress::Char('S') => {
                // vi-substitute-line: Vi substitute entire line.
                self.insert = true;
                Cmd::KillWholeLine
            }
            // KeyPress::Char('U') => Cmd::???, // revert-line
            KeyPress::Char('w') => Cmd::ForwardWord(self.num_args(), At::Start, Word::Vi), // vi-next-word
            KeyPress::Char('W') => Cmd::ForwardWord(self.num_args(), At::Start, Word::Big), // vi-next-word
            KeyPress::Char('x') => Cmd::DeleteChar(self.num_args()), // vi-delete: TODO move backward if eol
            KeyPress::Char('X') => Cmd::BackwardDeleteChar(self.num_args()), // vi-rubout
            // KeyPress::Char('y') => Cmd::???, // vi-yank-to
            // KeyPress::Char('Y') => Cmd::???, // vi-yank-to
            KeyPress::Char('h') |
            KeyPress::Ctrl('H') |
            KeyPress::Backspace => Cmd::BackwardChar(self.num_args()), // TODO Validate
            KeyPress::Ctrl('G') => Cmd::Abort,
            KeyPress::Char('l') |
            KeyPress::Char(' ') => Cmd::ForwardChar(self.num_args()),
            KeyPress::Ctrl('L') => Cmd::ClearScreen,
            KeyPress::Char('+') |
            KeyPress::Char('j') |
            KeyPress::Ctrl('N') => Cmd::NextHistory,
            KeyPress::Char('-') |
            KeyPress::Char('k') |
            KeyPress::Ctrl('P') => Cmd::PreviousHistory,
            KeyPress::Ctrl('R') => {
                self.insert = true; // TODO Validate
                Cmd::ReverseSearchHistory
            }
            KeyPress::Ctrl('S') => {
                self.insert = true; // TODO Validate
                Cmd::ForwardSearchHistory
            }
            KeyPress::Esc => Cmd::Noop,
            _ => self.common(key),
        };
        Ok(cmd)
    }

    fn vi_insert<R: RawReader>(&mut self, rdr: &mut R, config: &Config) -> Result<Cmd> {
        let key = try!(rdr.next_key(config.keyseq_timeout()));
        let cmd = match key {
            KeyPress::Char(c) => Cmd::SelfInsert(c),
            KeyPress::Ctrl('H') |
            KeyPress::Backspace => Cmd::BackwardDeleteChar(1),
            KeyPress::Tab => Cmd::Complete,
            KeyPress::Esc => {
                // vi-movement-mode/vi-command-mode: Vi enter command mode (use alternative key bindings).
                self.insert = false;
                Cmd::BackwardChar(1)
            }
            _ => self.common(key),
        };
        Ok(cmd)
    }

    fn vi_delete_motion<R: RawReader>(&mut self,
                                      rdr: &mut R,
                                      config: &Config,
                                      key: KeyPress)
                                      -> Result<Cmd> {
        let mut mvt = try!(rdr.next_key(config.keyseq_timeout()));
        if mvt == key {
            return Ok(Cmd::KillWholeLine);
        }
        if let KeyPress::Char(digit @ '1'...'9') = mvt {
            // vi-arg-digit
            mvt = try!(self.vi_arg_digit(rdr, config, digit));
        }
        Ok(match mvt {
            KeyPress::Char('$') => Cmd::KillLine, // vi-change-to-eol: Vi change to end of line.
            KeyPress::Char('0') => Cmd::UnixLikeDiscard, // vi-kill-line-prev: Vi cut from beginning of line to cursor.
            KeyPress::Char('b') => Cmd::BackwardKillWord(self.num_args(), Word::Vi),
            KeyPress::Char('B') => Cmd::BackwardKillWord(self.num_args(), Word::Big),
            KeyPress::Char('e') => Cmd::KillWord(self.num_args(), At::End, Word::Vi),
            KeyPress::Char('E') => Cmd::KillWord(self.num_args(), At::End, Word::Big),
            KeyPress::Char(c) if c == 'f' || c == 'F' || c == 't' || c == 'T' => {
                let cs = try!(self.vi_char_search(rdr, config, c));
                match cs {
                    Some(cs) => Cmd::ViDeleteTo(self.num_args(), cs),
                    None => Cmd::Unknown,
                }
            }
            KeyPress::Char('h') |
            KeyPress::Ctrl('H') |
            KeyPress::Backspace => Cmd::BackwardDeleteChar(self.num_args()), // vi-delete-prev-char: Vi move to previous character (backspace).
            KeyPress::Char('l') |
            KeyPress::Char(' ') => Cmd::DeleteChar(self.num_args()),
            KeyPress::Char('w') => Cmd::KillWord(self.num_args(), At::Start, Word::Vi),
            KeyPress::Char('W') => Cmd::KillWord(self.num_args(), At::Start, Word::Big),
            _ => Cmd::Unknown,
        })
    }

    fn vi_char_search<R: RawReader>(&mut self,
                                    rdr: &mut R,
                                    config: &Config,
                                    cmd: char)
                                    -> Result<Option<CharSearch>> {
        let ch = try!(rdr.next_key(config.keyseq_timeout()));
        Ok(match ch {
            KeyPress::Char(ch) => {
                Some(match cmd {
                    'f' => CharSearch::Forward(ch),
                    't' => CharSearch::ForwardBefore(ch),
                    'F' => CharSearch::Backward(ch),
                    'T' => CharSearch::BackwardAfter(ch),
                    _ => unreachable!(),
                })
            }
            _ => None,
        })
    }

    fn common(&mut self, key: KeyPress) -> Cmd {
        match key {
            KeyPress::Home => Cmd::BeginningOfLine,
            KeyPress::Left => Cmd::BackwardChar(self.num_args()),
            KeyPress::Ctrl('C') => Cmd::Interrupt,
            KeyPress::Ctrl('D') => Cmd::EndOfFile,
            KeyPress::Delete => Cmd::DeleteChar(self.num_args()),
            KeyPress::End => Cmd::EndOfLine,
            KeyPress::Right => Cmd::ForwardChar(self.num_args()),
            KeyPress::Ctrl('J') |
            KeyPress::Enter => Cmd::AcceptLine,
            KeyPress::Down => Cmd::NextHistory,
            KeyPress::Up => Cmd::PreviousHistory,
            KeyPress::Ctrl('R') => Cmd::ReverseSearchHistory,
            KeyPress::Ctrl('S') => Cmd::ForwardSearchHistory, // most terminals override Ctrl+S to suspend execution
            KeyPress::Ctrl('T') => Cmd::TransposeChars,
            KeyPress::Ctrl('U') => Cmd::UnixLikeDiscard,
            KeyPress::Ctrl('Q') | // most terminals override Ctrl+Q to resume execution
            KeyPress::Ctrl('V') => Cmd::QuotedInsert,
            KeyPress::Ctrl('W') => Cmd::BackwardKillWord(self.num_args(), Word::Big),
            KeyPress::Ctrl('Y') => Cmd::Yank(self.num_args()),
            KeyPress::Ctrl('Z') => Cmd::Suspend,
            KeyPress::UnknownEscSeq => Cmd::Noop,
            _ => Cmd::Unknown,
        }
    }

    fn num_args(&mut self) -> i32 {
        let num_args = match self.num_args {
            0 => 1,
            _ => self.num_args,
        };
        self.num_args = 0;
        num_args
    }
}
