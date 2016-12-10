use super::Config;
use super::EditMode;
use super::KeyPress;
use super::RawReader;
use super::Result;

//#[derive(Clone)]
pub enum Cmd {
    Abort, // Miscellaneous Command
    AcceptLine, // Command For History
    BackwardChar, // Command For Moving
    BackwardDeleteChar, // Command For Text
    BackwardKillWord, // Command For Killing
    BackwardWord, // Command For Moving
    BeginningOfHistory, // Command For History
    BeginningOfLine, // Command For Moving
    CapitalizeWord, // Command For Text
    CharacterSearch(bool), // Miscellaneous Command (TODO Move right to the next occurance of c)
    CharacterSearchBackward(bool), /* Miscellaneous Command (TODO Move left to the previous occurance of c) */
    ClearScreen, // Command For Moving
    Complete, // Command For Completion
    DeleteChar, // Command For Text
    DowncaseWord, // Command For Text
    EndOfFile, // Command For Text
    EndOfHistory, // Command For History
    EndOfLine, // Command For Moving
    ForwardChar, // Command For Moving
    ForwardSearchHistory, // Command For History
    ForwardWord, // Command For Moving
    KillLine, // Command For Killing
    KillWholeLine, // Command For Killing (TODO Delete current line)
    KillWord, // Command For Killing
    NextHistory, // Command For History
    Noop,
    PreviousHistory, // Command For History
    QuotedInsert, // Command For Text
    Replace, // TODO DeleteChar + SelfInsert
    ReverseSearchHistory, // Command For History
    SelfInsert, // Command For Text
    TransposeChars, // Command For Text
    TransposeWords, // Command For Text
    Unknown,
    UnixLikeDiscard, // Command For Killing
    UnixWordRubout, // Command For Killing
    UpcaseWord, // Command For Text
    Yank, // Command For Killing
    YankPop, // Command For Killing
}

// TODO numeric arguments: http://web.mit.edu/gnu/doc/html/rlman_1.html#SEC7
pub struct EditState {
    mode: EditMode,
    // TODO Validate Vi Command, Insert, Visual mode
    insert: bool, // vi only ?
}

impl EditState {
    pub fn new(config: &Config) -> EditState {
        EditState {
            mode: config.edit_mode(),
            insert: true,
        }
    }

    pub fn next_cmd<R: RawReader>(&mut self,
                                  rdr: &mut R,
                                  config: &Config)
                                  -> Result<(KeyPress, Cmd)> {
        match self.mode {
            EditMode::Emacs => self.emacs(rdr, config),
            EditMode::Vi if self.insert => self.vi_insert(rdr, config),
            EditMode::Vi => self.vi_command(rdr, config),
        }
    }

    fn emacs<R: RawReader>(&mut self, rdr: &mut R, config: &Config) -> Result<(KeyPress, Cmd)> {
        let key = try!(rdr.next_key(config.keyseq_timeout()));
        let cmd = match key {
            KeyPress::Char(_) => Cmd::SelfInsert,
            KeyPress::Esc => Cmd::Abort, // TODO Validate
            KeyPress::Ctrl('A') => Cmd::BeginningOfLine,
            KeyPress::Home => Cmd::BeginningOfLine,
            KeyPress::Ctrl('B') => Cmd::BackwardChar,
            KeyPress::Left => Cmd::BackwardChar,
            // KeyPress::Ctrl('D') if s.line.is_empty() => Cmd::EndOfFile,
            KeyPress::Ctrl('D') => Cmd::DeleteChar,
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
            KeyPress::Ctrl('W') => Cmd::UnixWordRubout,
            KeyPress::Ctrl('Y') => Cmd::Yank,
            KeyPress::Meta('\x08') => Cmd::BackwardKillWord,
            KeyPress::Meta('\x7f') => Cmd::BackwardKillWord,
            // KeyPress::Meta('-') => { // digit-argument
            // }
            // KeyPress::Meta('0'...'9') => { // digit-argument
            // }
            KeyPress::Meta('<') => Cmd::BeginningOfHistory,
            KeyPress::Meta('>') => Cmd::EndOfHistory,
            KeyPress::Meta('B') => Cmd::BackwardWord,
            KeyPress::Meta('C') => Cmd::CapitalizeWord,
            KeyPress::Meta('D') => Cmd::KillWord,
            KeyPress::Meta('F') => Cmd::ForwardWord,
            KeyPress::Meta('L') => Cmd::DowncaseWord,
            KeyPress::Meta('T') => Cmd::TransposeWords,
            KeyPress::Meta('U') => Cmd::UpcaseWord,
            KeyPress::Meta('Y') => Cmd::YankPop,
            _ => Cmd::Unknown,
        };
        Ok((key, cmd))
    }

    fn vi_command<R: RawReader>(&mut self,
                                rdr: &mut R,
                                config: &Config)
                                -> Result<(KeyPress, Cmd)> {
        let key = try!(rdr.next_key(config.keyseq_timeout()));
        let cmd = match key {
            KeyPress::Char('$') => Cmd::EndOfLine,
            // TODO KeyPress::Char('%') => Cmd::???, Move to the corresponding opening/closing bracket
            KeyPress::Char('0') => Cmd::BeginningOfLine, // vi-zero: Vi move to the beginning of line.
            // KeyPress::Char('1'...'9') => Cmd::???, // vi-arg-digit
            KeyPress::Char('^') => Cmd::BeginningOfLine, // TODO Move to the first non-blank character of line.
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
            KeyPress::Char('b') => Cmd::BackwardWord,
            // TODO KeyPress::Char('B') => Cmd::???, Move one non-blank word left.
            KeyPress::Char('c') => {
                self.insert = true;
                let mvt = try!(rdr.next_key(config.keyseq_timeout()));
                match mvt {
                    KeyPress::Char('$') => Cmd::KillLine, // vi-change-to-eol: Vi change to end of line.
                    KeyPress::Char('0') => Cmd::UnixLikeDiscard,
                    KeyPress::Char('c') => Cmd::KillWholeLine,
                    // TODO KeyPress::Char('f') => ???,
                    // TODO KeyPress::Char('F') => ???,
                    KeyPress::Char('h') => Cmd::BackwardDeleteChar,
                    KeyPress::Char('l') => Cmd::DeleteChar,
                    KeyPress::Char(' ') => Cmd::DeleteChar,
                    // TODO KeyPress::Char('t') => ???,
                    // TODO KeyPress::Char('T') => ???,
                    KeyPress::Char('w') => Cmd::KillWord,
                    _ => Cmd::Unknown,
                }
            }
            KeyPress::Char('C') => {
                self.insert = true;
                Cmd::KillLine
            }
            KeyPress::Char('d') => {
                let mvt = try!(rdr.next_key(config.keyseq_timeout()));
                match mvt {
                    KeyPress::Char('$') => Cmd::KillLine,
                    KeyPress::Char('0') => Cmd::UnixLikeDiscard, // vi-kill-line-prev: Vi cut from beginning of line to cursor.
                    KeyPress::Char('d') => Cmd::KillWholeLine,
                    // TODO KeyPress::Char('f') => ???,
                    // TODO KeyPress::Char('F') => ???,
                    KeyPress::Char('h') => Cmd::BackwardDeleteChar, // vi-delete-prev-char: Vi move to previous character (backspace).
                    KeyPress::Char('l') => Cmd::DeleteChar,
                    KeyPress::Char(' ') => Cmd::DeleteChar,
                    // TODO KeyPress::Char('t') => ???,
                    // TODO KeyPress::Char('T') => ???,
                    KeyPress::Char('w') => Cmd::KillWord,
                    _ => Cmd::Unknown,
                }
            }
            KeyPress::Char('D') => Cmd::KillLine,
            // TODO KeyPress::Char('e') => Cmd::???, vi-to-end-word: Vi move to the end of the current word. Move to the end of the current word.
            // TODO KeyPress::Char('E') => Cmd::???, vi-end-word: Vi move to the end of the current space delimited word. Move to the end of the current non-blank word.
            KeyPress::Char('i') => {
                // vi-insert: Vi enter insert mode.
                self.insert = true;
                Cmd::Noop
            }
            KeyPress::Char('I') => {
                // vi-insert-at-bol: Vi enter insert mode at the beginning of line.
                self.insert = true;
                Cmd::BeginningOfLine
            }
            KeyPress::Char('f') => {
                // vi-next-char: Vi move to the character specified next.
                let ch = try!(rdr.next_key(config.keyseq_timeout()));
                match ch {
                    KeyPress::Char(_) => return Ok((ch, Cmd::CharacterSearch(false))),
                    _ => Cmd::Unknown,
                }
            }
            KeyPress::Char('F') => {
                // vi-prev-char: Vi move to the character specified previous.
                let ch = try!(rdr.next_key(config.keyseq_timeout()));
                match ch {
                    KeyPress::Char(_) => return Ok((ch, Cmd::CharacterSearchBackward(false))),
                    _ => Cmd::Unknown,
                }
            }
            // TODO KeyPress::Char('G') => Cmd::???, Move to the history line n
            KeyPress::Char('p') => Cmd::Yank, // vi-paste-next: Vi paste previous deletion to the right of the cursor.
            KeyPress::Char('P') => Cmd::Yank, // vi-paste-prev: Vi paste previous deletion to the left of the cursor. TODO Insert the yanked text before the cursor.
            KeyPress::Char('r') => {
                // vi-replace-char: Vi replace character under the cursor with the next character typed.
                let ch = try!(rdr.next_key(config.keyseq_timeout()));
                match ch {
                    KeyPress::Char(_) => return Ok((ch, Cmd::Replace)),
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
            KeyPress::Char('t') => {
                // vi-to-next-char: Vi move up to the character specified next.
                let ch = try!(rdr.next_key(config.keyseq_timeout()));
                match ch {
                    KeyPress::Char(_) => return Ok((ch, Cmd::CharacterSearchBackward(true))),
                    _ => Cmd::Unknown,
                }
            }
            KeyPress::Char('T') => {
                // vi-to-prev-char: Vi move up to the character specified previous.
                let ch = try!(rdr.next_key(config.keyseq_timeout()));
                match ch {
                    KeyPress::Char(_) => return Ok((ch, Cmd::CharacterSearch(true))),
                    _ => Cmd::Unknown,
                }
            }
            // KeyPress::Char('U') => Cmd::???, // revert-line
            KeyPress::Char('w') => Cmd::ForwardWord, // vi-next-word: Vi move to the next word.
            // TODO KeyPress::Char('W') => Cmd::???, // vi-next-space-word: Vi move to the next space delimited word. Move one non-blank word right.
            KeyPress::Char('x') => Cmd::DeleteChar, // vi-delete: TODO move backward if eol
            KeyPress::Char('X') => Cmd::BackwardDeleteChar,
            KeyPress::Home => Cmd::BeginningOfLine,
            KeyPress::Char('h') => Cmd::BackwardChar,
            KeyPress::Left => Cmd::BackwardChar,
            KeyPress::Ctrl('D') => Cmd::EndOfFile,
            KeyPress::Delete => Cmd::DeleteChar,
            KeyPress::End => Cmd::EndOfLine,
            KeyPress::Ctrl('G') => Cmd::Abort,
            KeyPress::Ctrl('H') => Cmd::BackwardChar,
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
            KeyPress::Ctrl('R') => Cmd::ReverseSearchHistory,
            KeyPress::Ctrl('S') => Cmd::ForwardSearchHistory,
            KeyPress::Ctrl('T') => Cmd::TransposeChars,
            KeyPress::Ctrl('U') => Cmd::UnixLikeDiscard,
            KeyPress::Ctrl('V') => Cmd::QuotedInsert,
            KeyPress::Ctrl('W') => Cmd::UnixWordRubout,
            KeyPress::Ctrl('Y') => Cmd::Yank,
            KeyPress::Esc => Cmd::Noop,
            _ => Cmd::Unknown,
        };
        Ok((key, cmd))
    }

    fn vi_insert<R: RawReader>(&mut self, rdr: &mut R, config: &Config) -> Result<(KeyPress, Cmd)> {
        let key = try!(rdr.next_key(config.keyseq_timeout()));
        let cmd = match key {
            KeyPress::Char(_) => Cmd::SelfInsert,
            KeyPress::Home => Cmd::BeginningOfLine,
            KeyPress::Left => Cmd::BackwardChar,
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
            KeyPress::Ctrl('W') => Cmd::UnixWordRubout,
            KeyPress::Ctrl('Y') => Cmd::Yank,
            KeyPress::Esc => {
                // vi-movement-mode/vi-command-mode: Vi enter command mode (use alternative key bindings).
                self.insert = false;
                Cmd::Noop
            }
            _ => Cmd::Unknown,
        };
        Ok((key, cmd))
    }
}
