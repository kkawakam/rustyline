use std::io;
use std::io::Read;
use std::marker::PhantomData;
use std::mem;
use std::sync::atomic;

use kernel32;
use winapi;

use consts::{self, KeyPress};
use ::error;
use ::Result;
use SIGWINCH;

pub type Handle = winapi::HANDLE;
pub type Mode = winapi::DWORD;
pub const STDIN_FILENO: winapi::DWORD = winapi::STD_INPUT_HANDLE;
pub const STDOUT_FILENO: winapi::DWORD = winapi::STD_OUTPUT_HANDLE;

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

/// Try to get the number of columns in the current terminal, or assume 80 if it fails.
pub fn get_columns(handle: Handle) -> usize {
    let mut info = unsafe { mem::zeroed() };
    match unsafe { kernel32::GetConsoleScreenBufferInfo(handle, &mut info) } {
        0 => 80,
        _ => info.dwSize.X as usize,
    }
}

/// Checking for an unsupported TERM in windows is a no-op
pub fn is_unsupported_term() -> bool {
    false
}

fn get_console_mode(handle: winapi::HANDLE) -> Result<Mode> {
    let mut original_mode = 0;
    check!(kernel32::GetConsoleMode(handle, &mut original_mode));
    Ok(original_mode)
}

/// Return whether or not STDIN, STDOUT or STDERR is a TTY
pub fn is_a_tty(fd: winapi::DWORD) -> bool {
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
    let raw = original_mode &
              !(winapi::wincon::ENABLE_LINE_INPUT | winapi::wincon::ENABLE_ECHO_INPUT |
                winapi::wincon::ENABLE_PROCESSED_INPUT);
    check!(kernel32::SetConsoleMode(handle, raw));
    Ok(original_mode)
}

/// Disable Raw mode for the term
pub fn disable_raw_mode(original_mode: Mode) -> Result<()> {
    let handle = try!(get_std_handle(STDIN_FILENO));
    check!(kernel32::SetConsoleMode(handle, original_mode));
    Ok(())
}

pub fn stdout_handle() -> Result<Handle> {
    let handle = try!(get_std_handle(STDOUT_FILENO));
    Ok(handle)
}

/// Console input reader
pub struct RawReader<R> {
    handle: winapi::HANDLE,
    buf: Option<u16>,
    phantom: PhantomData<R>,
}

impl<R: Read> RawReader<R> {
    pub fn new(_: R) -> Result<RawReader<R>> {
        let handle = try!(get_std_handle(STDIN_FILENO));
        Ok(RawReader {
            handle: handle,
            buf: None,
            phantom: PhantomData,
        })
    }

    pub fn next_key(&mut self, _: bool) -> Result<KeyPress> {
        use std::char::decode_utf16;

        let mut rec: winapi::INPUT_RECORD = unsafe { mem::zeroed() };
        let mut count = 0;
        let mut esc_seen = false;
        loop {
            // TODO GetNumberOfConsoleInputEvents
            check!(kernel32::ReadConsoleInputW(self.handle,
                                               &mut rec,
                                               1 as winapi::DWORD,
                                               &mut count));

            // TODO ENABLE_WINDOW_INPUT ???
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

            let ctrl = key_event.dwControlKeyState &
                       (winapi::LEFT_CTRL_PRESSED | winapi::RIGHT_CTRL_PRESSED) !=
                       0;
            let meta = (key_event.dwControlKeyState &
                        (winapi::LEFT_ALT_PRESSED | winapi::RIGHT_ALT_PRESSED) !=
                        0) || esc_seen;

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
                        _ => return Ok(KeyPress::UNKNOWN_ESC_SEQ),
                    }
                } else {
                    return Ok(consts::char_to_key_press(c));
                }
            }
        }
    }
}

impl<R: Read> Iterator for RawReader<R> {
    type Item = u16;

    fn next(&mut self) -> Option<u16> {
        let buf = self.buf;
        self.buf = None;
        buf
    }
}
