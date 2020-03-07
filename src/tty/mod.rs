//! This module implements and describes common TTY methods & traits
use std::io::Write;

use crate::config::{BellStyle, ColorMode, Config, OutputStreamType};
use crate::highlight::Highlighter;
use crate::keys::KeyPress;
use crate::layout::{Layout, Position};
use crate::line_buffer::LineBuffer;
use crate::Result;

/// Terminal state
pub trait RawMode: Sized {
    /// Disable RAW mode for the terminal.
    fn disable_raw_mode(&self) -> Result<()>;
}

/// Input event
pub enum Event {
    KeyPress(KeyPress),
    ExternalPrint(String),
}

/// Translate bytes read from stdin to keys.
pub trait RawReader {
    /// Blocking wait for either a key press or an external print
    fn wait_for_input(&mut self, single_esc_abort: bool) -> Result<Event>; // TODO replace calls to `next_key` by `wait_for_input` where relevant
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
    ) -> Result<()>;

    /// Calculate the number of columns and rows used to display `s` on a
    /// `cols` width terminal starting at `orig`.
    fn calculate_position(&self, s: &str, orig: Position) -> Position;

    fn write_and_flush(&self, buf: &[u8]) -> Result<()>;

    /// Beep, used for completion when there is nothing to complete or when all
    /// the choices were already shown.
    fn beep(&mut self) -> Result<()>;

    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self) -> Result<()>;
    /// Clear rows used by prompt and edited line
    fn clear_rows(&mut self, layout: &Layout) -> Result<()>;

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
    ) -> Result<()> {
        (**self).refresh_line(prompt, line, hint, old_layout, new_layout, highlighter)
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

    fn clear_rows(&mut self, layout: &Layout) -> Result<()> {
        (**self).clear_rows(layout)
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

/// Terminal contract
pub trait Term {
    type Reader: RawReader; // rl_instream
    type Writer: Renderer<Reader = Self::Reader>; // rl_outstream
    type Mode: RawMode;
    type ExternalPrinter: Write;

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
    /// Create an external printer
    fn create_external_printer(&mut self) -> Result<Self::ExternalPrinter>;
}

#[cfg(not(any(test, target_arch = "wasm32")))]
fn add_prompt_and_highlight(
    buffer: &mut String,
    highlighter: Option<&dyn Highlighter>,
    line: &LineBuffer,
    prompt: &str,
    default_prompt: bool,
    layout: &Layout,
    cursor: &mut Position,
) {
    use crate::highlight::{split_highlight, PromptInfo};

    if let Some(highlighter) = highlighter {
        if highlighter.has_continuation_prompt() {
            if &line[..] == "" {
                // line.lines() is an empty iterator for empty line so
                // we need to treat it as a special case
                let prompt = highlighter.highlight_prompt(
                    prompt,
                    PromptInfo {
                        default: default_prompt,
                        offset: 0,
                        cursor: Some(0),
                        input: "",
                        line: "",
                        line_no: 0,
                    },
                );
                buffer.push_str(&prompt);
            } else {
                let highlighted = highlighter.highlight(line, line.pos());
                let lines = line.split('\n');
                let mut highlighted_left = highlighted.to_string();
                let mut offset = 0;
                for (line_no, orig) in lines.enumerate() {
                    let (hl, tail) = split_highlight(&highlighted_left, orig.len() + 1);
                    let prompt = highlighter.highlight_prompt(
                        prompt,
                        PromptInfo {
                            default: default_prompt,
                            offset,
                            cursor: if line.pos() > offset && line.pos() < orig.len() {
                                Some(line.pos() - offset)
                            } else {
                                None
                            },
                            input: line,
                            line: orig,
                            line_no,
                        },
                    );
                    buffer.push_str(&prompt);
                    buffer.push_str(&hl);
                    highlighted_left = tail.to_string();
                    offset += orig.len() + 1;
                }
            }
            cursor.col += layout.prompt_size.col;
        } else {
            // display the prompt
            buffer.push_str(&highlighter.highlight_prompt(
                prompt,
                PromptInfo {
                    default: default_prompt,
                    offset: 0,
                    cursor: Some(line.pos()),
                    input: line,
                    line,
                    line_no: 0,
                },
            ));
            // display the input line
            buffer.push_str(&highlighter.highlight(line, line.pos()));
            // we have to generate our own newline on line wrap
            if layout.end.col == 0 && layout.end.row > 0 && !buffer.ends_with('\n') {
                buffer.push_str("\n");
            }
            if cursor.row == 0 {
                cursor.col += layout.prompt_size.col;
            }
        }
    } else {
        // display the prompt
        buffer.push_str(prompt);
        // display the input line
        buffer.push_str(line);
        // we have to generate our own newline on line wrap
        if layout.end.col == 0 && layout.end.row > 0 && !buffer.ends_with('\n') {
            buffer.push_str("\n");
        }
        if cursor.row == 0 {
            cursor.col += layout.prompt_size.col;
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(any(test, target_arch = "wasm32"))] {
        mod test;
        pub use self::test::*;
    } else if #[cfg(windows)] {
        // If on Windows platform import Windows TTY module
        // and re-export into mod.rs scope
        mod windows;
        pub use self::windows::*;
    } else if #[cfg(unix)] {
        // If on Unix platform import Unix TTY module
        // and re-export into mod.rs scope
        mod unix;
        pub use self::unix::*;
    }
}
