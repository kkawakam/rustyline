//! This module implements and describes common TTY methods & traits

use unicode_width::UnicodeWidthStr;

use crate::config::{BellStyle, ColorMode, Config, OutputStreamType};
use crate::edit::Prompt;
use crate::highlight::{Highlighter, PromptInfo, split_highlight};
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
    ) -> Layout {
        // calculate the desired position of the cursor
        let pos = line.pos();
        let left_margin = if prompt.has_continuation {
            prompt.size.col
        } else {
            0
        };
        let cursor = self.calculate_position(&line[..pos],
            prompt.size, left_margin);
        // calculate the position of the end of the input line
        let mut end = if pos == line.len() {
            cursor
        } else {
            self.calculate_position(&line[pos..], cursor, left_margin)
        };
        if let Some(info) = info {
            end = self.calculate_position(&info, end, left_margin);
        }

        let new_layout = Layout {
            prompt_size: prompt.size,
            left_margin,
            default_prompt: prompt.is_default,
            cursor,
            end,
        };
        debug_assert!(new_layout.cursor <= new_layout.end);
        new_layout
    }

    /// Calculate the number of columns and rows used to display `s` on a
    /// `cols` width terminal starting at `orig`.
    fn calculate_position(&self, s: &str, orig: Position, left_margin: usize)
        -> Position;

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
        prompt: &Prompt,
        line: &LineBuffer,
        hint: Option<&str>,
        old_layout: &Layout,
        new_layout: &Layout,
        highlighter: Option<&dyn Highlighter>,
    ) -> Result<()> {
        (**self).refresh_line(prompt, line, hint, old_layout, new_layout, highlighter)
    }

    fn calculate_position(&self, s: &str, orig: Position, left_margin: usize)
        -> Position
    {
        (**self).calculate_position(s, orig, left_margin)
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

fn add_prompt_and_highlight<F>(
    mut push_str: F, highlighter: Option<&dyn Highlighter>,
    line: &LineBuffer, prompt: &Prompt)
    where F: FnMut(&str),
{
    if let Some(highlighter) = highlighter {
        if prompt.has_continuation {
            if &line[..] == "" {
                // line.lines() is an empty iterator for empty line so
                // we need to treat it as a special case
                let prompt = highlighter.highlight_prompt(prompt.text,
                    PromptInfo {
                        is_default: prompt.is_default,
                        offset: 0,
                        cursor: Some(0),
                        input: "",
                        line: "",
                        line_no: 0,
                    });
                push_str(&prompt);
            } else {
                let highlighted = highlighter.highlight(line, line.pos());
                let lines = line.split('\n');
                let mut highlighted_left = highlighted.to_string();
                let mut offset = 0;
                for (line_no, orig) in lines.enumerate() {
                    let (hl, tail) = split_highlight(&highlighted_left,
                        orig.len()+1);
                    let has_cursor =
                        line.pos() > offset && line.pos() < orig.len();
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
                    push_str(&prompt);
                    push_str(&hl);
                    highlighted_left = tail.to_string();
                    offset += orig.len() + 1;
                }
            }
        } else {
            // display the prompt
            push_str(&highlighter.highlight_prompt(prompt.text,
                PromptInfo {
                    is_default: prompt.is_default,
                    offset: 0,
                    cursor: Some(line.pos()),
                    input: line,
                    line: line,
                    line_no: 0,
                }));
            // display the input line
            push_str(&highlighter.highlight(line, line.pos()));
        }
    } else {
        // display the prompt
        push_str(prompt.text);
        // display the input line
        push_str(line);
    }
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
