use config::Config;
use config::EditMode;
use consts::KeyPress;
use tty::RawReader;
use super::Result;

pub type RepeatCount = usize;

#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    Abort, // Miscellaneous Command
    AcceptLine,
    BackwardChar(RepeatCount),
    BackwardWord(RepeatCount, Word), // Backward until start of word
    BeginningOfHistory,
    BeginningOfLine,
    CapitalizeWord,
    ClearScreen,
    Complete,
    DowncaseWord,
    EndOfFile,
    EndOfHistory,
    EndOfLine,
    ForwardChar(RepeatCount),
    ForwardSearchHistory,
    ForwardWord(RepeatCount, At, Word), // Forward until start/end of word
    Interrupt,
    Kill(Movement),
    NextHistory,
    Noop,
    PreviousHistory,
    QuotedInsert,
    Replace(RepeatCount, char),
    ReverseSearchHistory,
    SelfInsert(RepeatCount, char),
    Suspend,
    TransposeChars,
    TransposeWords(RepeatCount),
    Undo,
    Unknown,
    UpcaseWord,
    ViCharSearch(RepeatCount, CharSearch),
    ViYankTo(Movement),
    Yank(RepeatCount, Anchor),
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
    BeforeEnd,
    AfterEnd,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum Anchor {
    After,
    Before,
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
    num_args: i16,
    last_cmd: Cmd, // vi only
}

#[derive(Debug, Clone, PartialEq)]
pub enum Movement {
    WholeLine, // not really a movement
    BeginningOfLine,
    EndOfLine,
    BackwardWord(RepeatCount, Word),
    ForwardWord(RepeatCount, At, Word),
    ViCharSearch(RepeatCount, CharSearch),
    BackwardChar(RepeatCount),
    ForwardChar(RepeatCount),
}

impl EditState {
    pub fn new(config: &Config) -> EditState {
        EditState {
            mode: config.edit_mode(),
            insert: true,
            num_args: 0,
            last_cmd: Cmd::Noop,
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

    // TODO dynamic prompt (arg: ?)
    fn emacs_digit_argument<R: RawReader>(&mut self,
                                          rdr: &mut R,
                                          config: &Config,
                                          digit: char)
                                          -> Result<KeyPress> {
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
            let key = try!(rdr.next_key(config.keyseq_timeout()));
            match key {
                KeyPress::Char(digit @ '0'...'9') |
                KeyPress::Meta(digit @ '0'...'9') => {
                    if self.num_args == -1 {
                        self.num_args *= digit.to_digit(10).unwrap() as i16;
                    } else {
                        self.num_args = self.num_args
                            .saturating_mul(10)
                            .saturating_add(digit.to_digit(10).unwrap() as i16);
                    }
                }
                _ => return Ok(key),
            };
        }
    }

    fn emacs<R: RawReader>(&mut self, rdr: &mut R, config: &Config) -> Result<Cmd> {
        let mut key = try!(rdr.next_key(config.keyseq_timeout()));
        if let KeyPress::Meta(digit @ '-') = key {
            key = try!(self.emacs_digit_argument(rdr, config, digit));
        } else if let KeyPress::Meta(digit @ '0'...'9') = key {
            key = try!(self.emacs_digit_argument(rdr, config, digit));
        }
        let (n, positive) = self.emacs_num_args(); // consume them in all cases
        let cmd = match key {
            KeyPress::Char(c) => {
                if positive {
                    Cmd::SelfInsert(n, c)
                } else {
                    Cmd::Unknown
                }
            }
            KeyPress::Ctrl('A') => Cmd::BeginningOfLine,
            KeyPress::Ctrl('B') => {
                if positive {
                    Cmd::BackwardChar(n)
                } else {
                    Cmd::ForwardChar(n)
                }
            }
            KeyPress::Ctrl('E') => Cmd::EndOfLine,
            KeyPress::Ctrl('F') => {
                if positive {
                    Cmd::ForwardChar(n)
                } else {
                    Cmd::BackwardChar(n)
                }
            }
            KeyPress::Ctrl('G') |
            KeyPress::Esc => Cmd::Abort,
            KeyPress::Ctrl('H') |
            KeyPress::Backspace => {
                if positive {
                    Cmd::Kill(Movement::BackwardChar(n))
                } else {
                    Cmd::Kill(Movement::ForwardChar(n))
                }
            }
            KeyPress::Tab => Cmd::Complete,
            KeyPress::Ctrl('K') => {
                if positive {
                    Cmd::Kill(Movement::EndOfLine)
                } else {
                    Cmd::Kill(Movement::BeginningOfLine)
                }
            }
            KeyPress::Ctrl('L') => Cmd::ClearScreen,
            KeyPress::Ctrl('N') => Cmd::NextHistory,
            KeyPress::Ctrl('P') => Cmd::PreviousHistory,
            KeyPress::Ctrl('X') => {
                let snd_key = try!(rdr.next_key(config.keyseq_timeout()));
                match snd_key {
                    KeyPress::Ctrl('G') => Cmd::Abort,
                    KeyPress::Ctrl('U') => Cmd::Undo,
                    _ => Cmd::Unknown,
                }
            }
            KeyPress::Ctrl('_') => Cmd::Undo,
            KeyPress::Meta('\x08') |
            KeyPress::Meta('\x7f') => {
                if positive {
                    Cmd::Kill(Movement::BackwardWord(n, Word::Emacs))
                } else {
                    Cmd::Kill(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
                }
            }
            KeyPress::Meta('<') => Cmd::BeginningOfHistory,
            KeyPress::Meta('>') => Cmd::EndOfHistory,
            KeyPress::Meta('B') => {
                if positive {
                    Cmd::BackwardWord(n, Word::Emacs)
                } else {
                    Cmd::ForwardWord(n, At::AfterEnd, Word::Emacs)
                }
            }
            KeyPress::Meta('C') => Cmd::CapitalizeWord,
            KeyPress::Meta('D') => {
                if positive {
                    Cmd::Kill(Movement::ForwardWord(n, At::AfterEnd, Word::Emacs))
                } else {
                    Cmd::Kill(Movement::BackwardWord(n, Word::Emacs))
                }
            }
            KeyPress::Meta('F') => {
                if positive {
                    Cmd::ForwardWord(n, At::AfterEnd, Word::Emacs)
                } else {
                    Cmd::BackwardWord(n, Word::Emacs)
                }
            }
            KeyPress::Meta('L') => Cmd::DowncaseWord,
            KeyPress::Meta('T') => Cmd::TransposeWords(n),
            KeyPress::Meta('U') => Cmd::UpcaseWord,
            KeyPress::Meta('Y') => Cmd::YankPop,
            _ => self.common(key, n, positive),
        };
        debug!(target: "rustyline", "Emacs command: {:?}", cmd);
        Ok(cmd)
    }

    fn vi_arg_digit<R: RawReader>(&mut self,
                                  rdr: &mut R,
                                  config: &Config,
                                  digit: char)
                                  -> Result<KeyPress> {
        self.num_args = digit.to_digit(10).unwrap() as i16;
        loop {
            let key = try!(rdr.next_key(config.keyseq_timeout()));
            match key {
                KeyPress::Char(digit @ '0'...'9') => {
                    self.num_args = self.num_args
                        .saturating_mul(10)
                        .saturating_add(digit.to_digit(10).unwrap() as i16);
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
        let n = self.vi_num_args(); // consume them in all cases
        let cmd = match key {
            KeyPress::Char('$') |
            KeyPress::End => Cmd::EndOfLine,
            KeyPress::Char('.') => self.last_cmd.clone(), // vi-redo
            // TODO KeyPress::Char('%') => Cmd::???, Move to the corresponding opening/closing bracket
            KeyPress::Char('0') => Cmd::BeginningOfLine, // vi-zero: Vi move to the beginning of line.
            KeyPress::Char('^') => Cmd::BeginningOfLine, // vi-first-print TODO Move to the first non-blank character of line.
            KeyPress::Char('a') => {
                // vi-append-mode: Vi enter insert mode after the cursor.
                self.insert = true;
                Cmd::ForwardChar(n)
            }
            KeyPress::Char('A') => {
                // vi-append-eol: Vi enter insert mode at end of line.
                self.insert = true;
                Cmd::EndOfLine
            }
            KeyPress::Char('b') => Cmd::BackwardWord(n, Word::Vi), // vi-prev-word
            KeyPress::Char('B') => Cmd::BackwardWord(n, Word::Big),
            KeyPress::Char('c') => {
                self.insert = true;
                match try!(self.vi_cmd_motion(rdr, config, key, n)) {
                    Some(mvt) => Cmd::Kill(mvt),
                    None => Cmd::Unknown,
                }
            }
            KeyPress::Char('C') => {
                self.insert = true;
                Cmd::Kill(Movement::EndOfLine)
            }
            KeyPress::Char('d') => {
                match try!(self.vi_cmd_motion(rdr, config, key, n)) {
                    Some(mvt) => Cmd::Kill(mvt),
                    None => Cmd::Unknown,
                }
            }
            KeyPress::Char('D') |
            KeyPress::Ctrl('K') => Cmd::Kill(Movement::EndOfLine),
            KeyPress::Char('e') => Cmd::ForwardWord(n, At::BeforeEnd, Word::Vi),
            KeyPress::Char('E') => Cmd::ForwardWord(n, At::BeforeEnd, Word::Big),
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
                    Some(cs) => Cmd::ViCharSearch(n, cs),
                    None => Cmd::Unknown,
                }
            }
            // TODO KeyPress::Char('G') => Cmd::???, Move to the history line n
            KeyPress::Char('p') => Cmd::Yank(n, Anchor::After), // vi-put
            KeyPress::Char('P') => Cmd::Yank(n, Anchor::Before), // vi-put
            KeyPress::Char('r') => {
                // vi-replace-char: Vi replace character under the cursor with the next character typed.
                let ch = try!(rdr.next_key(config.keyseq_timeout()));
                match ch {
                    KeyPress::Char(c) => Cmd::Replace(n, c),
                    KeyPress::Esc => Cmd::Noop,
                    _ => Cmd::Unknown,
                }
            }
            // TODO KeyPress::Char('R') => Cmd::???, vi-replace-mode: Vi enter replace mode. Replaces characters under the cursor. (overwrite-mode)
            KeyPress::Char('s') => {
                // vi-substitute-char: Vi replace character under the cursor and enter insert mode.
                self.insert = true;
                Cmd::Kill(Movement::ForwardChar(n))
            }
            KeyPress::Char('S') => {
                // vi-substitute-line: Vi substitute entire line.
                self.insert = true;
                Cmd::Kill(Movement::WholeLine)
            }
            KeyPress::Char('u') => Cmd::Undo,
            // KeyPress::Char('U') => Cmd::???, // revert-line
            KeyPress::Char('w') => Cmd::ForwardWord(n, At::Start, Word::Vi), // vi-next-word
            KeyPress::Char('W') => Cmd::ForwardWord(n, At::Start, Word::Big), // vi-next-word
            KeyPress::Char('x') => Cmd::Kill(Movement::ForwardChar(n)), // vi-delete: TODO move backward if eol
            KeyPress::Char('X') => Cmd::Kill(Movement::BackwardChar(n)), // vi-rubout
            KeyPress::Char('y') => {
                match try!(self.vi_cmd_motion(rdr, config, key, n)) {
                    Some(mvt) => Cmd::ViYankTo(mvt),
                    None => Cmd::Unknown,
                }
            }
            // KeyPress::Char('Y') => Cmd::???, // vi-yank-to
            KeyPress::Char('h') |
            KeyPress::Ctrl('H') |
            KeyPress::Backspace => Cmd::BackwardChar(n), // TODO Validate
            KeyPress::Ctrl('G') => Cmd::Abort,
            KeyPress::Char('l') |
            KeyPress::Char(' ') => Cmd::ForwardChar(n),
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
            _ => self.common(key, n, true),
        };
        debug!(target: "rustyline", "Vi command: {:?}", cmd);
        if key != KeyPress::Char('.') {
            self.last_cmd = cmd.clone();
        }
        Ok(cmd)
    }

    fn vi_insert<R: RawReader>(&mut self, rdr: &mut R, config: &Config) -> Result<Cmd> {
        let key = try!(rdr.next_key(config.keyseq_timeout()));
        let cmd = match key {
            KeyPress::Char(c) => Cmd::SelfInsert(1, c),
            KeyPress::Ctrl('H') |
            KeyPress::Backspace => Cmd::Kill(Movement::BackwardChar(1)),
            KeyPress::Tab => Cmd::Complete,
            KeyPress::Esc => {
                // vi-movement-mode/vi-command-mode: Vi enter command mode (use alternative key bindings).
                self.insert = false;
                Cmd::BackwardChar(1)
            }
            _ => self.common(key, 1, true),
        };
        debug!(target: "rustyline", "Vi insert: {:?}", cmd);
        self.last_cmd = cmd.clone();
        Ok(cmd)
    }

    fn vi_cmd_motion<R: RawReader>(&mut self,
                                   rdr: &mut R,
                                   config: &Config,
                                   key: KeyPress,
                                   n: RepeatCount)
                                   -> Result<Option<Movement>> {
        let mut mvt = try!(rdr.next_key(config.keyseq_timeout()));
        if mvt == key {
            return Ok(Some(Movement::WholeLine));
        }
        let mut n = n;
        if let KeyPress::Char(digit @ '1'...'9') = mvt {
            // vi-arg-digit
            mvt = try!(self.vi_arg_digit(rdr, config, digit));
            n = self.vi_num_args();
        }
        Ok(match mvt {
            KeyPress::Char('$') => Some(Movement::EndOfLine), // vi-change-to-eol: Vi change to end of line.
            KeyPress::Char('0') => Some(Movement::BeginningOfLine), // vi-kill-line-prev: Vi cut from beginning of line to cursor.
            KeyPress::Char('b') => Some(Movement::BackwardWord(n, Word::Vi)),
            KeyPress::Char('B') => Some(Movement::BackwardWord(n, Word::Big)),
            KeyPress::Char('e') => Some(Movement::ForwardWord(n, At::AfterEnd, Word::Vi)),
            KeyPress::Char('E') => Some(Movement::ForwardWord(n, At::AfterEnd, Word::Big)),
            KeyPress::Char(c) if c == 'f' || c == 'F' || c == 't' || c == 'T' => {
                let cs = try!(self.vi_char_search(rdr, config, c));
                match cs {
                    Some(cs) => Some(Movement::ViCharSearch(n, cs)),
                    None => None,
                }
            }
            KeyPress::Char('h') |
            KeyPress::Ctrl('H') |
            KeyPress::Backspace => Some(Movement::BackwardChar(n)), // vi-delete-prev-char: Vi move to previous character (backspace).
            KeyPress::Char('l') |
            KeyPress::Char(' ') => Some(Movement::ForwardChar(n)),
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

    fn common(&mut self, key: KeyPress, n: RepeatCount, positive: bool) -> Cmd {
        match key {
            KeyPress::Home => Cmd::BeginningOfLine,
            KeyPress::Left => {
                if positive {
                    Cmd::BackwardChar(n)
                } else {
                    Cmd::ForwardChar(n)
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
            KeyPress::End => Cmd::EndOfLine,
            KeyPress::Right => {
                if positive {
                    Cmd::ForwardChar(n)
                } else {
                    Cmd::BackwardChar(n)
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
