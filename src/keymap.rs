use config::Config;
use config::EditMode;
use consts::KeyPress;
use tty::RawReader;
use super::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    Abort, // Miscellaneous Command
    AcceptLine, // Command For History
    BackwardChar(i32), // Command For Moving
    BackwardDeleteChar(i32), // Command For Text
    BackwardKillWord(i32, Word), // Command For Killing
    BackwardWord(i32, Word), // Command For Moving
    BeginningOfHistory, // Command For History
    BeginningOfLine, // Command For Moving
    CapitalizeWord, // Command For Text
    ClearScreen, // Command For Moving
    Complete, // Command For Completion
    DeleteChar(i32), // Command For Text
    DowncaseWord, // Command For Text
    EndOfFile, // Command For Text
    EndOfHistory, // Command For History
    EndOfLine, // Command For Moving
    ForwardChar(i32), // Command For Moving
    ForwardSearchHistory, // Command For History
    ForwardWord(i32, Word), // Command For Moving
    Interrupt,
    KillLine, // Command For Killing
    KillWholeLine, // Command For Killing
    KillWord(i32, Word), // Command For Killing
    NextHistory, // Command For History
    Noop,
    PreviousHistory, // Command For History
    QuotedInsert, // Command For Text
    Replace(i32, char), // TODO DeleteChar + SelfInsert
    ReverseSearchHistory, // Command For History
    SelfInsert(char), // Command For Text
    Suspend,
    TransposeChars, // Command For Text
    TransposeWords, // Command For Text
    Unknown,
    UnixLikeDiscard, // Command For Killing
    // UnixWordRubout, // = BackwardKillWord(Word::BigWord) Command For Killing
    UpcaseWord, // Command For Text
    ViCharSearch(CharSearch), // TODO
    ViEndWord(i32, Word), // TODO
    ViKillTo(i32, CharSearch), // TODO
    Yank(i32), // Command For Killing
    YankPop, // Command For Killing
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum Word {
    // non-blanks characters
    BigWord,
    // alphanumeric characters
    Word,
    // alphanumeric (and '_') characters
    ViWord,
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
                KeyPress::Char(digit @ '0'...'9') => {
                    self.num_args = self.num_args * 10 + digit.to_digit(10).unwrap() as i32;
                }
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
            KeyPress::Esc => Cmd::Abort, // TODO Validate
            KeyPress::Ctrl('A') => Cmd::BeginningOfLine,
            KeyPress::Ctrl('B') => Cmd::BackwardChar(self.num_args()),
            KeyPress::Ctrl('E') => Cmd::EndOfLine,
            KeyPress::Ctrl('F') => Cmd::ForwardChar(self.num_args()),
            KeyPress::Ctrl('G') => Cmd::Abort,
            KeyPress::Ctrl('H') => Cmd::BackwardDeleteChar(self.num_args()),
            KeyPress::Backspace => Cmd::BackwardDeleteChar(self.num_args()),
            KeyPress::Tab => Cmd::Complete,
            KeyPress::Ctrl('K') => Cmd::KillLine,
            KeyPress::Ctrl('L') => Cmd::ClearScreen,
            KeyPress::Ctrl('N') => Cmd::NextHistory,
            KeyPress::Ctrl('P') => Cmd::PreviousHistory,
            KeyPress::Meta('\x08') => Cmd::BackwardKillWord(self.num_args(), Word::Word),
            KeyPress::Meta('\x7f') => Cmd::BackwardKillWord(self.num_args(), Word::Word),
            KeyPress::Meta('<') => Cmd::BeginningOfHistory,
            KeyPress::Meta('>') => Cmd::EndOfHistory,
            KeyPress::Meta('B') => Cmd::BackwardWord(self.num_args(), Word::Word),
            KeyPress::Meta('C') => Cmd::CapitalizeWord,
            KeyPress::Meta('D') => Cmd::KillWord(self.num_args(), Word::Word),
            KeyPress::Meta('F') => Cmd::ForwardWord(self.num_args(), Word::Word),
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
            KeyPress::Char('$') => Cmd::EndOfLine,
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
            KeyPress::Char('b') => Cmd::BackwardWord(self.num_args(), Word::ViWord), // vi-prev-word
            KeyPress::Char('B') => Cmd::BackwardWord(self.num_args(), Word::BigWord),
            KeyPress::Char('c') => {
                self.insert = true;
                try!(self.vi_delete_motion(rdr, config, key))
            }
            KeyPress::Char('C') => {
                self.insert = true;
                Cmd::KillLine
            }
            KeyPress::Char('d') => try!(self.vi_delete_motion(rdr, config, key)),
            KeyPress::Char('D') => Cmd::KillLine,
            KeyPress::Char('e') => Cmd::ViEndWord(self.num_args(), Word::ViWord),
            KeyPress::Char('E') => Cmd::ViEndWord(self.num_args(), Word::BigWord),
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
                    Some(cs) => Cmd::ViCharSearch(cs),
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
            KeyPress::Char('w') => Cmd::ForwardWord(self.num_args(), Word::ViWord), // vi-next-word FIXME
            KeyPress::Char('W') => Cmd::ForwardWord(self.num_args(), Word::BigWord), // vi-next-word FIXME
            KeyPress::Char('x') => Cmd::DeleteChar(self.num_args()), // vi-delete: TODO move backward if eol
            KeyPress::Char('X') => Cmd::BackwardDeleteChar(self.num_args()), // vi-rubout
            // KeyPress::Char('y') => Cmd::???, // vi-yank-to
            // KeyPress::Char('Y') => Cmd::???, // vi-yank-to
            KeyPress::Char('h') => Cmd::BackwardChar(self.num_args()),
            KeyPress::Ctrl('H') => Cmd::BackwardChar(self.num_args()),
            KeyPress::Backspace => Cmd::BackwardChar(self.num_args()), // TODO Validate
            KeyPress::Ctrl('G') => Cmd::Abort,
            KeyPress::Char('l') => Cmd::ForwardChar(self.num_args()),
            KeyPress::Char(' ') => Cmd::ForwardChar(self.num_args()),
            KeyPress::Ctrl('L') => Cmd::ClearScreen,
            KeyPress::Char('+') => Cmd::NextHistory,
            KeyPress::Char('j') => Cmd::NextHistory,
            KeyPress::Ctrl('N') => Cmd::NextHistory,
            KeyPress::Char('-') => Cmd::PreviousHistory,
            KeyPress::Char('k') => Cmd::PreviousHistory,
            KeyPress::Ctrl('P') => Cmd::PreviousHistory,
            KeyPress::Ctrl('K') => Cmd::KillLine,
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
            KeyPress::Ctrl('H') => Cmd::BackwardDeleteChar(1),
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
            KeyPress::Char('b') => Cmd::BackwardKillWord(self.num_args(), Word::ViWord),
            KeyPress::Char('B') => Cmd::BackwardKillWord(self.num_args(), Word::BigWord),
            KeyPress::Char('e') => Cmd::KillWord(self.num_args(), Word::ViWord),
            KeyPress::Char('E') => Cmd::KillWord(self.num_args(), Word::BigWord),
            KeyPress::Char(c) if c == 'f' || c == 'F' || c == 't' || c == 'T' => {
                let cs = try!(self.vi_char_search(rdr, config, c));
                match cs {
                    Some(cs) => Cmd::ViKillTo(self.num_args(), cs),
                    None => Cmd::Unknown,
                }
            }
            KeyPress::Char('h') => Cmd::BackwardDeleteChar(self.num_args()), // vi-delete-prev-char: Vi move to previous character (backspace).
            KeyPress::Ctrl('H') => Cmd::BackwardDeleteChar(self.num_args()),
            KeyPress::Backspace => Cmd::BackwardDeleteChar(self.num_args()),
            KeyPress::Char('l') => Cmd::DeleteChar(self.num_args()),
            KeyPress::Char(' ') => Cmd::DeleteChar(self.num_args()),
            KeyPress::Char('w') => Cmd::KillWord(self.num_args(), Word::ViWord), // FIXME
            KeyPress::Char('W') => Cmd::KillWord(self.num_args(), Word::BigWord), // FIXME
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
            KeyPress::Ctrl('J') => Cmd::AcceptLine,
            KeyPress::Enter => Cmd::AcceptLine,
            KeyPress::Down => Cmd::NextHistory,
            KeyPress::Up => Cmd::PreviousHistory,
            KeyPress::Ctrl('Q') => Cmd::QuotedInsert, // most terminals override Ctrl+Q to resume execution
            KeyPress::Ctrl('R') => Cmd::ReverseSearchHistory,
            KeyPress::Ctrl('S') => Cmd::ForwardSearchHistory, // most terminals override Ctrl+S to suspend execution
            KeyPress::Ctrl('T') => Cmd::TransposeChars,
            KeyPress::Ctrl('U') => Cmd::UnixLikeDiscard,
            KeyPress::Ctrl('V') => Cmd::QuotedInsert,
            KeyPress::Ctrl('W') => Cmd::BackwardKillWord(self.num_args(), Word::BigWord),
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
