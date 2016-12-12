use config::Config;
use config::EditMode;
use consts::KeyPress;
use tty::RawReader;
use super::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    Abort, // Miscellaneous Command
    AcceptLine, // Command For History
    BackwardChar, // Command For Moving
    BackwardDeleteChar, // Command For Text
    BackwardKillWord(Word), // Command For Killing
    BackwardWord(Word), // Command For Moving
    BeginningOfHistory, // Command For History
    BeginningOfLine, // Command For Moving
    CapitalizeWord, // Command For Text
    ClearScreen, // Command For Moving
    Complete, // Command For Completion
    DeleteChar, // Command For Text
    DowncaseWord, // Command For Text
    EndOfFile, // Command For Text
    EndOfHistory, // Command For History
    EndOfLine, // Command For Moving
    ForwardChar, // Command For Moving
    ForwardSearchHistory, // Command For History
    ForwardWord(Word), // Command For Moving
    Interrupt,
    KillLine, // Command For Killing
    KillWholeLine, // Command For Killing
    KillWord(Word), // Command For Killing
    NextHistory, // Command For History
    Noop,
    PreviousHistory, // Command For History
    QuotedInsert, // Command For Text
    Replace(char), // TODO DeleteChar + SelfInsert
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
    ViEndWord(Word), // TODO
    ViKillTo(CharSearch), // TODO
    Yank, // Command For Killing
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

// TODO numeric arguments: http://web.mit.edu/gnu/doc/html/rlman_1.html#SEC7
pub struct EditState {
    mode: EditMode,
    // Vi Command/Alternate, Insert/Input mode
    insert: bool, // vi only ?
}

impl EditState {
    pub fn new(config: &Config) -> EditState {
        EditState {
            mode: config.edit_mode(),
            insert: true,
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

    fn emacs<R: RawReader>(&mut self, rdr: &mut R, config: &Config) -> Result<Cmd> {
        let key = try!(rdr.next_key(config.keyseq_timeout()));
        let cmd = match key {
            KeyPress::Char(c) => Cmd::SelfInsert(c),
            KeyPress::Esc => Cmd::Abort, // TODO Validate
            KeyPress::Ctrl('A') => Cmd::BeginningOfLine,
            KeyPress::Home => Cmd::BeginningOfLine,
            KeyPress::Ctrl('B') => Cmd::BackwardChar,
            KeyPress::Left => Cmd::BackwardChar,
            KeyPress::Ctrl('C') => Cmd::Interrupt,
            KeyPress::Ctrl('D') => Cmd::EndOfFile,
            // KeyPress::Ctrl('D') => Cmd::DeleteChar,
            KeyPress::Delete => Cmd::DeleteChar,
            KeyPress::Ctrl('E') => Cmd::EndOfLine,
            KeyPress::End => Cmd::EndOfLine,
            KeyPress::Ctrl('F') => Cmd::ForwardChar,
            KeyPress::Right => Cmd::ForwardChar,
            KeyPress::Ctrl('G') => Cmd::Abort,
            KeyPress::Ctrl('H') => Cmd::BackwardDeleteChar,
            KeyPress::Backspace => Cmd::BackwardDeleteChar,
            KeyPress::Tab => Cmd::Complete,
            KeyPress::Ctrl('J') => Cmd::AcceptLine,
            KeyPress::Enter => Cmd::AcceptLine,
            KeyPress::Ctrl('K') => Cmd::KillLine,
            KeyPress::Ctrl('L') => Cmd::ClearScreen,
            KeyPress::Ctrl('N') => Cmd::NextHistory,
            KeyPress::Down => Cmd::NextHistory,
            KeyPress::Ctrl('P') => Cmd::PreviousHistory,
            KeyPress::Up => Cmd::PreviousHistory,
            KeyPress::Ctrl('Q') => Cmd::QuotedInsert, // most terminals override Ctrl+Q to resume execution
            KeyPress::Ctrl('R') => Cmd::ReverseSearchHistory,
            KeyPress::Ctrl('S') => Cmd::ForwardSearchHistory, // most terminals override Ctrl+S to suspend execution
            KeyPress::Ctrl('T') => Cmd::TransposeChars,
            KeyPress::Ctrl('U') => Cmd::UnixLikeDiscard,
            KeyPress::Ctrl('V') => Cmd::QuotedInsert,
            KeyPress::Ctrl('W') => Cmd::BackwardKillWord(Word::BigWord),
            KeyPress::Ctrl('Y') => Cmd::Yank,
            KeyPress::Ctrl('Z') => Cmd::Suspend,
            KeyPress::Meta('\x08') => Cmd::BackwardKillWord(Word::Word),
            KeyPress::Meta('\x7f') => Cmd::BackwardKillWord(Word::Word),
            // KeyPress::Meta('-') => { // digit-argument
            // }
            // KeyPress::Meta('0'...'9') => { // digit-argument
            // }
            KeyPress::Meta('<') => Cmd::BeginningOfHistory,
            KeyPress::Meta('>') => Cmd::EndOfHistory,
            KeyPress::Meta('B') => Cmd::BackwardWord(Word::Word),
            KeyPress::Meta('C') => Cmd::CapitalizeWord,
            KeyPress::Meta('D') => Cmd::KillWord(Word::Word),
            KeyPress::Meta('F') => Cmd::ForwardWord(Word::Word),
            KeyPress::Meta('L') => Cmd::DowncaseWord,
            KeyPress::Meta('T') => Cmd::TransposeWords,
            KeyPress::Meta('U') => Cmd::UpcaseWord,
            KeyPress::Meta('Y') => Cmd::YankPop,
            KeyPress::UnknownEscSeq => Cmd::Noop,
            _ => Cmd::Unknown,
        };
        Ok(cmd)
    }

    fn vi_command<R: RawReader>(&mut self, rdr: &mut R, config: &Config) -> Result<Cmd> {
        let key = try!(rdr.next_key(config.keyseq_timeout()));
        let cmd = match key {
            KeyPress::Char('$') => Cmd::EndOfLine,
            KeyPress::End => Cmd::EndOfLine,
            // TODO KeyPress::Char('%') => Cmd::???, Move to the corresponding opening/closing bracket
            KeyPress::Char('0') => Cmd::BeginningOfLine, // vi-zero: Vi move to the beginning of line.
            KeyPress::Home => Cmd::BeginningOfLine,
            // KeyPress::Char('1'...'9') => Cmd::???, // vi-arg-digit
            KeyPress::Char('^') => Cmd::BeginningOfLine, // vi-first-print TODO Move to the first non-blank character of line.
            KeyPress::Char('a') => {
                // vi-append-mode: Vi enter insert mode after the cursor.
                self.insert = true;
                Cmd::ForwardChar
            }
            KeyPress::Char('A') => {
                // vi-append-eol: Vi enter insert mode at end of line.
                self.insert = true;
                Cmd::EndOfLine
            }
            KeyPress::Char('b') => Cmd::BackwardWord(Word::ViWord), // vi-prev-word
            KeyPress::Char('B') => Cmd::BackwardWord(Word::BigWord),
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
            KeyPress::Char('e') => Cmd::ViEndWord(Word::ViWord),
            KeyPress::Char('E') => Cmd::ViEndWord(Word::BigWord),
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
            KeyPress::Char('p') => Cmd::Yank, // vi-put
            KeyPress::Char('P') => Cmd::Yank, // vi-put TODO Insert the yanked text before the cursor.
            KeyPress::Char('r') => {
                // vi-replace-char: Vi replace character under the cursor with the next character typed.
                let ch = try!(rdr.next_key(config.keyseq_timeout()));
                match ch {
                    KeyPress::Char(c) => Cmd::Replace(c),
                    KeyPress::Esc => Cmd::Noop,
                    _ => Cmd::Unknown,
                }
            }
            // TODO KeyPress::Char('R') => Cmd::???, vi-replace-mode: Vi enter replace mode. Replaces characters under the cursor. (overwrite-mode)
            KeyPress::Char('s') => {
                // vi-substitute-char: Vi replace character under the cursor and enter insert mode.
                self.insert = true;
                Cmd::DeleteChar
            }
            KeyPress::Char('S') => {
                // vi-substitute-line: Vi substitute entire line.
                self.insert = true;
                Cmd::KillWholeLine
            }
            // KeyPress::Char('U') => Cmd::???, // revert-line
            KeyPress::Char('w') => Cmd::ForwardWord(Word::ViWord), // vi-next-word
            KeyPress::Char('W') => Cmd::ForwardWord(Word::BigWord), // vi-next-word
            KeyPress::Char('x') => Cmd::DeleteChar, // vi-delete: TODO move backward if eol
            KeyPress::Char('X') => Cmd::BackwardDeleteChar, // vi-rubout
            // KeyPress::Char('y') => Cmd::???, // vi-yank-to
            // KeyPress::Char('Y') => Cmd::???, // vi-yank-to
            KeyPress::Char('h') => Cmd::BackwardChar,
            KeyPress::Ctrl('H') => Cmd::BackwardChar,
            KeyPress::Backspace => Cmd::BackwardChar, // TODO Validate
            KeyPress::Left => Cmd::BackwardChar,
            KeyPress::Ctrl('C') => Cmd::Interrupt,
            KeyPress::Ctrl('D') => Cmd::EndOfFile,
            KeyPress::Delete => Cmd::DeleteChar,
            KeyPress::Ctrl('G') => Cmd::Abort,
            KeyPress::Char('l') => Cmd::ForwardChar,
            KeyPress::Char(' ') => Cmd::ForwardChar,
            KeyPress::Right => Cmd::ForwardChar,
            KeyPress::Ctrl('L') => Cmd::ClearScreen,
            KeyPress::Ctrl('J') => Cmd::AcceptLine,
            KeyPress::Enter => Cmd::AcceptLine,
            KeyPress::Char('+') => Cmd::NextHistory,
            KeyPress::Char('j') => Cmd::NextHistory,
            KeyPress::Ctrl('N') => Cmd::NextHistory,
            KeyPress::Down => Cmd::NextHistory,
            KeyPress::Char('-') => Cmd::PreviousHistory,
            KeyPress::Char('k') => Cmd::PreviousHistory,
            KeyPress::Ctrl('P') => Cmd::PreviousHistory,
            KeyPress::Up => Cmd::PreviousHistory,
            KeyPress::Ctrl('K') => Cmd::KillLine,
            KeyPress::Ctrl('Q') => Cmd::QuotedInsert, // most terminals override Ctrl+Q to resume execution
            KeyPress::Ctrl('R') => {
                self.insert = true; // TODO Validate
                Cmd::ReverseSearchHistory
            }
            KeyPress::Ctrl('S') => {
                self.insert = true; // TODO Validate
                Cmd::ForwardSearchHistory
            }
            KeyPress::Ctrl('T') => Cmd::TransposeChars,
            KeyPress::Ctrl('U') => Cmd::UnixLikeDiscard,
            KeyPress::Ctrl('V') => Cmd::QuotedInsert,
            KeyPress::Ctrl('W') => Cmd::KillWord(Word::BigWord),
            KeyPress::Ctrl('Y') => Cmd::Yank,
            KeyPress::Ctrl('Z') => Cmd::Suspend,
            KeyPress::Esc => Cmd::Noop,
            KeyPress::UnknownEscSeq => Cmd::Noop,
            _ => Cmd::Unknown,
        };
        Ok(cmd)
    }

    fn vi_insert<R: RawReader>(&mut self, rdr: &mut R, config: &Config) -> Result<Cmd> {
        let key = try!(rdr.next_key(config.keyseq_timeout()));
        let cmd = match key {
            KeyPress::Char(c) => Cmd::SelfInsert(c),
            KeyPress::Home => Cmd::BeginningOfLine,
            KeyPress::Left => Cmd::BackwardChar,
            KeyPress::Ctrl('C') => Cmd::Interrupt,
            KeyPress::Ctrl('D') => Cmd::EndOfFile, // vi-eof-maybe
            KeyPress::Delete => Cmd::DeleteChar,
            KeyPress::End => Cmd::EndOfLine,
            KeyPress::Right => Cmd::ForwardChar,
            KeyPress::Ctrl('H') => Cmd::BackwardDeleteChar,
            KeyPress::Backspace => Cmd::BackwardDeleteChar,
            KeyPress::Tab => Cmd::Complete,
            KeyPress::Ctrl('J') => Cmd::AcceptLine,
            KeyPress::Enter => Cmd::AcceptLine,
            KeyPress::Down => Cmd::NextHistory,
            KeyPress::Up => Cmd::PreviousHistory,
            KeyPress::Ctrl('R') => Cmd::ReverseSearchHistory,
            KeyPress::Ctrl('S') => Cmd::ForwardSearchHistory,
            KeyPress::Ctrl('T') => Cmd::TransposeChars,
            KeyPress::Ctrl('U') => Cmd::UnixLikeDiscard,
            KeyPress::Ctrl('V') => Cmd::QuotedInsert,
            KeyPress::Ctrl('W') => Cmd::KillWord(Word::BigWord),
            KeyPress::Ctrl('Y') => Cmd::Yank,
            KeyPress::Ctrl('Z') => Cmd::Suspend,
            KeyPress::Esc => {
                // vi-movement-mode/vi-command-mode: Vi enter command mode (use alternative key bindings).
                self.insert = false;
                Cmd::BackwardChar
            }
            KeyPress::UnknownEscSeq => Cmd::Noop,
            _ => Cmd::Unknown,
        };
        Ok(cmd)
    }

    fn vi_delete_motion<R: RawReader>(&mut self,
                                      rdr: &mut R,
                                      config: &Config,
                                      key: KeyPress)
                                      -> Result<Cmd> {
        let mvt = try!(rdr.next_key(config.keyseq_timeout()));
        if mvt == key {
            return Ok(Cmd::KillWholeLine);
        }
        Ok(match mvt {
            KeyPress::Char('$') => Cmd::KillLine, // vi-change-to-eol: Vi change to end of line.
            KeyPress::Char('0') => Cmd::UnixLikeDiscard, // vi-kill-line-prev: Vi cut from beginning of line to cursor.
            KeyPress::Char('b') => Cmd::BackwardKillWord(Word::ViWord),
            KeyPress::Char('B') => Cmd::BackwardKillWord(Word::BigWord),
            KeyPress::Char('e') => Cmd::KillWord(Word::ViWord),
            KeyPress::Char('E') => Cmd::KillWord(Word::BigWord),
            KeyPress::Char(c) if c == 'f' || c == 'F' || c == 't' || c == 'T' => {
                let cs = try!(self.vi_char_search(rdr, config, c));
                match cs {
                    Some(cs) => Cmd::ViKillTo(cs),
                    None => Cmd::Unknown,
                }
            }
            KeyPress::Char('h') => Cmd::BackwardDeleteChar, // vi-delete-prev-char: Vi move to previous character (backspace).
            KeyPress::Ctrl('H') => Cmd::BackwardDeleteChar,
            KeyPress::Backspace => Cmd::BackwardDeleteChar,
            KeyPress::Char('l') => Cmd::DeleteChar,
            KeyPress::Char(' ') => Cmd::DeleteChar,
            KeyPress::Char('w') => Cmd::KillWord(Word::ViWord),
            KeyPress::Char('W') => Cmd::KillWord(Word::BigWord),
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
}
