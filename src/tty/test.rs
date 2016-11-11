//! Tests specific definitions
use std::io::Write;
use std::iter::IntoIterator;
use std::slice::Iter;
use std::vec::IntoIter;

#[cfg(windows)]
use winapi;

use consts::KeyPress;
use ::error::ReadlineError;
use ::Result;
use super::{RawReader, Term};

pub type Mode = ();

pub fn enable_raw_mode() -> Result<Mode> {
    Ok(())
}
pub fn disable_raw_mode(_: Mode) -> Result<()> {
    Ok(())
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

pub type Terminal = DummyTerminal;

#[derive(Clone,Debug)]
pub struct DummyTerminal {
    pub keys: Vec<KeyPress>,
}

impl DummyTerminal {
    /// Create a RAW reader
    pub fn create_reader(&self) -> Result<IntoIter<KeyPress>> {
        Ok(self.keys.clone().into_iter())
    }

    #[cfg(windows)]
    pub fn get_console_screen_buffer_info(&self) -> Result<winapi::CONSOLE_SCREEN_BUFFER_INFO> {
        Ok(info)
    }

    #[cfg(windows)]
    pub fn set_console_cursor_position(&mut self, pos: winapi::COORD) -> Result<()> {
        Ok(())
    }

    #[cfg(windows)]
    pub fn fill_console_output_character(&mut self,
                                         length: winapi::DWORD,
                                         pos: winapi::COORD)
                                         -> Result<()> {
        Ok(())
    }
}

impl Term for DummyTerminal {
    fn new() -> DummyTerminal {
        DummyTerminal { keys: Vec::new() }
    }

    // Init checks:

    /// Check if current terminal can provide a rich line-editing user interface.
    fn is_unsupported(&self) -> bool {
        false
    }

    /// check if stdin is connected to a terminal.
    fn is_stdin_tty(&self) -> bool {
        true
    }

    // Interactive loop:

    /// Get the number of columns in the current terminal.
    fn get_columns(&self) -> usize {
        80
    }

    /// Get the number of rows in the current terminal.
    fn get_rows(&self) -> usize {
        24
    }

    /// Check if a SIGWINCH signal has been received
    fn sigwinch(&self) -> bool {
        false
    }

    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self, _: &mut Write) -> Result<()> {
        Ok(())
    }
}
