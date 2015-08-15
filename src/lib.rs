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
extern crate libc;
extern crate nix;

#[allow(non_camel_case_types)]
pub mod consts;
pub mod error;

use std::result;
use std::io;
use std::str;
use std::io::{Write, Read};
use nix::errno::Errno;
use nix::sys::termios;
use nix::sys::termios::{BRKINT, ICRNL, INPCK, ISTRIP, IXON, OPOST, CS8, ECHO, ICANON, IEXTEN, ISIG, VMIN, VTIME};
use consts::{KeyPress, u8_to_key_press};

/// The error type for I/O and Linux Syscalls (Errno)
pub type Result<T> = result::Result<T, error::ReadlineError>;

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
    match std::env::var("TERM") {
        Ok(term) => {
            let mut unsupported = false;
            for iter in &UNSUPPORTED_TERM {
                unsupported = term == *iter
            }
            unsupported
        }
        Err(_) => false
    }
}

/// Enable raw mode for the TERM
fn enable_raw_mode() -> Result<termios::Termios> {
    if !is_a_tty() {
        Err(error::ReadlineError
                          ::from(nix::Error
                                    ::from_errno(Errno::ENOTTY)))
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

/// Handles reading and editting the readline buffer.
/// It will also handle special inputs in an appropriate fashion
/// (e.g., C-c will exit readline)
fn readline_edit() -> Result<String> {
    let mut buffer = String::with_capacity(MAX_LINE);
    let mut input: [u8; 4] = [0; 4]; // UTF-8 can be max 4 bytes
    loop {
        if io::stdin().read(&mut input).is_ok()
        {
            match u8_to_key_press(input[0]) {
                KeyPress::CTRL_A => print!("Pressed C-a"),
                KeyPress::CTRL_B => print!("Pressed C-b"),
                KeyPress::CTRL_C => print!("Pressed C-c"),
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
                KeyPress::ESC    => print!("Pressed esc"),
                KeyPress::ENTER  => break,
                _      => {
                    match str::from_utf8(&input) {
                        Ok(s) => buffer.push_str(s) ,
                        Err(_) => panic!("Invalid UTF-8 Character"),  
                    }
                }
            }
        }
        
    }
    Ok(buffer)
}

/// Readline method that will enable RAW mode, call the ```readline_edit()```
/// method and disable raw mode
fn readline_raw() -> Result<String> {
    if is_a_tty() {
        let original_termios = try!(enable_raw_mode());
        let user_input = readline_edit();
        try!(disable_raw_mode(original_termios));
        user_input
    } else {
        let mut line = String::new();
        try!(io::stdin().read_line(&mut line));
        Ok(line)
    }
}

/// This method will read a line from STDIN and will display a `prompt`
pub fn readline(prompt: &'static str) -> Result<String> {
    // Write prompt and flush it to stdout
    let mut stdout = io::stdout();
    try!(stdout.write(prompt.as_bytes()));
    try!(stdout.flush());

    if is_unsupported_term() {
        let mut line = String::new();
        try!(io::stdin().read_line(&mut line));
        Ok(line)
    } else {
        readline_raw()
    }
}
