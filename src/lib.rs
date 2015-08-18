//!Readline for Rust
//!
//!This implementation is based on [Antirez's Linenoise](https://github.com/antirez/linenoise)
//!
//!# Example
//!
//!Usage
//!
//!```
//!let readline = rustyline::readline(">> ", &mut None);
//!match readline {
//!     Ok(line) => println!("Line: {:?}",line),
//!     Err(_)   => println!("No input"),
//! }
//!```
#![feature(drain)]
#![feature(io)]
#![feature(str_char)]
#![feature(unicode)]
extern crate libc;
extern crate nix;
extern crate unicode_width;

#[allow(non_camel_case_types)]
mod consts;
pub mod error;
pub mod history;

use std::result;
use std::io;
use std::io::{Write, Read};
use nix::errno::Errno;
use nix::sys::termios;

use consts::{KeyPress, char_to_key_press};
use history::History;

/// The error type for I/O and Linux Syscalls (Errno)
pub type Result<T> = result::Result<T, error::ReadlineError>;

// Represent the state during line editing.
struct State<'prompt> {
    prompt: &'prompt str, // Prompt to display
    prompt_width: usize, // Prompt Unicode width
    buf: String, // Edited line buffer
    pos: usize, // Current cursor position (byte position)
    cols: usize, // Number of columns in terminal
    history_index: usize, // The history index we are currently editing.
    bytes: [u8; 4],
}

impl<'prompt> State<'prompt> {
    fn new(prompt: &'prompt str, capacity: usize, cols: usize) -> State<'prompt> {
        State {
            prompt: prompt,
            prompt_width: unicode_width::UnicodeWidthStr::width(prompt),
            buf: String::with_capacity(capacity),
            pos: 0,
            cols: cols,
            history_index: 0,
            bytes: [0; 4],
        }
    }
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

fn write_and_flush(w: &mut Write, buf: &[u8]) -> Result<()> {
    try!(w.write_all(buf));
    try!(w.flush());
    Ok(())
}

/// Clear the screen. Used to handle ctrl+l
fn clear_screen(stdout: &mut io::Stdout) -> Result<()> {
    write_and_flush(stdout, b"\x1b[H\x1b[2J")
}

/// Beep, used for completion when there is nothing to complete or when all
/// the choices were already shown.
/*fn beep() -> Result<()> {
    write_and_flush(&mut io::stderr(), b"\x07")
}*/

// Control characters are treated as having zero width.
fn width(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(s)
}

/// Rewrite the currently edited line accordingly to the buffer content,
/// cursor position, and number of columns of the terminal.
fn refresh_line(s: &mut State, stdout: &mut Write) -> Result<()> {
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

/// Insert the character `ch` at cursor current position.
fn edit_insert(s: &mut State, stdout: &mut Write, ch: char) -> Result<()> {
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
            s.pos += ch.len_utf8();
            refresh_line(s, stdout)
        }
    } else {
        Ok(())
    }
}

/// Move cursor on the left.
fn edit_move_left(s: &mut State, stdout: &mut Write) -> Result<()> {
    if s.pos > 0 {
        let ch = s.buf.char_at_reverse(s.pos);
        s.pos -= ch.len_utf8();
        refresh_line(s, stdout)
    } else {
        Ok(())
    }
}

/// Move cursor on the right.
fn edit_move_right(s: &mut State, stdout: &mut Write) -> Result<()> {
    if s.pos != s.buf.len() {
        let ch = s.buf.char_at(s.pos);
        s.pos += ch.len_utf8();
        refresh_line(s, stdout)
    } else {
        Ok(())
    }
}

/// Move cursor to the start of the line.
fn edit_move_home(s: &mut State, stdout: &mut Write) -> Result<()> {
    if s.pos > 0 {
        s.pos = 0;
        refresh_line(s, stdout)
    } else {
        Ok(())
    }
}

/// Move cursor to the end of the line.
fn edit_move_end(s: &mut State, stdout: &mut Write) -> Result<()> {
    if s.pos != s.buf.len() {
        s.pos = s.buf.len();
        refresh_line(s, stdout)
    } else {
        Ok(())
    }
}

/// Delete the character at the right of the cursor without altering the cursor
/// position. Basically this is what happens with the "Delete" keyboard key.
fn edit_delete(s: &mut State, stdout: &mut Write) -> Result<()> {
    if s.buf.len() > 0 && s.pos < s.buf.len() {
        s.buf.remove(s.pos);
        refresh_line(s, stdout)
    } else {
        Ok(())
    }
}

/// Backspace implementation.
fn edit_backspace(s: &mut State, stdout: &mut Write) -> Result<()> {
    if s.pos > 0 && s.buf.len() > 0 {
        let ch = s.buf.char_at_reverse(s.pos);
        s.pos -= ch.len_utf8();
        s.buf.remove(s.pos);
        refresh_line(s, stdout)
    } else {
        Ok(())
    }
}

/// Kill the text from point to the end of the line.
fn edit_kill_line(s: &mut State, stdout: &mut Write) -> Result<()> {
    if s.buf.len() > 0 && s.pos < s.buf.len() {
        s.buf.drain(s.pos..);
        refresh_line(s, stdout)
    } else {
        Ok(())
    }
}

/// Kill backward from point to the beginning of the line.
fn edit_discard_line(s: &mut State, stdout: &mut Write) -> Result<()> {
    if s.pos > 0 && s.buf.len() > 0 {
        s.buf.drain(..s.pos);
        s.pos = 0;
        refresh_line(s, stdout)
    } else {
        Ok(())
    }
}

/// Exchange the char before cursor with the character at cursor.
fn edit_transpose_chars(s: &mut State, stdout: &mut Write) -> Result<()> {
    if s.pos > 0 && s.pos < s.buf.len() {
        let ch = s.buf.remove(s.pos);
        let size = ch.len_utf8();
        let och = s.buf.char_at_reverse(s.pos);
        let osize = och.len_utf8();
        s.buf.insert(s.pos - osize, ch);
        if s.pos != s.buf.len()-size {
            s.pos += size;
        } else {
            if size >= osize {
                s.pos += size - osize;
            } else {
                s.pos -= osize - size;
            }
        }
        refresh_line(s, stdout)
    } else {
        Ok(())
    }
}

/// Delete the previous word, maintaining the cursor at the start of the
/// current word.
fn edit_delete_prev_word(s: &mut State, stdout: &mut Write) -> Result<()> {
    if s.pos > 0 {
        let old_pos = s.pos;
        let mut ch = s.buf.char_at_reverse(s.pos);
        while s.pos > 0 && ch.is_whitespace() {
            s.pos -= ch.len_utf8();
            ch = s.buf.char_at_reverse(s.pos);
        }
        while s.pos > 0 && !ch.is_whitespace() {
            s.pos -= ch.len_utf8();
            ch = s.buf.char_at_reverse(s.pos);
        }
        s.buf.drain(s.pos..old_pos);
        refresh_line(s, stdout)
    } else {
        Ok(())
    }
}

/// Substitute the currently edited line with the next or previous history
/// entry.
fn edit_history_next(s: &mut State, history: &mut History, stdout: &mut Write, prev: bool) -> Result<()> {
    if history.len() > 1 {
        unimplemented!();
        //s.buf = ;
        //s.pos = s.buf.len();
        //refresh_line(s, stdout)
    } else {
        Ok(())
    }
}

/// Handles reading and editting the readline buffer.
/// It will also handle special inputs in an appropriate fashion
/// (e.g., C-c will exit readline)
fn readline_edit(prompt: &str, history: &mut Option<History>) -> Result<String> {
    let mut stdout = io::stdout();
    try!(write_and_flush(&mut stdout, prompt.as_bytes()));

    let mut s = State::new(prompt, MAX_LINE, get_columns());
    let stdin = io::stdin();
    let mut chars = stdin.lock().chars();
    loop {
        let ch = try!(chars.next().unwrap());
        match char_to_key_press(ch) {
            KeyPress::CTRL_A => try!(edit_move_home(&mut s, &mut stdout)), // Move to the beginning of line.
            KeyPress::CTRL_B => try!(edit_move_left(&mut s, &mut stdout)), // Move back a character.
            KeyPress::CTRL_C => {
                return Err(from_errno(Errno::EAGAIN))
            },
            KeyPress::CTRL_D => {
                if s.buf.len() > 0 { // Delete one character at point.
                    try!(edit_delete(&mut s, &mut stdout))
                } else {
                    break
                }
            },
            KeyPress::CTRL_E => try!(edit_move_end(&mut s, &mut stdout)), // Move to the end of line.
            KeyPress::CTRL_F => try!(edit_move_right(&mut s, &mut stdout)), // Move forward a character.
            KeyPress::CTRL_H | KeyPress::BACKSPACE => try!(edit_backspace(&mut s, &mut stdout)), // Delete one character backward.
            KeyPress::CTRL_K => try!(edit_kill_line(&mut s, &mut stdout)), // Kill the text from point to the end of the line.
            KeyPress::CTRL_L => { // Clear the screen leaving the current line at the top of the screen.
                try!(clear_screen(&mut stdout));
                try!(refresh_line(&mut s, &mut stdout))
            },
            KeyPress::CTRL_N => { // Fetch the next command from the history list.
                if history.is_some() {
                    try!(edit_history_next(&mut s, history.as_mut().unwrap(), &mut stdout, false))
                }
            },
            KeyPress::CTRL_P => { // Fetch the previous command from the history list.
                if history.is_some() {
                    try!(edit_history_next(&mut s, history.as_mut().unwrap(), &mut stdout, true))
                }
            },
            KeyPress::CTRL_T => try!(edit_transpose_chars(&mut s, &mut stdout)), // Exchange the char before cursor with the character at cursor.
            KeyPress::CTRL_U => try!(edit_discard_line(&mut s, &mut stdout)), // Kill backward from point to the beginning of the line.
            KeyPress::CTRL_W => try!(edit_delete_prev_word(&mut s, &mut stdout)), // Kill the word behind point, using white space as a word boundary
            KeyPress::ESC    => print!("Pressed esc"),
            KeyPress::ENTER  => break, // Accept the line regardless of where the cursor is.
            _      => try!(edit_insert(&mut s, &mut stdout, ch)), // Insert the character typed.
        }
    }
    Ok(s.buf)
}

/// Readline method that will enable RAW mode, call the ```readline_edit()```
/// method and disable raw mode
fn readline_raw(prompt: &str, history: &mut Option<History>) -> Result<String> {
    if is_a_tty() {
        let original_termios = try!(enable_raw_mode());
        let user_input = readline_edit(prompt, history);
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
pub fn readline(prompt: &str, history: &mut Option<History>) -> Result<String> {
    if is_unsupported_term() {
        // Write prompt and flush it to stdout
        let mut stdout = io::stdout();
        try!(write_and_flush(&mut stdout, prompt.as_bytes()));

        readline_direct()
    } else {
        readline_raw(prompt, history)
    }
}

#[cfg(test)]
mod test {
    use State;

    fn init_state(line: &str, pos: usize, cols: usize) -> State<'static> {
        State {
            prompt: "",
            prompt_width: 0,
            buf: String::from(line),
            pos: pos,
            cols: cols,
            history_index: 0,
            bytes: [0; 4],
        }
    }

    #[test]
    fn insert() {
        let mut s = State::new("", 128, 80);
        let mut stdout = ::std::io::sink();
        super::edit_insert(&mut s, &mut stdout, 'α').unwrap();
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);

        super::edit_insert(&mut s, &mut stdout, 'ß').unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);

        s.pos = 0;
        super::edit_insert(&mut s, &mut stdout, 'γ').unwrap();
        assert_eq!("γαß", s.buf);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn moves() {
        let mut s = init_state("αß", 4, 80);
        let mut stdout = ::std::io::sink();
        super::edit_move_left(&mut s, &mut stdout).unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(2, s.pos);

        super::edit_move_right(&mut s, &mut stdout).unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);

        super::edit_move_home(&mut s, &mut stdout).unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(0, s.pos);

        super::edit_move_end(&mut s, &mut stdout).unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);
    }

    #[test]
    fn delete() {
        let mut s = init_state("αß", 2, 80);
        let mut stdout = ::std::io::sink();
        super::edit_delete(&mut s, &mut stdout).unwrap();
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);

        super::edit_backspace(&mut s, &mut stdout).unwrap();
        assert_eq!("", s.buf);
        assert_eq!(0, s.pos);
    }

    #[test]
    fn kill() {
        let mut s = init_state("αßγδε", 6, 80);
        let mut stdout = ::std::io::sink();
        super::edit_kill_line(&mut s, &mut stdout).unwrap();
        assert_eq!("αßγ", s.buf);
        assert_eq!(6, s.pos);

        s.pos = 4;
        super::edit_discard_line(&mut s, &mut stdout).unwrap();
        assert_eq!("γ", s.buf);
        assert_eq!(0, s.pos);
    }

    #[test]
    fn transpose() {
        let mut s = init_state("aßc", 1, 80);
        let mut stdout = ::std::io::sink();
        super::edit_transpose_chars(&mut s, &mut stdout).unwrap();
        assert_eq!("ßac", s.buf);
        assert_eq!(3, s.pos);

        s.buf = String::from("aßc");
        s.pos = 3;
        super::edit_transpose_chars(&mut s, &mut stdout).unwrap();
        assert_eq!("acß", s.buf);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn delete_prev_word() {
        let mut s = init_state("a ß  c", 6, 80);
        let mut stdout = ::std::io::sink();
        super::edit_delete_prev_word(&mut s, &mut stdout).unwrap();
        assert_eq!("a c", s.buf);
        assert_eq!(2, s.pos);
    }
}
