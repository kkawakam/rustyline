//! This module implements and describes common TTY methods & traits
use std::cmp::{min, max};

use crate::config::{BellStyle, ColorMode, Config, OutputStreamType};
use crate::edit::Prompt;
use crate::highlight::{Highlighter, PromptInfo, split_highlight};
use crate::keys::KeyPress;
use crate::layout::{Layout, Position, Meter};
use crate::line_buffer::LineBuffer;
use crate::Result;

mod screen;

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
        prompt: &Prompt,
        line: &LineBuffer,
        hint: Option<&str>,
        old_layout: &Layout,
        new_layout: &Layout,
        highlighter: Option<&dyn Highlighter>,
    ) -> Result<()>;

    /// Compute layout for rendering prompt + line + some info (either hint,
    /// validation msg, ...). on the screen. Depending on screen width, line
    /// wrapping may be applied.
    fn compute_layout(
        &self,
        prompt: &Prompt,
        line: &LineBuffer,
        info: Option<&str>,
        scroll_top: usize,
    ) -> Layout {
        // calculate the desired position of the cursor
        let pos = line.pos();
        let mut meter = self.meter();
        meter.set_position(prompt.size);
        if prompt.has_continuation {
            meter.left_margin(prompt.size.col);
        };
        let cursor = meter.update(&line[..pos]);
        // calculate the position of the end of the input line
        meter.update(&line[pos..]);
        if let Some(info) = info {
            meter.left_margin(0);
            meter.update(&info);
        }
        let end = meter.get_position();

        let screen_rows = self.get_rows();
        let scroll_top = if screen_rows <= 1 {
            // Single line visible, ugly case but possible
            cursor.row
        } else if screen_rows > end.row {
            // Whole data fits screen
            0
        } else {
            let min_scroll = cursor.row.saturating_sub(screen_rows - 1);
            let max_scroll = min(cursor.row,
                end.row.saturating_sub(screen_rows - 1));
            max(min_scroll, min(max_scroll, scroll_top))
        };

        let new_layout = Layout {
            prompt_size: prompt.size,
            default_prompt: prompt.is_default,
            cursor,
            end,
            scroll_top,
            screen_rows,
        };
        debug_assert!(new_layout.cursor <= new_layout.end);
        new_layout
    }

    fn render_screen(&mut self,
        prompt: &Prompt,
        line: &LineBuffer,
        hint: Option<&str>,
        new_layout: &Layout,
        highlighter: Option<&dyn Highlighter>)
    {
        let rows = self.get_rows();
        let cols = self.get_columns();
        let tab_stop = self.get_tab_stop();
        let mut scr = screen::Screen::new(self.get_buffer(),
            cols, rows, tab_stop, new_layout.scroll_top);
        if let Some(highlighter) = highlighter {
            if highlighter.has_continuation_prompt() {
                let highlighted = highlighter.highlight(line, line.pos());
                let lines = line.split("\n");
                let mut highlighted_left = highlighted.to_string();
                let mut offset = 0;
                for (line_no, orig) in lines.enumerate() {
                    let hl_line_len = highlighted_left.find('\n')
                        .map(|p| p + 1)
                        .unwrap_or(highlighted_left.len());
                    let (hl, tail) = split_highlight(&highlighted_left,
                        hl_line_len);
                    let has_cursor =
                        line.pos() > offset && line.pos() <= orig.len();
                    let prompt = highlighter.highlight_prompt(prompt.text,
                        PromptInfo {
                            is_default: prompt.is_default,
                            offset,
                            cursor: if has_cursor {
                                Some(line.pos() - offset)
                            } else {
                                None
                            },
                            input: line,
                            line: orig,
                            line_no,
                        });
                    scr.add_text(&prompt);
                    scr.add_text(&hl);
                    highlighted_left = tail.to_string();
                    offset += orig.len() + 1;
                }
            } else {
                scr.add_text(&highlighter.highlight_prompt(prompt.text,
                    PromptInfo {
                        is_default: prompt.is_default,
                        offset: 0,
                        cursor: Some(line.pos()),
                        input: line,
                        line: line,
                        line_no: 0,
                    }));
                scr.add_text(&highlighter.highlight(line, line.pos()));
            }
        } else {
            scr.add_text(prompt.text);
            scr.add_text(line);
        }
        // append hint
        if let Some(hint) = hint {
            if let Some(highlighter) = highlighter {
                scr.add_text(&highlighter.highlight_hint(hint));
            } else {
                scr.add_text(hint);
            }
        }
        if new_layout.cursor == new_layout.end &&
           new_layout.cursor.row > scr.get_position().row
        {
            scr.add_text("\n");
        }
    }

    fn meter(&self) -> Meter {
        Meter::new(self.get_columns(), self.get_tab_stop())
    }

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
    /// Get the tab stop in the current terminal.
    fn get_tab_stop(&self) -> usize;
    /// Get the number of rows in the current terminal.
    fn get_rows(&self) -> usize;
    /// Check if output supports colors.
    fn colors_enabled(&self) -> bool;
    /// Returns rendering buffer. Internal.
    fn get_buffer(&mut self) -> &mut String;

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
        prompt: &Prompt,
        line: &LineBuffer,
        hint: Option<&str>,
        old_layout: &Layout,
        new_layout: &Layout,
        highlighter: Option<&dyn Highlighter>,
    ) -> Result<()> {
        (**self).refresh_line(prompt, line, hint, old_layout, new_layout, highlighter)
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

    fn get_tab_stop(&self) -> usize {
        (**self).get_tab_stop()
    }

    fn get_rows(&self) -> usize {
        (**self).get_rows()
    }

    fn get_buffer(&mut self) -> &mut String {
        (**self).get_buffer()
    }

    fn colors_enabled(&self) -> bool {
        (**self).colors_enabled()
    }

    fn move_cursor_at_leftmost(&mut self, rdr: &mut R::Reader) -> Result<()> {
        (**self).move_cursor_at_leftmost(rdr)
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
