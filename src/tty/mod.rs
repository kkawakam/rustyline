//! This module implements and describes common TTY methods & traits

/// Unsupported Terminals that don't support RAW mode
const UNSUPPORTED_TERM: [&str; 3] = ["dumb", "cons25", "emacs"];

use crate::config::Config;
use crate::highlight::Highlighter;
use crate::keys::KeyEvent;
use crate::layout::{GraphemeClusterMode, Layout, Position, Unit};
use crate::line_buffer::LineBuffer;
use crate::{Cmd, Result};

/// Terminal state
pub trait RawMode: Sized {
    /// Disable RAW mode for the terminal.
    fn disable_raw_mode(&self) -> Result<()>;
}

/// Input event
pub enum Event {
    KeyPress(KeyEvent),
    ExternalPrint(String),
    #[cfg(target_os = "macos")]
    Timeout(bool),
}

/// Translate bytes read from stdin to keys.
pub trait RawReader {
    type Buffer;
    /// Blocking wait for either a key press or an external print
    fn wait_for_input(&mut self, single_esc_abort: bool) -> Result<Event>; // TODO replace calls to `next_key` by `wait_for_input` where relevant
    /// Blocking read of key pressed.
    fn next_key(&mut self, single_esc_abort: bool) -> Result<KeyEvent>;
    /// For CTRL-V support
    #[cfg(unix)]
    fn next_char(&mut self) -> Result<char>;
    /// Bracketed paste
    fn read_pasted_text(&mut self) -> Result<String>;
    /// Check if `key` is bound to a peculiar command
    fn find_binding(&self, key: &KeyEvent) -> Option<Cmd>;
    /// Backup type ahead
    fn unbuffer(self) -> Option<Buffer>;
}

/// Display prompt, line and cursor in terminal output
pub trait Renderer {
    type Reader: RawReader;

    fn move_cursor(&mut self, old: Position, new: Position) -> Result<()>;

    /// Display `prompt`, line and cursor in terminal output
    fn refresh_line(
        &mut self,
        prompt: &str,
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
            end = self.calculate_position(info, end);
        }

        let new_layout = Layout {
            grapheme_cluster_mode: self.grapheme_cluster_mode(),
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

    fn write_and_flush(&mut self, buf: &str) -> Result<()>;

    /// Beep, used for completion when there is nothing to complete or when all
    /// the choices were already shown.
    fn beep(&mut self) -> Result<()>;

    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self) -> Result<()>;
    /// Clear rows used by prompt and edited line
    fn clear_rows(&mut self, layout: &Layout) -> Result<()>;
    /// Clear from cursor to the end of line
    fn clear_to_eol(&mut self) -> Result<()>;

    /// Update the number of columns/rows in the current terminal.
    fn update_size(&mut self);
    /// Get the number of columns in the current terminal.
    fn get_columns(&self) -> Unit;
    /// Get the number of rows in the current terminal.
    fn get_rows(&self) -> Unit;
    /// Check if output supports colors.
    fn colors_enabled(&self) -> bool;
    /// Tell how grapheme clusters are rendered.
    fn grapheme_cluster_mode(&self) -> GraphemeClusterMode;

    /// Make sure prompt is at the leftmost edge of the screen
    fn move_cursor_at_leftmost(&mut self, rdr: &mut Self::Reader) -> Result<()>;
    /// Begin synchronized update on unix platform
    fn begin_synchronized_update(&mut self) -> Result<()> {
        Ok(())
    }
    /// End synchronized update on unix platform
    fn end_synchronized_update(&mut self) -> Result<()> {
        Ok(())
    }
}

// ignore ANSI escape sequence
fn width(gcm: GraphemeClusterMode, s: &str, esc_seq: &mut u8) -> Unit {
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
        gcm.width(s)
    }
}

/// External printer
pub trait ExternalPrinter {
    /// Print message to stdout
    fn print(&mut self, msg: String) -> Result<()>;
}

/// Terminal contract
pub trait Term {
    type Buffer;
    type KeyMap;
    type Reader: RawReader<Buffer = Self::Buffer>; // rl_instream
    type Writer: Renderer<Reader = Self::Reader>; // rl_outstream
    type Mode: RawMode;
    type ExternalPrinter: ExternalPrinter;
    type CursorGuard;

    fn new(config: &Config) -> Result<Self>
    where
        Self: Sized;
    /// Check if current terminal can provide a rich line-editing user
    /// interface.
    fn is_unsupported(&self) -> bool;
    /// check if input stream is connected to a terminal.
    fn is_input_tty(&self) -> bool;
    /// check if output stream is connected to a terminal.
    fn is_output_tty(&self) -> bool;
    /// Enable RAW mode for the terminal.
    fn enable_raw_mode(&mut self, config: &Config) -> Result<(Self::Mode, Self::KeyMap)>;
    /// Create a RAW reader
    fn create_reader(
        &self,
        buffer: Option<Self::Buffer>,
        config: &Config,
        key_map: Self::KeyMap,
    ) -> Self::Reader;
    /// Create a writer
    fn create_writer(&self, config: &Config) -> Self::Writer;
    fn writeln(&self) -> Result<()>;
    /// Create an external printer
    fn create_external_printer(&mut self) -> Result<Self::ExternalPrinter>;
    /// Change cursor visibility
    fn set_cursor_visibility(&mut self, visible: bool) -> Result<Option<Self::CursorGuard>>;
}

/// Check TERM environment variable to see if current term is in our
/// unsupported list
fn is_unsupported_term() -> bool {
    match std::env::var("TERM") {
        Ok(term) => {
            for iter in &UNSUPPORTED_TERM {
                if (*iter).eq_ignore_ascii_case(&term) {
                    return true;
                }
            }
            false
        }
        Err(_) => false,
    }
}

// If on Windows platform import Windows TTY module
// and re-export into mod.rs scope
#[cfg(all(windows, not(target_arch = "wasm32")))]
mod windows;
#[cfg(all(windows, not(target_arch = "wasm32"), not(test)))]
pub use self::windows::*;

// If on Unix platform import Unix TTY module
// and re-export into mod.rs scope
#[cfg(all(unix, not(target_arch = "wasm32")))]
mod unix;
#[cfg(all(unix, not(target_arch = "wasm32"), not(test)))]
pub use self::unix::*;

#[cfg(any(test, target_arch = "wasm32"))]
mod test;
#[cfg(any(test, target_arch = "wasm32"))]
pub use self::test::*;

#[cfg(test)]
mod test_ {
    #[test]
    fn test_unsupported_term() {
        std::env::set_var("TERM", "xterm");
        assert!(!super::is_unsupported_term());

        std::env::set_var("TERM", "dumb");
        assert!(super::is_unsupported_term());
    }
}
