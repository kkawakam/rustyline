//! Tests specific definitions
use std::io::{self, Sink, Write};
use std::iter::IntoIterator;
use std::slice::Iter;
use std::vec::IntoIter;

#[cfg(windows)]
use winapi;

use config::Config;
use consts::KeyPress;
use error::ReadlineError;
use Result;
use super::{RawMode, RawReader, Term};

pub type Mode = ();

impl RawMode for Mode {
    fn disable_raw_mode(&self) -> Result<()> {
        Ok(())
    }
}

impl<'a> RawReader for Iter<'a, KeyPress> {
    fn next_key(&mut self) -> Result<KeyPress> {
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
    fn next_key(&mut self) -> Result<KeyPress> {
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
    #[cfg(windows)]
    pub fn get_console_screen_buffer_info(&self) -> Result<winapi::CONSOLE_SCREEN_BUFFER_INFO> {
        let dw_size = winapi::COORD { X: 80, Y: 24 };
        let dw_cursor_osition = winapi::COORD { X: 0, Y: 0 };
        let sr_window = winapi::SMALL_RECT {
            Left: 0,
            Top: 0,
            Right: 0,
            Bottom: 0,
        };
        let info = winapi::CONSOLE_SCREEN_BUFFER_INFO {
            dwSize: dw_size,
            dwCursorPosition: dw_cursor_osition,
            wAttributes: 0,
            srWindow: sr_window,
            dwMaximumWindowSize: dw_size,
        };
        Ok(info)
    }

    #[cfg(windows)]
    pub fn set_console_cursor_position(&mut self, _: winapi::COORD) -> Result<()> {
        Ok(())
    }

    #[cfg(windows)]
    pub fn fill_console_output_character(&mut self,
                                         _: winapi::DWORD,
                                         _: winapi::COORD)
                                         -> Result<()> {
        Ok(())
    }
}

impl Term for DummyTerminal {
    type Reader = IntoIter<KeyPress>;
    type Writer = Sink;
    type Mode = Mode;

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

    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self, _: &mut Write) -> Result<()> {
        Ok(())
    }
}

#[cfg(unix)]
pub fn suspend() -> Result<()> {
    Ok(())
}
