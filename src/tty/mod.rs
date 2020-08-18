//! This module implements and describes common TTY methods & traits

use unicode_width::UnicodeWidthStr;

use crate::config::{BellStyle, ColorMode, Config, OutputStreamType};
use crate::highlight::{Highlighter, PromptState};
use crate::keys::KeyPress;
use crate::layout::{Layout, Position};
use crate::line_buffer::LineBuffer;
use crate::Result;

/// Terminal state
pub trait RawMode: Sized {
    /// Disable RAW mode for the terminal.
    fn disable_raw_mode(&self) -> Result<()>;
}

/// Translate bytes read from stdin to keys.
pub trait RawReader {
    /// Blocking read of key pressed.
    fn next_key(&mut self, single_esc_abort: bool) -> Result<KeyPress>;
    /// For CTRL-V support
    #[cfg(unix)]
    fn next_char(&mut self) -> Result<char>;
    /// Bracketed paste
    fn read_pasted_text(&mut self) -> Result<String>;
}

/// Display prompt, line and cursor in terminal output
pub trait Renderer {
    type Reader: RawReader;

    fn move_cursor(&mut self, old: Position, new: Position) -> Result<()>;

    /// Display `prompt`, line and cursor in terminal output
    #[allow(clippy::too_many_arguments)]
    fn refresh_line(
        &mut self,
        prompt: &str,
        line: &LineBuffer,
        hint: Option<&str>,
        old_layout: &Layout,
        new_layout: &Layout,
        highlighter: Option<&dyn Highlighter>,
        prompt_state: PromptState,
    ) -> Result<()>;

    /// Compute layout for rendering prompt + line + some info (either hint,
    /// validation msg, ...). on the screen. Depending on screen width, line
    /// wrapping may be applied.
    fn compute_layout(
        &self,
        prompt_size: Position,
        default_prompt: bool,
        line: &LineBuffer,
        info: Option<&str>,
    ) -> Layout {
        // calculate the desired position of the cursor
        let pos = line.pos();
        let cursor = self.calculate_position(&line[..pos], prompt_size);
        // calculate the position of the end of the input line
        let mut end = if pos == line.len() {
            cursor
        } else {
            self.calculate_position(&line[pos..], cursor)
        };
        if let Some(info) = info {
            end = self.calculate_position(&info, end);
        }

        let new_layout = Layout {
            prompt_size,
            default_prompt,
            cursor,
            end,
        };
        debug_assert!(new_layout.prompt_size <= new_layout.cursor);
        debug_assert!(new_layout.cursor <= new_layout.end);
        new_layout
    }

    /// Calculate the number of columns and rows used to display `s` on a
    /// `cols` width terminal starting at `orig`.
    fn calculate_position(&self, s: &str, orig: Position) -> Position;

    fn write_and_flush(&self, buf: &[u8]) -> Result<()>;

    /// Beep, used for completion when there is nothing to complete or when all
    /// the choices were already shown.
    fn beep(&mut self) -> Result<()>;

    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self) -> Result<()>;

    /// Check if a SIGWINCH signal has been received
    fn sigwinch(&self) -> bool;
    /// Update the number of columns/rows in the current terminal.
    fn update_size(&mut self);
    /// Get the number of columns in the current terminal.
    fn get_columns(&self) -> usize;
    /// Get the number of rows in the current terminal.
    fn get_rows(&self) -> usize;
    /// Check if output supports colors.
    fn colors_enabled(&self) -> bool;

    /// Make sure prompt is at the leftmost edge of the screen
    fn move_cursor_at_leftmost(&mut self, rdr: &mut Self::Reader) -> Result<()>;
}

impl<'a, R: Renderer + ?Sized> Renderer for &'a mut R {
    type Reader = R::Reader;

    fn move_cursor(&mut self, old: Position, new: Position) -> Result<()> {
        (**self).move_cursor(old, new)
    }

    fn refresh_line(
        &mut self,
        prompt: &str,
        line: &LineBuffer,
        hint: Option<&str>,
        old_layout: &Layout,
        new_layout: &Layout,
        highlighter: Option<&dyn Highlighter>,
        prompt_state: PromptState,
    ) -> Result<()> {
        (**self).refresh_line(
            prompt,
            line,
            hint,
            old_layout,
            new_layout,
            highlighter,
            prompt_state,
        )
    }

    fn calculate_position(&self, s: &str, orig: Position) -> Position {
        (**self).calculate_position(s, orig)
    }

    fn write_and_flush(&self, buf: &[u8]) -> Result<()> {
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

    fn colors_enabled(&self) -> bool {
        (**self).colors_enabled()
    }

    fn move_cursor_at_leftmost(&mut self, rdr: &mut R::Reader) -> Result<()> {
        (**self).move_cursor_at_leftmost(rdr)
    }
}

// ignore ANSI escape sequence
fn width(s: &str, esc_seq: &mut u8) -> usize {
    if *esc_seq == 1 {
        if s == "[" {
            // CSI
            *esc_seq = 2;
        } else {
            // two-character sequence
            *esc_seq = 0;
        }
        0
    } else if *esc_seq == 2 {
        if s == ";" || (s.as_bytes()[0] >= b'0' && s.as_bytes()[0] <= b'9') {
            /*} else if s == "m" {
            // last
             *esc_seq = 0;*/
        } else {
            // not supported
            *esc_seq = 0;
        }
        0
    } else if s == "\x1b" {
        *esc_seq = 1;
        0
    } else if s == "\n" {
        0
    } else {
        s.width()
    }
}

/// Terminal contract
pub trait Term {
    type Reader: RawReader; // rl_instream
    type Writer: Renderer<Reader = Self::Reader>; // rl_outstream
    type Mode: RawMode;

    fn new(
        color_mode: ColorMode,
        stream: OutputStreamType,
        tab_stop: usize,
        bell_style: BellStyle,
    ) -> Self;
    /// Check if current terminal can provide a rich line-editing user
    /// interface.
    fn is_unsupported(&self) -> bool;
    /// check if stdin is connected to a terminal.
    fn is_stdin_tty(&self) -> bool;
    /// check if output stream is connected to a terminal.
    fn is_output_tty(&self) -> bool;
    /// Enable RAW mode for the terminal.
    fn enable_raw_mode(&mut self) -> Result<Self::Mode>;
    /// Create a RAW reader
    fn create_reader(&self, config: &Config) -> Result<Self::Reader>;
    /// Create a writer
    fn create_writer(&self) -> Self::Writer;
}

// If on Windows platform import Windows TTY module
// and re-export into mod.rs scope
#[cfg(all(windows, not(target_arch = "wasm32")))]
mod windows;
#[cfg(all(windows, not(target_arch = "wasm32")))]
pub use self::windows::*;

// If on Unix platform import Unix TTY module
// and re-export into mod.rs scope
#[cfg(all(unix, not(target_arch = "wasm32")))]
mod unix;
#[cfg(all(unix, not(target_arch = "wasm32")))]
pub use self::unix::*;

#[cfg(any(test, target_arch = "wasm32"))]
mod test;
#[cfg(any(test, target_arch = "wasm32"))]
pub use self::test::*;
