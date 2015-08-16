//!Readline for Rust
//!
//!This implementation is based on [Antirez's Linenoise](https://github.com/antirez/linenoise)
//!
//!# Example
//!
//!Usage
//!
//!```
//!let readline = rustyline::readline(">> ");
//!match readline {
//!     Ok(line) => println!("Line: {:?}",line),
//!     Err(_)   => println!("No input"),
//! }
//!```
#![feature(io)]
#![feature(str_char)]
#![feature(unicode)]
extern crate libc;
extern crate nix;
extern crate unicode_width;

#[allow(non_camel_case_types)]
pub mod consts;
pub mod error;

use std::result;
use std::io;
use std::io::{Write, Read};
use nix::errno::Errno;
use nix::sys::termios;

use consts::{KeyPress, char_to_key_press};

/// The error type for I/O and Linux Syscalls (Errno)
pub type Result<T> = result::Result<T, error::ReadlineError>;

// Represent the state during line editing.
struct State<'prompt> {
    prompt: &'prompt str, // Prompt to display
    prompt_width: usize, // Prompt Unicode width
    buf: String, // Edited line buffer
    pos: usize, // Current cursor position
//    oldpos: usize, // Previous refresh cursor position
    cols: usize, // Number of columns in terminal
    bytes: [u8; 4]
}

/// Maximum buffer size for the line read
static MAX_LINE: usize = 4096;

/// Unsupported Terminals that don't support RAW mode
static UNSUPPORTED_TERM: [&'static str; 3] = ["dumb","cons25","emacs"];

/// Check to see if STDIN is a TTY
fn is_a_tty() -> bool {
    let isatty = unsafe { libc::isatty(libc::STDIN_FILENO as i32) } != 0;
    isatty
}

/// Check to see if the current `TERM` is unsupported
fn is_unsupported_term() -> bool {
    use std::ascii::AsciiExt;
    match std::env::var("TERM") {
        Ok(term) => {
            let mut unsupported = false;
            for iter in &UNSUPPORTED_TERM {
                unsupported = (*iter).eq_ignore_ascii_case(&term)
            }
            unsupported
        }
        Err(_) => false
    }
}

fn from_errno(errno: Errno) -> error::ReadlineError {
    error::ReadlineError::from(nix::Error::from_errno(errno))
}

/// Enable raw mode for the TERM
fn enable_raw_mode() -> Result<termios::Termios> {
    use nix::sys::termios::{BRKINT, ICRNL, INPCK, ISTRIP, IXON, OPOST, CS8, ECHO, ICANON, IEXTEN, ISIG, VMIN, VTIME};
    if !is_a_tty() {
        Err(from_errno(Errno::ENOTTY))
    } else {
        let original_term = try!(termios::tcgetattr(libc::STDIN_FILENO));
        let mut raw = original_term;
        raw.c_iflag = raw.c_iflag   & !(BRKINT | ICRNL | INPCK | ISTRIP | IXON);
        raw.c_oflag = raw.c_oflag   & !(OPOST);
        raw.c_cflag = raw.c_cflag   | (CS8);
        raw.c_lflag = raw.c_lflag   & !(ECHO | ICANON | IEXTEN | ISIG);
        raw.c_cc[VMIN] = 1;
        raw.c_cc[VTIME] = 0;
        try!(termios::tcsetattr(libc::STDIN_FILENO, termios::TCSAFLUSH, &raw));
        Ok(original_term)
    }
}

/// Disable Raw mode for the term
fn disable_raw_mode(original_termios: termios::Termios) -> Result<()> {
    try!(termios::tcsetattr(libc::STDIN_FILENO,
                             termios::TCSAFLUSH,
                             &original_termios));
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
const TIOCGWINSZ: libc::c_ulong = 0x40087468;

#[cfg(any(target_os = "linux", target_os = "android"))]
const TIOCGWINSZ: libc::c_ulong = 0x5413;

/// Try to get the number of columns in the current terminal,
/// or assume 80 if it fails.
#[cfg(any(target_os = "linux",
          target_os = "android",
          target_os = "macos",
          target_os = "freebsd"))]
fn get_columns() -> usize {
    use std::mem::zeroed;
    use libc::c_ushort;
    use nix::sys::ioctl;

    unsafe {
        #[repr(C)]
        struct winsize {
            ws_row: c_ushort,
            ws_col: c_ushort,
            ws_xpixel: c_ushort,
            ws_ypixel: c_ushort
        }

        let mut size: winsize = zeroed();
        match ioctl::read_into(libc::STDOUT_FILENO, TIOCGWINSZ, &mut size) {
            Ok(_) => size.ws_col as usize, // TODO getCursorPosition
            Err(_) => 80,
        }
    }
}

fn write_and_flush(stdout: &mut io::Stdout, buf: &[u8]) -> Result<()> {
    try!(stdout.write_all(buf));
    try!(stdout.flush());
    Ok(())
}

// Control characters are treated as having zero width.
fn width(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(s)
}

/// Rewrite the currently edited line accordingly to the buffer content,
/// cursor position, and number of columns of the terminal.
fn refresh_line(s: &mut State, stdout: &mut io::Stdout) -> Result<()> {
    use std::fmt::Write;
    use unicode_width::UnicodeWidthChar;

    let buf = &s.buf;
    let mut start = 0;
    let mut w1 = width(&buf[start..s.pos]);
    while s.prompt_width + w1 >= s.cols {
        let ch = buf.char_at(start);
        start += ch.len_utf8();
        w1 -= UnicodeWidthChar::width(ch).unwrap_or(0);
    }
    let mut end = buf.len();
    let mut w2 = width(&buf[start..end]);
    while s.prompt_width + w2 > s.cols {
        let ch = buf.char_at_reverse(end);
        end -= ch.len_utf8();
        w2 -= UnicodeWidthChar::width(ch).unwrap_or(0);
    }

    let mut ab = String::new();
    // Cursor to left edge
    ab.push('\r');
    // Write the prompt and the current buffer content
    ab.push_str(s.prompt);
    ab.push_str(&s.buf[start..end]);
    // Erase to right
    ab.push_str("\x1b[0K");
    // Move cursor to original position.
    ab.write_fmt(format_args!("\r\x1b[{}C", w1 + s.prompt_width)).unwrap();
    write_and_flush(stdout, ab.as_bytes())
}

/// Insert the character 'c' at cursor current position.
fn edit_insert(s: &mut State, stdout: &mut io::Stdout, ch: char) -> Result<()> {
    if s.buf.len() < s.buf.capacity() {
        if s.buf.len() == s.pos {
            s.buf.push(ch);
            let size = ch.encode_utf8(&mut s.bytes).unwrap();
            s.pos += size;
            if s.prompt_width + width(&s.buf) < s.cols {
                // Avoid a full update of the line in the trivial case.
                write_and_flush(stdout, &mut s.bytes[0..size])
            } else {
                refresh_line(s, stdout)
            }
        } else {
            s.buf.insert(s.pos, ch);
            refresh_line(s, stdout)
        }
    } else {
        Ok(())
    }
}

/// Handles reading and editting the readline buffer.
/// It will also handle special inputs in an appropriate fashion
/// (e.g., C-c will exit readline)
fn readline_edit(prompt: &str) -> Result<String> {
    let mut stdout = io::stdout();
    try!(write_and_flush(&mut stdout, prompt.as_bytes()));

    let mut s = State {
        prompt: prompt,
        prompt_width: unicode_width::UnicodeWidthStr::width(prompt),
        buf: String::with_capacity(MAX_LINE),
        pos: 0,
//        oldpos: 0,
        cols: get_columns(),
        bytes: [0; 4],
    };
    let stdin = io::stdin();
    let mut chars = stdin.lock().chars();
    loop {
        let ch = try!(chars.next().unwrap());
        match char_to_key_press(ch) {
            KeyPress::CTRL_A => print!("Pressed C-a"),
            KeyPress::CTRL_B => print!("Pressed C-b"),
            KeyPress::CTRL_C => {
                return Err(from_errno(Errno::EAGAIN))
            },
            KeyPress::CTRL_D => print!("Pressed C-d"),
            KeyPress::CTRL_E => print!("Pressed C-e"),
            KeyPress::CTRL_F => print!("Pressed C-f"),
            KeyPress::CTRL_H => print!("Pressed C-h"),
            KeyPress::CTRL_K => print!("Pressed C-k"),
            KeyPress::CTRL_L => print!("Pressed C-l"),
            KeyPress::CTRL_N => print!("Pressed C-n"),
            KeyPress::CTRL_P => print!("Pressed C-p"),
            KeyPress::CTRL_T => print!("Pressed C-t"),
            KeyPress::CTRL_U => print!("Pressed C-u"),
            KeyPress::CTRL_W => print!("Pressed C-w"),
            KeyPress::ESC    => print!("Pressed esc") ,
            KeyPress::ENTER  => break,
            _      => try!(edit_insert(&mut s, &mut stdout, ch)),
        }
    }
    Ok(s.buf)
}

/// Readline method that will enable RAW mode, call the ```readline_edit()```
/// method and disable raw mode
fn readline_raw(prompt: &str) -> Result<String> {
    if is_a_tty() {
        let original_termios = try!(enable_raw_mode());
        let user_input = readline_edit(prompt);
        try!(disable_raw_mode(original_termios));
        println!("");
        user_input
    } else {
        readline_direct()
    }
}

fn readline_direct() -> Result<String> {
        let mut line = String::new();
        try!(io::stdin().read_line(&mut line));
        Ok(line)
}

/// This method will read a line from STDIN and will display a `prompt`
pub fn readline(prompt: &str) -> Result<String> {
    if is_unsupported_term() {
        // Write prompt and flush it to stdout
        let mut stdout = io::stdout();
        try!(write_and_flush(&mut stdout, prompt.as_bytes()));

        readline_direct()
    } else {
        readline_raw(prompt)
    }
}
