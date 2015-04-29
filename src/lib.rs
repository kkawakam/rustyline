extern crate libc;
extern crate nix;

use std::io;
use std::io::{Write, Read, Error, ErrorKind};
use nix::errno::Errno;
use nix::Error::Sys;
use nix::sys::termios;
use nix::sys::termios::{BRKINT, ICRNL, INPCK, ISTRIP, IXON, OPOST, CS8, ECHO, ICANON, IEXTEN, ISIG, VMIN, VTIME};

static MAX_LINE: i32 = 4096;
static UNSUPPORTED_TERM: [&'static str; 3] = ["dumb","cons25","emacs"];

fn is_a_tty() -> bool {
    let isatty = unsafe { libc::isatty(libc::STDIN_FILENO as i32) } != 0;
    isatty
}

fn is_unsupported_term() -> bool {
    let term = std::env::var("TERM").ok().unwrap();
    let mut unsupported = false;
    for iter in &UNSUPPORTED_TERM {
        unsupported = term == *iter
    }
    unsupported
}

fn enable_raw_mode() -> Result<termios::Termios, nix::Error> {
    if !is_a_tty() {
        Err(Sys(Errno::ENOTTY)) 
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

fn disable_raw_mode(original_termios: termios::Termios) -> Result<(), nix::Error> {
    try!(termios::tcsetattr(libc::STDIN_FILENO, termios::TCSAFLUSH, &original_termios));
    Ok(())
}

fn readline_edit() -> Result<String, io::Error> {
    let mut buffer = Vec::new();
    let mut input: [u8; 1] = [0];
    let numread = io::stdin().read(&mut input).unwrap();
    buffer.push(input[0]);
    println!("Read #{:?} bytes with a value of{:?}",numread,input[0]);
    Ok(String::from_utf8(buffer).unwrap())
}

fn readline_raw() -> Result<String, io::Error> {
    if is_a_tty() {
        let original_termios = match enable_raw_mode() {
            Err(Sys(Errno::ENOTTY)) => return Err(Error::new(ErrorKind::Other, "Not a TTY")),
            Err(Sys(Errno::EBADF))  => return Err(Error::new(ErrorKind::Other, "Not a file descriptor")),
            Err(..)                 => return Err(Error::new(ErrorKind::Other, "Unknown Error")),
            Ok(term)                => term
        };
        
        let user_input = readline_edit();

        match disable_raw_mode(original_termios) {
            Err(..) => return Err(Error::new(ErrorKind::Other, "Failed to revert to original termios")),
            Ok(..)  => ()
        }

        user_input
    } else {

        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(_) => Ok(line),
            Err(e) => Err(e),
        }
    }
}

pub fn readline(prompt: &'static str) -> Result<String, io::Error> {
    // Write prompt and flush it to stdout
    let mut stdout = io::stdout();
    try!(stdout.write(prompt.as_bytes()));
    try!(stdout.flush());

    if is_unsupported_term() {
        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(_) => Ok(line),
            Err(e) => Err(e),
        }
    } else {
        readline_raw()
    }
}
