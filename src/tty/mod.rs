//! This module implements and describes common TTY methods & traits
use std::io::{self, Write};

use Result;
use config::Config;
use consts::KeyPress;
use line_buffer::LineBuffer;

/// Terminal state
pub trait RawMode: Copy + Sized {
    /// Disable RAW mode for the terminal.
    fn disable_raw_mode(&self) -> Result<()>;
}

/// Translate bytes read from stdin to keys.
pub trait RawReader: Sized {
    /// Blocking read of key pressed.
    fn next_key(&mut self) -> Result<KeyPress>;
    /// For CTRL-V support
    #[cfg(unix)]
    fn next_char(&mut self) -> Result<char>;
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub col: usize,
    pub row: usize,
}

/// Display prompt, line and cursor in terminal output
pub trait Renderer {
    fn move_cursor(&mut self, old: Position, new: Position) -> Result<()>;

    /// Display prompt, line and cursor in terminal output
    fn refresh_line(
        &mut self,
        prompt: &str,
        prompt_size: Position,
        line: &LineBuffer,
        current_row: usize,
        old_rows: usize,
    ) -> Result<(Position, Position)>;

    /// Calculate the number of columns and rows used to display `s` on a
    /// `cols` width terminal
    /// starting at `orig`.
    fn calculate_position(&self, s: &str, orig: Position) -> Position;

    fn write_and_flush(&mut self, buf: &[u8]) -> Result<()>;

    /// Beep, used for completion when there is nothing to complete or when all
    /// the choices were already shown.
    fn beep(&mut self) -> Result<()> {
        // TODO bell-style
        try!(io::stderr().write_all(b"\x07"));
        try!(io::stderr().flush());
        Ok(())
    }

    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self) -> Result<()>;

    /// Check if a SIGWINCH signal has been received
    fn sigwinch(&self) -> bool;
    /// Update the number of columns/rows in the current terminal.
    fn update_size(&mut self);
    /// Get the number of rows in the current terminal.
    fn get_columns(&self) -> usize;
    /// Get the number of rows in the current terminal.
    fn get_rows(&self) -> usize;
}

impl<'a, R: Renderer + ?Sized> Renderer for &'a mut R {
    fn move_cursor(&mut self, old: Position, new: Position) -> Result<()> {
        (**self).move_cursor(old, new)
    }
    fn refresh_line(
        &mut self,
        prompt: &str,
        prompt_size: Position,
        line: &LineBuffer,
        current_row: usize,
        old_rows: usize,
    ) -> Result<(Position, Position)> {
        (**self).refresh_line(prompt, prompt_size, line, current_row, old_rows)
    }
    fn calculate_position(&self, s: &str, orig: Position) -> Position {
        (**self).calculate_position(s, orig)
    }
    fn write_and_flush(&mut self, buf: &[u8]) -> Result<()> {
        (**self).write_and_flush(buf)
    }
    fn beep(&mut self) -> Result<()> {
        (**self).beep()
    }
    fn clear_screen(&mut self) -> Result<()> {
        (**self).clear_screen()
    }
    fn sigwinch(&self) -> bool {
        (**self).sigwinch()
    }
    fn update_size(&mut self) {
        (**self).update_size()
    }
    fn get_columns(&self) -> usize {
        (**self).get_columns()
    }
    fn get_rows(&self) -> usize {
        (**self).get_rows()
    }
}

/// Terminal contract
pub trait Term {
    type Reader: RawReader;
    type Writer: Renderer;
    type Mode: RawMode;

    fn new() -> Self;
    /// Check if current terminal can provide a rich line-editing user
    /// interface.
    fn is_unsupported(&self) -> bool;
    /// check if stdin is connected to a terminal.
    fn is_stdin_tty(&self) -> bool;
    /// Enable RAW mode for the terminal.
    fn enable_raw_mode(&self) -> Result<Self::Mode>;
    /// Create a RAW reader
    fn create_reader(&self, config: &Config) -> Result<Self::Reader>;
    /// Create a writer
    fn create_writer(&self) -> Self::Writer;
}

// If on Windows platform import Windows TTY module
// and re-export into mod.rs scope
#[cfg(all(windows, not(test)))]
mod windows;
#[cfg(all(windows, not(test)))]
pub use self::windows::*;

// If on Unix platform import Unix TTY module
// and re-export into mod.rs scope
#[cfg(all(unix, not(test)))]
mod unix;
#[cfg(all(unix, not(test)))]
pub use self::unix::*;

#[cfg(test)]
mod test;
#[cfg(test)]
pub use self::test::*;
