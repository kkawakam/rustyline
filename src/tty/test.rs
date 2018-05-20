//! Tests specific definitions
use std::cell::Cell;
use std::iter::IntoIterator;
use std::rc::Rc;
use std::slice::Iter;
use std::vec::IntoIter;

use super::{truncate, Position, RawMode, RawReader, Renderer, Term};
use config::Config;
use consts::KeyPress;
use error::ReadlineError;
use line_buffer::LineBuffer;
use Result;

pub type Mode = ();

impl RawMode for Mode {
    fn disable_raw_mode(&self) -> Result<()> {
        Ok(())
    }
}

impl<'a> RawReader for Iter<'a, KeyPress> {
    fn next_key(&mut self, _: bool) -> Result<KeyPress> {
        match self.next() {
            Some(key) => Ok(*key),
            None => Err(ReadlineError::Eof),
        }
    }
    #[cfg(unix)]
    fn next_char(&mut self) -> Result<char> {
        unimplemented!();
    }
}

impl RawReader for IntoIter<KeyPress> {
    fn next_key(&mut self, _: bool) -> Result<KeyPress> {
        match self.next() {
            Some(key) => Ok(key),
            None => Err(ReadlineError::Eof),
        }
    }
    #[cfg(unix)]
    fn next_char(&mut self) -> Result<char> {
        match self.next() {
            Some(KeyPress::Char(c)) => Ok(c),
            None => Err(ReadlineError::Eof),
            _ => unimplemented!(),
        }
    }
}

pub struct Sink {
    cursor: Rc<Cell<usize>>, // cursor position before last command
    last: usize,
}

impl Sink {
    pub fn new() -> Sink {
        Sink {
            cursor: Rc::new(Cell::new(0)),
            last: 0,
        }
    }
}

impl Renderer for Sink {
    fn move_cursor(&mut self, _: Position, new: Position) -> Result<()> {
        self.cursor.replace(self.last);
        self.last = new.col;
        Ok(())
    }

    fn refresh_line(
        &mut self,
        _: &str,
        prompt_size: Position,
        line: &LineBuffer,
        hint: Option<String>,
        _: usize,
        _: usize,
    ) -> Result<(Position, Position)> {
        let cursor = self.calculate_position(&line[..line.pos()], prompt_size);
        self.last = cursor.col;
        if let Some(hint) = hint {
            truncate(&hint, 0, 80);
        }
        let end = self.calculate_position(&line, prompt_size);
        Ok((cursor, end))
    }

    fn calculate_position(&self, s: &str, orig: Position) -> Position {
        let mut pos = orig;
        pos.col += s.len();
        pos
    }

    fn write_and_flush(&mut self, _: &[u8]) -> Result<()> {
        Ok(())
    }

    fn beep(&mut self) -> Result<()> {
        Ok(())
    }

    fn clear_screen(&mut self) -> Result<()> {
        Ok(())
    }

    fn sigwinch(&self) -> bool {
        false
    }
    fn update_size(&mut self) {}
    fn get_columns(&self) -> usize {
        80
    }
    fn get_rows(&self) -> usize {
        24
    }
}

pub type Terminal = DummyTerminal;

#[derive(Clone, Debug)]
pub struct DummyTerminal {
    pub keys: Vec<KeyPress>,
    pub cursor: Rc<Cell<usize>>, // cursor position before last command
}

impl Term for DummyTerminal {
    type Reader = IntoIter<KeyPress>;
    type Writer = Sink;
    type Mode = Mode;

    fn new() -> DummyTerminal {
        DummyTerminal {
            keys: Vec::new(),
            cursor: Rc::new(Cell::new(0)),
        }
    }

    // Init checks:

    fn is_unsupported(&self) -> bool {
        false
    }

    fn is_stdin_tty(&self) -> bool {
        true
    }

    // Interactive loop:

    fn enable_raw_mode(&self) -> Result<Mode> {
        Ok(())
    }

    fn create_reader(&self, _: &Config) -> Result<IntoIter<KeyPress>> {
        Ok(self.keys.clone().into_iter())
    }

    fn create_writer(&self) -> Sink {
        Sink {
            cursor: self.cursor.clone(),
            last: 0,
        }
    }
}

#[cfg(unix)]
pub fn suspend() -> Result<()> {
    Ok(())
}
