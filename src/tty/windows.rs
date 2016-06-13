extern crate kernel32;
extern crate winapi;

use super::StandardStream;

/// Return whether or not STDIN, STDOUT or STDERR is a TTY
fn is_a_tty(stream: StandardStream) -> bool {
    let handle = match stream {
            StandardStream::StdIn => winapi::winbase::STD_INPUT_HANDLE,
            StandardStream::Stdout => winapi::winbase::STD_OUTPUT_HANDLE,
    };

    unsafe {
            let handle = kernel32::GetStdHandle(handle);
            let mut out = 0;
            kernel32::GetConsoleMode(handle, &mut out) != 0
    }
}

/// Checking for an unsupported TERM in windows is a no-op
pub fn is_unsupported_term() -> bool {
    false
}
