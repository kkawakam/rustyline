//! Windows specific definitions
use std::io::{self, Stdout, Write};
use std::mem;
use std::sync::atomic;

use kernel32;
use unicode_width::UnicodeWidthChar;
use winapi;

use config::Config;
use consts::{self, KeyPress};
use error;
use Result;
use line_buffer::LineBuffer;
use super::{Position, RawMode, RawReader, Renderer, Term};

const STDIN_FILENO: winapi::DWORD = winapi::STD_INPUT_HANDLE;
const STDOUT_FILENO: winapi::DWORD = winapi::STD_OUTPUT_HANDLE;

fn get_std_handle(fd: winapi::DWORD) -> Result<winapi::HANDLE> {
    let handle = unsafe { kernel32::GetStdHandle(fd) };
    if handle == winapi::INVALID_HANDLE_VALUE {
        try!(Err(io::Error::last_os_error()));
    } else if handle.is_null() {
        try!(Err(io::Error::new(io::ErrorKind::Other,
                                "no stdio handle available for this process")));
    }
    Ok(handle)
}

#[macro_export]
macro_rules! check {
    ($funcall:expr) => {
        {
        let rc = unsafe { $funcall };
        if rc == 0 {
            try!(Err(io::Error::last_os_error()));
        }
        rc
        }
    };
}

fn get_win_size(handle: winapi::HANDLE) -> (usize, usize) {
    let mut info = unsafe { mem::zeroed() };
    match unsafe { kernel32::GetConsoleScreenBufferInfo(handle, &mut info) } {
        0 => (80, 24),
        _ => (info.dwSize.X as usize, (1 + info.srWindow.Bottom - info.srWindow.Top) as usize),
    }
}

fn get_console_mode(handle: winapi::HANDLE) -> Result<winapi::DWORD> {
    let mut original_mode = 0;
    check!(kernel32::GetConsoleMode(handle, &mut original_mode));
    Ok(original_mode)
}

pub type Mode = ConsoleMode;

#[derive(Clone,Copy,Debug)]
pub struct ConsoleMode {
    original_mode: winapi::DWORD,
    stdin_handle: winapi::HANDLE,
}

impl RawMode for Mode {
    /// Disable RAW mode for the terminal.
    fn disable_raw_mode(&self) -> Result<()> {
        check!(kernel32::SetConsoleMode(self.stdin_handle, self.original_mode));
        Ok(())
    }
}

/// Console input reader
pub struct ConsoleRawReader {
    handle: winapi::HANDLE,
    buf: Option<u16>,
}

impl ConsoleRawReader {
    pub fn new() -> Result<ConsoleRawReader> {
        let handle = try!(get_std_handle(STDIN_FILENO));
        Ok(ConsoleRawReader {
               handle: handle,
               buf: None,
           })
    }
}

impl RawReader for ConsoleRawReader {
    fn next_key(&mut self) -> Result<KeyPress> {
        use std::char::decode_utf16;
        // use winapi::{LEFT_ALT_PRESSED, LEFT_CTRL_PRESSED, RIGHT_ALT_PRESSED, RIGHT_CTRL_PRESSED};
        use winapi::{LEFT_ALT_PRESSED, RIGHT_ALT_PRESSED};

        let mut rec: winapi::INPUT_RECORD = unsafe { mem::zeroed() };
        let mut count = 0;
        loop {
            // TODO GetNumberOfConsoleInputEvents
            check!(kernel32::ReadConsoleInputW(self.handle,
                                               &mut rec,
                                               1 as winapi::DWORD,
                                               &mut count));

            if rec.EventType == winapi::WINDOW_BUFFER_SIZE_EVENT {
                SIGWINCH.store(true, atomic::Ordering::SeqCst);
                debug!(target: "rustyline", "SIGWINCH");
                return Err(error::ReadlineError::WindowResize);
            } else if rec.EventType != winapi::KEY_EVENT {
                continue;
            }
            let key_event = unsafe { rec.KeyEvent() };
            // writeln!(io::stderr(), "key_event: {:?}", key_event).unwrap();
            if key_event.bKeyDown == 0 &&
               key_event.wVirtualKeyCode != winapi::VK_MENU as winapi::WORD {
                continue;
            }

            // let alt_gr = key_event.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_ALT_PRESSED) != 0;
            let alt = key_event.dwControlKeyState & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED) != 0;
            // let ctrl = key_event.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED) != 0;
            let meta = alt;

            let utf16 = key_event.UnicodeChar;
            if utf16 == 0 {
                match key_event.wVirtualKeyCode as i32 {
                    winapi::VK_LEFT => return Ok(KeyPress::Left),
                    winapi::VK_RIGHT => return Ok(KeyPress::Right),
                    winapi::VK_UP => return Ok(KeyPress::Up),
                    winapi::VK_DOWN => return Ok(KeyPress::Down),
                    winapi::VK_DELETE => return Ok(KeyPress::Delete),
                    winapi::VK_HOME => return Ok(KeyPress::Home),
                    winapi::VK_END => return Ok(KeyPress::End),
                    winapi::VK_PRIOR => return Ok(KeyPress::PageUp),
                    winapi::VK_NEXT => return Ok(KeyPress::PageDown),
                    _ => continue,
                };
            } else if utf16 == 27 {
                return Ok(KeyPress::Esc);
            } else {
                // TODO How to support surrogate pair ?
                self.buf = Some(utf16);
                let orc = decode_utf16(self).next();
                if orc.is_none() {
                    return Err(error::ReadlineError::Eof);
                }
                let c = try!(orc.unwrap());
                if meta {
                    return Ok(match c {
                                  '-' => KeyPress::Meta('-'),
                                  '0'...'9' => KeyPress::Meta(c),
                                  '<' => KeyPress::Meta('<'),
                                  '>' => KeyPress::Meta('>'),
                                  'b' | 'B' => KeyPress::Meta('B'),
                                  'c' | 'C' => KeyPress::Meta('C'),
                                  'd' | 'D' => KeyPress::Meta('D'),
                                  'f' | 'F' => KeyPress::Meta('F'),
                                  'l' | 'L' => KeyPress::Meta('L'),
                                  'n' | 'N' => KeyPress::Meta('N'),
                                  'p' | 'P' => KeyPress::Meta('P'),
                                  't' | 'T' => KeyPress::Meta('T'),
                                  'u' | 'U' => KeyPress::Meta('U'),
                                  'y' | 'Y' => KeyPress::Meta('Y'),
                                  _ => {
                        debug!(target: "rustyline", "unsupported esc sequence: M-{:?}", c);
                        KeyPress::UnknownEscSeq
                    }
                              });
                } else {
                    return Ok(consts::char_to_key_press(c));
                }
            }
        }
    }
}

impl Iterator for ConsoleRawReader {
    type Item = u16;

    fn next(&mut self) -> Option<u16> {
        let buf = self.buf;
        self.buf = None;
        buf
    }
}

pub struct ConsoleRenderer {
    out: Stdout,
    handle: winapi::HANDLE,
    cols: usize, // Number of columns in terminal
}

impl ConsoleRenderer {
    fn new(handle: winapi::HANDLE) -> ConsoleRenderer {
        let (cols, _) = get_win_size(handle);
        ConsoleRenderer {
            out: io::stdout(),
            handle: handle,
            cols: cols,
        }
    }

    fn get_console_screen_buffer_info(&self) -> Result<winapi::CONSOLE_SCREEN_BUFFER_INFO> {
        let mut info = unsafe { mem::zeroed() };
        check!(kernel32::GetConsoleScreenBufferInfo(self.handle, &mut info));
        Ok(info)
    }

    fn set_console_cursor_position(&mut self, pos: winapi::COORD) -> Result<()> {
        check!(kernel32::SetConsoleCursorPosition(self.handle, pos));
        Ok(())
    }

    fn fill_console_output_character(&mut self,
                                     length: winapi::DWORD,
                                     pos: winapi::COORD)
                                     -> Result<()> {
        let mut _count = 0;
        check!(kernel32::FillConsoleOutputCharacterA(self.handle,
                                                     ' ' as winapi::CHAR,
                                                     length,
                                                     pos,
                                                     &mut _count));
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

    fn refresh_line(&mut self,
                    prompt: &str,
                    prompt_size: Position,
                    line: &LineBuffer,
                    current_row: usize,
                    old_rows: usize)
                    -> Result<(Position, Position)> {
        // calculate the position of the end of the input line
        let end_pos = self.calculate_position(line, prompt_size);
        // calculate the desired position of the cursor
        let cursor = self.calculate_position(&line[..line.pos()], prompt_size);

        // position at the start of the prompt, clear to end of previous input
        let mut info = try!(self.get_console_screen_buffer_info());
        info.dwCursorPosition.X = 0;
        info.dwCursorPosition.Y -= current_row as i16;
        try!(self.set_console_cursor_position(info.dwCursorPosition));
        let mut _count = 0;
        try!(self.fill_console_output_character((info.dwSize.X * (old_rows as i16 + 1)) as u32,
                                                info.dwCursorPosition));
        let mut ab = String::new();
        // display the prompt
        ab.push_str(prompt); // TODO handle ansi escape code (SetConsoleTextAttribute)
        // display the input line
        ab.push_str(&line);
        try!(self.write_and_flush(ab.as_bytes()));

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
        let coord = winapi::COORD { X: 0, Y: 0 };
        check!(kernel32::SetConsoleCursorPosition(self.handle, coord));
        let mut _count = 0;
        let n = info.dwSize.X as winapi::DWORD * info.dwSize.Y as winapi::DWORD;
        check!(kernel32::FillConsoleOutputCharacterA(self.handle,
                                                     ' ' as winapi::CHAR,
                                                     n,
                                                     coord,
                                                     &mut _count));
        Ok(())
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

#[derive(Clone,Debug)]
pub struct Console {
    stdin_isatty: bool,
    stdin_handle: winapi::HANDLE,
    stdout_handle: winapi::HANDLE,
}

impl Console {}

impl Term for Console {
    type Reader = ConsoleRawReader;
    type Writer = ConsoleRenderer;
    type Mode = Mode;

    fn new() -> Console {
        use std::ptr;
        let stdin_handle = get_std_handle(STDIN_FILENO);
        let stdin_isatty = match stdin_handle {
            Ok(handle) => {
                // If this function doesn't fail then fd is a TTY
                get_console_mode(handle).is_ok()
            }
            Err(_) => false,
        };

        let stdout_handle = get_std_handle(STDOUT_FILENO).unwrap_or(ptr::null_mut());
        Console {
            stdin_isatty: stdin_isatty,
            stdin_handle: stdin_handle.unwrap_or(ptr::null_mut()),
            stdout_handle: stdout_handle,
        }
    }

    /// Checking for an unsupported TERM in windows is a no-op
    fn is_unsupported(&self) -> bool {
        false
    }

    fn is_stdin_tty(&self) -> bool {
        self.stdin_isatty
    }

    // pub fn install_sigwinch_handler(&mut self) {
    // See ReadConsoleInputW && WINDOW_BUFFER_SIZE_EVENT
    // }

    /// Enable RAW mode for the terminal.
    fn enable_raw_mode(&self) -> Result<Mode> {
        if !self.stdin_isatty {
            try!(Err(io::Error::new(io::ErrorKind::Other,
                                    "no stdio handle available for this process")));
        }
        let original_mode = try!(get_console_mode(self.stdin_handle));
        // Disable these modes
        let raw = original_mode &
                  !(winapi::wincon::ENABLE_LINE_INPUT | winapi::wincon::ENABLE_ECHO_INPUT |
                    winapi::wincon::ENABLE_PROCESSED_INPUT);
        // Enable these modes
        let raw = raw | winapi::wincon::ENABLE_EXTENDED_FLAGS;
        let raw = raw | winapi::wincon::ENABLE_INSERT_MODE;
        let raw = raw | winapi::wincon::ENABLE_QUICK_EDIT_MODE;
        let raw = raw | winapi::wincon::ENABLE_WINDOW_INPUT;
        check!(kernel32::SetConsoleMode(self.stdin_handle, raw));
        Ok(Mode {
               original_mode: original_mode,
               stdin_handle: self.stdin_handle,
           })
    }

    fn create_reader(&self, _: &Config) -> Result<ConsoleRawReader> {
        ConsoleRawReader::new()
    }

    fn create_writer(&self) -> ConsoleRenderer {
        ConsoleRenderer::new(self.stdout_handle)
    }
}
