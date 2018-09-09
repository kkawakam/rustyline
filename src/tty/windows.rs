//! Windows specific definitions
use std::io::{self, Stdout, Write};
use std::mem;
use std::sync::atomic;

use unicode_width::UnicodeWidthChar;
use winapi::shared::minwindef::{DWORD, WORD};
use winapi::um::winnt::{CHAR, HANDLE};
use winapi::um::{consoleapi, handleapi, processenv, winbase, wincon, winuser};

use super::{truncate, Position, RawMode, RawReader, Renderer, Term};
use config::{ColorMode, Config};
use error;
use highlight::Highlighter;
use keys::{self, KeyPress};
use line_buffer::LineBuffer;
use Result;

const STDIN_FILENO: DWORD = winbase::STD_INPUT_HANDLE;
const STDOUT_FILENO: DWORD = winbase::STD_OUTPUT_HANDLE;

fn get_std_handle(fd: DWORD) -> Result<HANDLE> {
    let handle = unsafe { processenv::GetStdHandle(fd) };
    if handle == handleapi::INVALID_HANDLE_VALUE {
        try!(Err(io::Error::last_os_error()));
    } else if handle.is_null() {
        try!(Err(io::Error::new(
            io::ErrorKind::Other,
            "no stdio handle available for this process",
        ),));
    }
    Ok(handle)
}

#[macro_export]
macro_rules! check {
    ($funcall:expr) => {{
        let rc = unsafe { $funcall };
        if rc == 0 {
            try!(Err(io::Error::last_os_error()));
        }
        rc
    }};
}

fn get_win_size(handle: HANDLE) -> (usize, usize) {
    let mut info = unsafe { mem::zeroed() };
    match unsafe { wincon::GetConsoleScreenBufferInfo(handle, &mut info) } {
        0 => (80, 24),
        _ => (
            info.dwSize.X as usize,
            (1 + info.srWindow.Bottom - info.srWindow.Top) as usize,
        ), // (info.srWindow.Right - info.srWindow.Left + 1)
    }
}

fn get_console_mode(handle: HANDLE) -> Result<DWORD> {
    let mut original_mode = 0;
    check!(consoleapi::GetConsoleMode(handle, &mut original_mode));
    Ok(original_mode)
}

pub type Mode = ConsoleMode;

#[derive(Clone, Copy, Debug)]
pub struct ConsoleMode {
    original_stdin_mode: DWORD,
    stdin_handle: HANDLE,
    original_stdout_mode: Option<DWORD>,
    stdout_handle: HANDLE,
}

impl RawMode for Mode {
    /// Disable RAW mode for the terminal.
    fn disable_raw_mode(&self) -> Result<()> {
        check!(consoleapi::SetConsoleMode(
            self.stdin_handle,
            self.original_stdin_mode,
        ));
        if let Some(original_stdout_mode) = self.original_stdout_mode {
            check!(consoleapi::SetConsoleMode(
                self.stdout_handle,
                original_stdout_mode,
            ));
        }
        Ok(())
    }
}

/// Console input reader
pub struct ConsoleRawReader {
    handle: HANDLE,
    buf: [u16; 2],
}

impl ConsoleRawReader {
    pub fn new() -> Result<ConsoleRawReader> {
        let handle = try!(get_std_handle(STDIN_FILENO));
        Ok(ConsoleRawReader {
            handle,
            buf: [0; 2],
        })
    }
}

impl RawReader for ConsoleRawReader {
    fn next_key(&mut self, _: bool) -> Result<KeyPress> {
        use std::char::decode_utf16;
        use winapi::um::wincon::{
            LEFT_ALT_PRESSED, LEFT_CTRL_PRESSED, RIGHT_ALT_PRESSED, RIGHT_CTRL_PRESSED,
            SHIFT_PRESSED,
        };

        let mut rec: wincon::INPUT_RECORD = unsafe { mem::zeroed() };
        let mut count = 0;
        let mut surrogate = false;
        loop {
            // TODO GetNumberOfConsoleInputEvents
            check!(consoleapi::ReadConsoleInputW(
                self.handle,
                &mut rec,
                1 as DWORD,
                &mut count,
            ));

            if rec.EventType == wincon::WINDOW_BUFFER_SIZE_EVENT {
                SIGWINCH.store(true, atomic::Ordering::SeqCst);
                debug!(target: "rustyline", "SIGWINCH");
                return Err(error::ReadlineError::WindowResize); // sigwinch + err => err ignored
            } else if rec.EventType != wincon::KEY_EVENT {
                continue;
            }
            let key_event = unsafe { rec.Event.KeyEvent() };
            // writeln!(io::stderr(), "key_event: {:?}", key_event).unwrap();
            if key_event.bKeyDown == 0 && key_event.wVirtualKeyCode != winuser::VK_MENU as WORD {
                continue;
            }
            // key_event.wRepeatCount seems to be always set to 1 (maybe because we only
            // read one character at a time)

            let alt_gr = key_event.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_ALT_PRESSED)
                == (LEFT_CTRL_PRESSED | RIGHT_ALT_PRESSED);
            let alt = key_event.dwControlKeyState & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED) != 0;
            let ctrl = key_event.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED) != 0;
            let meta = alt && !alt_gr;
            let shift = key_event.dwControlKeyState & SHIFT_PRESSED != 0;

            let utf16 = unsafe { *key_event.uChar.UnicodeChar() };
            if utf16 == 0 {
                match key_event.wVirtualKeyCode as i32 {
                    winuser::VK_LEFT => {
                        return Ok(if ctrl {
                            KeyPress::ControlLeft
                        } else if shift {
                            KeyPress::ShiftLeft
                        } else {
                            KeyPress::Left
                        })
                    }
                    winuser::VK_RIGHT => {
                        return Ok(if ctrl {
                            KeyPress::ControlRight
                        } else if shift {
                            KeyPress::ShiftRight
                        } else {
                            KeyPress::Right
                        })
                    }
                    winuser::VK_UP => {
                        return Ok(if ctrl {
                            KeyPress::ControlUp
                        } else if shift {
                            KeyPress::ShiftUp
                        } else {
                            KeyPress::Up
                        })
                    }
                    winuser::VK_DOWN => {
                        return Ok(if ctrl {
                            KeyPress::ControlDown
                        } else if shift {
                            KeyPress::ShiftDown
                        } else {
                            KeyPress::Down
                        })
                    }
                    winuser::VK_DELETE => return Ok(KeyPress::Delete),
                    winuser::VK_HOME => return Ok(KeyPress::Home),
                    winuser::VK_END => return Ok(KeyPress::End),
                    winuser::VK_PRIOR => return Ok(KeyPress::PageUp),
                    winuser::VK_NEXT => return Ok(KeyPress::PageDown),
                    winuser::VK_INSERT => return Ok(KeyPress::Insert),
                    winuser::VK_F1 => return Ok(KeyPress::F(1)),
                    winuser::VK_F2 => return Ok(KeyPress::F(2)),
                    winuser::VK_F3 => return Ok(KeyPress::F(3)),
                    winuser::VK_F4 => return Ok(KeyPress::F(4)),
                    winuser::VK_F5 => return Ok(KeyPress::F(5)),
                    winuser::VK_F6 => return Ok(KeyPress::F(6)),
                    winuser::VK_F7 => return Ok(KeyPress::F(7)),
                    winuser::VK_F8 => return Ok(KeyPress::F(8)),
                    winuser::VK_F9 => return Ok(KeyPress::F(9)),
                    winuser::VK_F10 => return Ok(KeyPress::F(10)),
                    winuser::VK_F11 => return Ok(KeyPress::F(11)),
                    winuser::VK_F12 => return Ok(KeyPress::F(12)),
                    // winuser::VK_BACK is correctly handled because the key_event.UnicodeChar is
                    // also set.
                    _ => continue,
                };
            } else if utf16 == 27 {
                return Ok(KeyPress::Esc);
            } else {
                if utf16 >= 0xD800 && utf16 < 0xDC00 {
                    surrogate = true;
                    self.buf[0] = utf16;
                    continue;
                }
                let buf = if surrogate {
                    self.buf[1] = utf16;
                    &self.buf[..]
                } else {
                    self.buf[0] = utf16;
                    &self.buf[..1]
                };
                let orc = decode_utf16(buf.iter().cloned()).next();
                if orc.is_none() {
                    return Err(error::ReadlineError::Eof);
                }
                let c = try!(orc.unwrap());
                if meta {
                    return Ok(KeyPress::Meta(c));
                } else {
                    let mut key = keys::char_to_key_press(c);
                    if key == KeyPress::Tab && shift {
                        key = KeyPress::BackTab;
                    } else if key == KeyPress::Char(' ') && ctrl {
                        key = KeyPress::Ctrl(' ');
                    }
                    return Ok(key);
                }
            }
        }
    }
}

pub struct ConsoleRenderer {
    out: Stdout,
    handle: HANDLE,
    cols: usize, // Number of columns in terminal
    buffer: String,
}

impl ConsoleRenderer {
    fn new(handle: HANDLE) -> ConsoleRenderer {
        // Multi line editing is enabled by ENABLE_WRAP_AT_EOL_OUTPUT mode
        let (cols, _) = get_win_size(handle);
        ConsoleRenderer {
            out: io::stdout(),
            handle,
            cols,
            buffer: String::with_capacity(1024),
        }
    }

    fn get_console_screen_buffer_info(&self) -> Result<wincon::CONSOLE_SCREEN_BUFFER_INFO> {
        let mut info = unsafe { mem::zeroed() };
        check!(wincon::GetConsoleScreenBufferInfo(self.handle, &mut info));
        Ok(info)
    }

    fn set_console_cursor_position(&mut self, pos: wincon::COORD) -> Result<()> {
        check!(wincon::SetConsoleCursorPosition(self.handle, pos));
        Ok(())
    }

    fn clear(&mut self, length: DWORD, pos: wincon::COORD) -> Result<()> {
        let mut _count = 0;
        check!(wincon::FillConsoleOutputCharacterA(
            self.handle,
            ' ' as CHAR,
            length,
            pos,
            &mut _count,
        ));
        Ok(())
    }
}

impl Renderer for ConsoleRenderer {
    fn move_cursor(&mut self, old: Position, new: Position) -> Result<()> {
        let mut info = try!(self.get_console_screen_buffer_info());
        if new.row > old.row {
            info.dwCursorPosition.Y += (new.row - old.row) as i16;
        } else {
            info.dwCursorPosition.Y -= (old.row - new.row) as i16;
        }
        if new.col > old.col {
            info.dwCursorPosition.X += (new.col - old.col) as i16;
        } else {
            info.dwCursorPosition.X -= (old.col - new.col) as i16;
        }
        self.set_console_cursor_position(info.dwCursorPosition)
    }

    fn refresh_line(
        &mut self,
        prompt: &str,
        prompt_size: Position,
        line: &LineBuffer,
        hint: Option<String>,
        current_row: usize,
        old_rows: usize,
        highlighter: Option<&Highlighter>,
    ) -> Result<(Position, Position)> {
        // calculate the position of the end of the input line
        let end_pos = self.calculate_position(line, prompt_size);
        // calculate the desired position of the cursor
        let cursor = self.calculate_position(&line[..line.pos()], prompt_size);

        // position at the start of the prompt, clear to end of previous input
        let mut info = try!(self.get_console_screen_buffer_info());
        info.dwCursorPosition.X = 0;
        info.dwCursorPosition.Y -= current_row as i16;
        try!(self.set_console_cursor_position(info.dwCursorPosition));
        try!(self.clear(
            (info.dwSize.X * (old_rows as i16 + 1)) as DWORD,
            info.dwCursorPosition,
        ));
        self.buffer.clear();
        if let Some(highlighter) = highlighter {
            // TODO handle ansi escape code (SetConsoleTextAttribute)
            // display the prompt
            self.buffer.push_str(&highlighter.highlight_prompt(prompt));
            // display the input line
            self.buffer
                .push_str(&highlighter.highlight(line, line.pos()));
        } else {
            // display the prompt
            self.buffer.push_str(prompt);
            // display the input line
            self.buffer.push_str(line);
        }
        // display hint
        if let Some(hint) = hint {
            let truncate = truncate(&hint, end_pos.col, self.cols);
            if let Some(highlighter) = highlighter {
                self.buffer.push_str(&highlighter.highlight_hint(truncate));
            } else {
                self.buffer.push_str(truncate);
            }
        }
        try!(self.out.write_all(self.buffer.as_bytes()));
        try!(self.out.flush());

        // position the cursor
        let mut info = try!(self.get_console_screen_buffer_info());
        info.dwCursorPosition.X = cursor.col as i16;
        info.dwCursorPosition.Y -= (end_pos.row - cursor.row) as i16;
        try!(self.set_console_cursor_position(info.dwCursorPosition));
        Ok((cursor, end_pos))
    }

    fn write_and_flush(&mut self, buf: &[u8]) -> Result<()> {
        try!(self.out.write_all(buf));
        try!(self.out.flush());
        Ok(())
    }

    /// Characters with 2 column width are correctly handled (not splitted).
    fn calculate_position(&self, s: &str, orig: Position) -> Position {
        let mut pos = orig;
        for c in s.chars() {
            let cw = if c == '\n' {
                pos.col = 0;
                pos.row += 1;
                None
            } else {
                c.width()
            };
            if let Some(cw) = cw {
                pos.col += cw;
                if pos.col > self.cols {
                    pos.row += 1;
                    pos.col = cw;
                }
            }
        }
        if pos.col == self.cols {
            pos.col = 0;
            pos.row += 1;
        }
        pos
    }

    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self) -> Result<()> {
        let info = try!(self.get_console_screen_buffer_info());
        let coord = wincon::COORD { X: 0, Y: 0 };
        check!(wincon::SetConsoleCursorPosition(self.handle, coord));
        let n = info.dwSize.X as DWORD * info.dwSize.Y as DWORD;
        self.clear(n, coord)
    }

    fn sigwinch(&self) -> bool {
        SIGWINCH.compare_and_swap(true, false, atomic::Ordering::SeqCst)
    }

    /// Try to get the number of columns in the current terminal,
    /// or assume 80 if it fails.
    fn update_size(&mut self) {
        let (cols, _) = get_win_size(self.handle);
        self.cols = cols;
    }

    fn get_columns(&self) -> usize {
        self.cols
    }

    /// Try to get the number of rows in the current terminal,
    /// or assume 24 if it fails.
    fn get_rows(&self) -> usize {
        let (_, rows) = get_win_size(self.handle);
        rows
    }
}

static SIGWINCH: atomic::AtomicBool = atomic::ATOMIC_BOOL_INIT;

pub type Terminal = Console;

#[derive(Clone, Debug)]
pub struct Console {
    stdin_isatty: bool,
    stdin_handle: HANDLE,
    stdout_isatty: bool,
    stdout_handle: HANDLE,
    pub(crate) color_mode: ColorMode,
    ansi_colors_supported: bool,
}

impl Console {}

impl Term for Console {
    type Mode = Mode;
    type Reader = ConsoleRawReader;
    type Writer = ConsoleRenderer;

    fn new(color_mode: ColorMode) -> Console {
        use std::ptr;
        let stdin_handle = get_std_handle(STDIN_FILENO);
        let stdin_isatty = match stdin_handle {
            Ok(handle) => {
                // If this function doesn't fail then fd is a TTY
                get_console_mode(handle).is_ok()
            }
            Err(_) => false,
        };
        let stdout_handle = get_std_handle(STDOUT_FILENO);
        let stdout_isatty = match stdout_handle {
            Ok(handle) => {
                // If this function doesn't fail then fd is a TTY
                get_console_mode(handle).is_ok()
            }
            Err(_) => false,
        };

        Console {
            stdin_isatty,
            stdin_handle: stdin_handle.unwrap_or(ptr::null_mut()),
            stdout_isatty,
            stdout_handle: stdout_handle.unwrap_or(ptr::null_mut()),
            color_mode,
            ansi_colors_supported: false,
        }
    }

    /// Checking for an unsupported TERM in windows is a no-op
    fn is_unsupported(&self) -> bool {
        false
    }

    fn is_stdin_tty(&self) -> bool {
        self.stdin_isatty
    }

    fn colors_enabled(&self) -> bool {
        // TODO ANSI Colors & Windows <10
        match self.color_mode {
            ColorMode::Enabled => self.stdout_isatty && self.ansi_colors_supported,
            ColorMode::Forced => true,
            ColorMode::Disabled => false,
        }
    }

    // pub fn install_sigwinch_handler(&mut self) {
    // See ReadConsoleInputW && WINDOW_BUFFER_SIZE_EVENT
    // }

    /// Enable RAW mode for the terminal.
    fn enable_raw_mode(&mut self) -> Result<Mode> {
        if !self.stdin_isatty {
            try!(Err(io::Error::new(
                io::ErrorKind::Other,
                "no stdio handle available for this process",
            ),));
        }
        let original_stdin_mode = try!(get_console_mode(self.stdin_handle));
        // Disable these modes
        let mut raw = original_stdin_mode & !(wincon::ENABLE_LINE_INPUT
            | wincon::ENABLE_ECHO_INPUT
            | wincon::ENABLE_PROCESSED_INPUT);
        // Enable these modes
        raw |= wincon::ENABLE_EXTENDED_FLAGS;
        raw |= wincon::ENABLE_INSERT_MODE;
        raw |= wincon::ENABLE_QUICK_EDIT_MODE;
        raw |= wincon::ENABLE_WINDOW_INPUT;
        check!(consoleapi::SetConsoleMode(self.stdin_handle, raw));

        let original_stdout_mode = if self.stdout_isatty {
            let original_stdout_mode = try!(get_console_mode(self.stdout_handle));
            // To enable ANSI colors (Windows 10 only):
            // https://docs.microsoft.com/en-us/windows/console/setconsolemode
            if original_stdout_mode & wincon::ENABLE_VIRTUAL_TERMINAL_PROCESSING == 0 {
                let raw = original_stdout_mode | wincon::ENABLE_VIRTUAL_TERMINAL_PROCESSING;
                self.ansi_colors_supported =
                    unsafe { consoleapi::SetConsoleMode(self.stdout_handle, raw) != 0 };
            }
            Some(original_stdout_mode)
        } else {
            None
        };

        Ok(Mode {
            original_stdin_mode,
            stdin_handle: self.stdin_handle,
            original_stdout_mode,
            stdout_handle: self.stdout_handle,
        })
    }

    fn create_reader(&self, _: &Config) -> Result<ConsoleRawReader> {
        ConsoleRawReader::new()
    }

    fn create_writer(&self) -> ConsoleRenderer {
        ConsoleRenderer::new(self.stdout_handle)
    }
}
