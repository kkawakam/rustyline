//! Tests specific definitions
use std::io::{self, Sink, Write};
use std::iter::IntoIterator;
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
        unimplemented!();
    }
}

impl Renderer for Sink {
    fn move_cursor(&mut self, _: Position, _: Position) -> Result<()> {
        Ok(())
    }

    fn refresh_line(
        &mut self,
        prompt: &str,
        prompt_size: Position,
        line: &LineBuffer,
        hint: Option<String>,
        _: usize,
        _: usize,
    ) -> Result<(Position, Position)> {
        try!(self.write_all(prompt.as_bytes()));
        try!(self.write_all(line.as_bytes()));
        if let Some(hint) = hint {
            try!(self.write_all(truncate(&hint, 0, 80).as_bytes()));
        }
        Ok((prompt_size, prompt_size))
    }

    /// Characters with 2 column width are correctly handled (not splitted).
    fn calculate_position(&self, _: &str, orig: Position) -> Position {
        orig
    }

    fn write_and_flush(&mut self, buf: &[u8]) -> Result<()> {
        try!(self.write_all(buf));
        try!(self.flush());
        Ok(())
    }

    fn beep(&mut self) -> Result<()> {
        Ok(())
    }

    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self) -> Result<()> {
        Ok(())
    }

    /// Check if a SIGWINCH signal has been received
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
}

impl Term for DummyTerminal {
    type Reader = IntoIter<KeyPress>;
    type Writer = Sink;
    type Mode = Mode;

    fn new() -> DummyTerminal {
        DummyTerminal { keys: Vec::new() }
    }

    // Init checks:

    /// Check if current terminal can provide a rich line-editing user
    /// interface.
    fn is_unsupported(&self) -> bool {
        false
    }

    /// check if stdin is connected to a terminal.
    fn is_stdin_tty(&self) -> bool {
        true
    }

    // Interactive loop:

    fn enable_raw_mode(&self) -> Result<Mode> {
        Ok(())
    }

    /// Create a RAW reader
    fn create_reader(&self, _: &Config) -> Result<IntoIter<KeyPress>> {
        Ok(self.keys.clone().into_iter())
    }

    fn create_writer(&self) -> Sink {
        io::sink()
    }
}

#[cfg(unix)]
pub fn suspend() -> Result<()> {
    Ok(())
}
