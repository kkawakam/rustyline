//! Windows specific definitions
use std::io;
use std::io::Write;
use std::mem;
use std::sync::atomic;

use kernel32;
use winapi;

use consts::{self, KeyPress};
use ::error;
use ::Result;
use super::{RawReader, Term};

pub type Mode = winapi::DWORD;
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

fn get_columns(handle: winapi::HANDLE) -> usize {
    let (cols, _) = get_win_size(handle);
    cols
}

fn get_rows(handle: winapi::HANDLE) -> usize {
    let (_, rows) = get_win_size(handle);
    rows
}

fn get_win_size(handle: winapi::HANDLE) -> (usize, usize) {
    let mut info = unsafe { mem::zeroed() };
    match unsafe { kernel32::GetConsoleScreenBufferInfo(handle, &mut info) } {
        0 => (80, 24),
        _ => (info.dwSize.X as usize, (1 + info.srWindow.Bottom - info.srWindow.Top) as usize),
    }
}

fn get_console_mode(handle: winapi::HANDLE) -> Result<Mode> {
    let mut original_mode = 0;
    check!(kernel32::GetConsoleMode(handle, &mut original_mode));
    Ok(original_mode)
}

/// Return whether or not STDIN, STDOUT or STDERR is a TTY
fn is_a_tty(fd: winapi::DWORD) -> bool {
    let handle = get_std_handle(fd);
    match handle {
        Ok(handle) => {
            // If this function doesn't fail then fd is a TTY
            get_console_mode(handle).is_ok()
        }
        Err(_) => false,
    }
}

/// Enable raw mode for the TERM
pub fn enable_raw_mode() -> Result<Mode> {
    let handle = try!(get_std_handle(STDIN_FILENO));
    let original_mode = try!(get_console_mode(handle));
    // Disable these modes
    let raw = original_mode &
              !(winapi::wincon::ENABLE_LINE_INPUT | winapi::wincon::ENABLE_ECHO_INPUT |
                winapi::wincon::ENABLE_PROCESSED_INPUT);
    // Enable these modes
    let raw = raw | winapi::wincon::ENABLE_EXTENDED_FLAGS;
    let raw = raw | winapi::wincon::ENABLE_INSERT_MODE;
    let raw = raw | winapi::wincon::ENABLE_QUICK_EDIT_MODE;
    let raw = raw | winapi::wincon::ENABLE_WINDOW_INPUT;
    check!(kernel32::SetConsoleMode(handle, raw));
    Ok(original_mode)
}

/// Disable Raw mode for the term
pub fn disable_raw_mode(original_mode: Mode) -> Result<()> {
    let handle = try!(get_std_handle(STDIN_FILENO));
    check!(kernel32::SetConsoleMode(handle, original_mode));
    Ok(())
}

/// Clear the screen. Used to handle ctrl+l
fn clear_screen(info: winapi::CONSOLE_SCREEN_BUFFER_INFO, handle: winapi::HANDLE) -> Result<()> {
    let coord = winapi::COORD { X: 0, Y: 0 };
    check!(kernel32::SetConsoleCursorPosition(handle, coord));
    let mut _count = 0;
    let n = info.dwSize.X as winapi::DWORD * info.dwSize.Y as winapi::DWORD;
    check!(kernel32::FillConsoleOutputCharacterA(handle,
                                                 ' ' as winapi::CHAR,
                                                 n,
                                                 coord,
                                                 &mut _count));
    Ok(())
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
    fn next_key(&mut self, _: bool) -> Result<KeyPress> {
        use std::char::decode_utf16;
        // use winapi::{LEFT_ALT_PRESSED, LEFT_CTRL_PRESSED, RIGHT_ALT_PRESSED, RIGHT_CTRL_PRESSED};
        use winapi::{LEFT_ALT_PRESSED, RIGHT_ALT_PRESSED};

        let mut rec: winapi::INPUT_RECORD = unsafe { mem::zeroed() };
        let mut count = 0;
        let mut esc_seen = false;
        loop {
            // TODO GetNumberOfConsoleInputEvents
            check!(kernel32::ReadConsoleInputW(self.handle,
                                               &mut rec,
                                               1 as winapi::DWORD,
                                               &mut count));

            if rec.EventType == winapi::WINDOW_BUFFER_SIZE_EVENT {
                SIGWINCH.store(true, atomic::Ordering::SeqCst);
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

            // let alt_gr = key_event.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_ALT_PRESSED) ==
            // (LEFT_CTRL_PRESSED | RIGHT_ALT_PRESSED);
            let alt = key_event.dwControlKeyState & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED) ==
                      (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED);
            // let ctrl = key_event.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED) ==
            // (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED);
            let meta = alt || esc_seen;

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
                esc_seen = true;
                continue;
            } else {
                // TODO How to support surrogate pair ?
                self.buf = Some(utf16);
                let orc = decode_utf16(self).next();
                if orc.is_none() {
                    return Err(error::ReadlineError::Eof);
                }
                let c = try!(orc.unwrap());
                if meta {
                    match c {
                        'b' | 'B' => return Ok(KeyPress::Meta('B')),
                        'c' | 'C' => return Ok(KeyPress::Meta('C')),
                        'd' | 'D' => return Ok(KeyPress::Meta('D')),
                        'f' | 'F' => return Ok(KeyPress::Meta('F')),
                        'l' | 'L' => return Ok(KeyPress::Meta('L')),
                        't' | 'T' => return Ok(KeyPress::Meta('T')),
                        'u' | 'U' => return Ok(KeyPress::Meta('U')),
                        'y' | 'Y' => return Ok(KeyPress::Meta('Y')),
                        _ => return Ok(KeyPress::UnknownEscSeq),
                    }
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

static SIGWINCH: atomic::AtomicBool = atomic::ATOMIC_BOOL_INIT;

pub type Terminal = Console;

#[derive(Clone,Debug)]
pub struct Console {
    stdin_isatty: bool,
    stdout_handle: winapi::HANDLE,
}

impl Console {
    pub fn create_reader(&self) -> Result<ConsoleRawReader> {
        ConsoleRawReader::new()
    }
}

impl Term for Console {
    pub fn new() -> Console {
        use std::ptr;
        let stdout_handle = get_std_handle(STDOUT_FILENO).unwrap_or(ptr::null_mut());
        Console {
            stdin_isatty: is_a_tty(STDIN_FILENO),
            stdout_handle: stdout_handle,
        }
    }

    /// Checking for an unsupported TERM in windows is a no-op
    pub fn is_unsupported(&self) -> bool {
        false
    }

    pub fn is_stdin_tty(&self) -> bool {
        self.stdin_isatty
    }

    // pub fn install_sigwinch_handler(&mut self) {
    // See ReadConsoleInputW && WINDOW_BUFFER_SIZE_EVENT
    // }

    /// Try to get the number of columns in the current terminal,
    /// or assume 80 if it fails.
    pub fn get_columns(&self) -> usize {
        get_columns(self.stdout_handle)
    }

    /// Try to get the number of rows in the current terminal,
    /// or assume 24 if it fails.
    pub fn get_rows(&self) -> usize {
        get_rows(self.stdout_handle)
    }

    pub fn sigwinch(&self) -> bool {
        SIGWINCH.compare_and_swap(true, false, atomic::Ordering::SeqCst)
    }

    pub fn clear_screen(&mut self, _: &mut Write) -> Result<()> {
        let info = try!(self.get_console_screen_buffer_info());
        clear_screen(info, self.stdout_handle)
    }

    pub fn get_console_screen_buffer_info(&self) -> Result<winapi::CONSOLE_SCREEN_BUFFER_INFO> {
        let mut info = unsafe { mem::zeroed() };
        check!(kernel32::GetConsoleScreenBufferInfo(self.stdout_handle, &mut info));
        Ok(info)
    }

    pub fn set_console_cursor_position(&mut self, pos: winapi::COORD) -> Result<()> {
        check!(kernel32::SetConsoleCursorPosition(self.stdout_handle, pos));
        Ok(())
    }

    pub fn fill_console_output_character(&mut self,
                                         length: winapi::DWORD,
                                         pos: winapi::COORD)
                                         -> Result<()> {
        let mut _count = 0;
        check!(kernel32::FillConsoleOutputCharacterA(self.stdout_handle,
                                                     ' ' as winapi::CHAR,
                                                     length,
                                                     pos,
                                                     &mut _count));
        Ok(())
    }
}
