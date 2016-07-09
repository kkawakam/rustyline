extern crate kernel32;
extern crate winapi;

use std::io;
use super::StandardStream;
use super::Terminal;
use ::Result;

macro_rules! check {
    ($funcall:expr) => (
        if $funcall == 0 {
            return Err(From::from(io::Error::last_os_error()));
        }
    );
}

/// Try to get the number of columns in the current terminal, or assume 80 if it fails.
pub fn get_columns() -> usize {
    // Get HANDLE to stdout
    let handle = unsafe { kernel32::GetStdHandle(winapi::STD_OUTPUT_HANDLE) };

    // Create CONSOLE_SCREEN_BUFFER_INFO with some default values
    let mut csbi = winapi::wincon::CONSOLE_SCREEN_BUFFER_INFO {
        dwSize: winapi::wincon::COORD { X: 0, Y: 0 },
        dwCursorPosition: winapi::wincon::COORD { X: 0, Y: 0 },
        wAttributes: 0,
        srWindow: winapi::wincon::SMALL_RECT {
            Left: 0,
            Top: 0,
            Right: 0,
            Bottom: 0,
        },
        dwMaximumWindowSize: winapi::wincon::COORD { X: 0, Y: 0 },
    };

    let success: bool = unsafe { kernel32::GetConsoleScreenBufferInfo(handle, &mut csbi) != 0 };

    // If we were not able to retrieve console info successfully,
    // we will default to a column size of 80
    if success && csbi.dwSize.X > 0 {
        csbi.dwSize.X as usize
    } else {
        80
    }
}

/// Get WindowsTerminal struct
pub fn get_terminal() -> WindowsTerminal {
    WindowsTerminal{ original_mode: None }
}

/// Checking for an unsupported TERM in windows is a no-op
pub fn is_unsupported_term() -> bool {
    false
}

/// Return whether or not STDIN, STDOUT or STDERR is a TTY
pub fn is_a_tty(stream: StandardStream) -> bool {
    let handle = match stream {
            StandardStream::StdIn => winapi::STD_INPUT_HANDLE,
            StandardStream::StdOut => winapi::STD_OUTPUT_HANDLE,
    };

    unsafe {
            let handle = kernel32::GetStdHandle(handle);
            let mut out = 0;
            kernel32::GetConsoleMode(handle, &mut out) != 0
    }
}

pub struct WindowsTerminal {
    original_mode: Option<winapi::minwindef::DWORD>
}

impl Terminal for WindowsTerminal {
    /// Enable raw mode for the TERM
    fn enable_raw_mode(&mut self) -> Result<()> {
        let mut original_mode: winapi::minwindef::DWORD = 0;
        unsafe {
            let handle = kernel32::GetStdHandle(winapi::STD_INPUT_HANDLE);
            check!(kernel32::GetConsoleMode(handle, &mut original_mode));
            check!(kernel32::SetConsoleMode(
                handle,
                original_mode & !(winapi::wincon::ENABLE_LINE_INPUT |
                            winapi::wincon::ENABLE_ECHO_INPUT |
                            winapi::wincon::ENABLE_PROCESSED_INPUT)
            ));
        };
        self.original_mode = Some(original_mode);
        Ok(())
    }

    /// Disable Raw mode for the term
    fn disable_raw_mode(&self) -> Result<()> {
        unsafe {
            let handle = kernel32::GetStdHandle(winapi::STD_INPUT_HANDLE);
            check!(kernel32::SetConsoleMode(handle,
                                    self.original_mode.expect("RAW MODE was not enabled previously")));
        }
        Ok(())
    }
}

/// Ensure that RAW mode is disabled even in the case of a panic!
#[allow(unused_must_use)]
impl Drop for WindowsTerminal {
    fn drop(&mut self) {
        self.disable_raw_mode();
    }
}
