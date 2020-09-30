//! Tests specific definitions
use std::iter::IntoIterator;
use std::slice::Iter;
use std::vec::IntoIter;

use super::{RawMode, RawReader, Renderer, Term};
use crate::config::{BellStyle, ColorMode, Config, OutputStreamType};
use crate::error::ReadlineError;
use crate::edit::Prompt;
use crate::highlight::Highlighter;
use crate::keys::KeyPress;
use crate::layout::{Layout, Position};
use crate::line_buffer::LineBuffer;
use crate::Result;

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

    fn read_pasted_text(&mut self) -> Result<String> {
        unimplemented!()
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

    fn read_pasted_text(&mut self) -> Result<String> {
        unimplemented!()
    }
}

pub struct Sink {
    buffer: String
}

impl Sink {
    pub fn new() -> Sink {
        Sink { buffer: String::new() }
    }
}

impl Renderer for Sink {
    type Reader = IntoIter<KeyPress>;

    fn move_cursor(&mut self, _: Position, _: Position) -> Result<()> {
        Ok(())
    }

    fn refresh_line(
        &mut self,
        _prompt: &Prompt,
        _line: &LineBuffer,
        _hint: Option<&str>,
        _old_layout: &Layout,
        _new_layout: &Layout,
        _highlighter: Option<&dyn Highlighter>,
    ) -> Result<()> {
        Ok(())
    }

    fn write_and_flush(&self, _: &[u8]) -> Result<()> {
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

    fn get_tab_stop(&self) -> usize {
        8
    }

    fn get_rows(&self) -> usize {
        24
    }
    fn get_buffer(&mut self) -> &mut String {
        &mut self.buffer
    }

    fn colors_enabled(&self) -> bool {
        false
    }

    fn move_cursor_at_leftmost(&mut self, _: &mut IntoIter<KeyPress>) -> Result<()> {
        Ok(())
    }
}

pub type Terminal = DummyTerminal;

#[derive(Clone, Debug)]
pub struct DummyTerminal {
    pub keys: Vec<KeyPress>,
    pub cursor: usize, // cursor position before last command
    pub color_mode: ColorMode,
    pub bell_style: BellStyle,
}

impl Term for DummyTerminal {
    type Mode = Mode;
    type Reader = IntoIter<KeyPress>;
    type Writer = Sink;

    fn new(
        color_mode: ColorMode,
        _stream: OutputStreamType,
        _tab_stop: usize,
        bell_style: BellStyle,
    ) -> DummyTerminal {
        DummyTerminal {
            keys: Vec::new(),
            cursor: 0,
            color_mode,
            bell_style,
        }
    }

    // Init checks:

    #[cfg(not(target_arch = "wasm32"))]
    fn is_unsupported(&self) -> bool {
        false
    }

    #[cfg(target_arch = "wasm32")]
    fn is_unsupported(&self) -> bool {
        true
    }

    fn is_stdin_tty(&self) -> bool {
        true
    }

    fn is_output_tty(&self) -> bool {
        false
    }

    // Interactive loop:

    fn enable_raw_mode(&mut self) -> Result<Mode> {
        Ok(())
    }

    fn create_reader(&self, _: &Config) -> Result<IntoIter<KeyPress>> {
        Ok(self.keys.clone().into_iter())
    }

    fn create_writer(&self) -> Sink {
        Sink::new()
    }
}

#[cfg(unix)]
pub fn suspend() -> Result<()> {
    Ok(())
}
