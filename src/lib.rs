extern crate libc;
extern crate nix;

use std::io;
use std::io::{Write, Read};
use std::env;
use nix::errno;

static MAX_LINE: i32 = 4096;
static UNSUPPORTED_TERM: [&'static str; 3] = ["dumb","cons25","emacs"];


fn is_a_tty() -> bool {
    let isatty = unsafe { libc::isatty(libc::STDIN_FILENO as i32) } != 0;
    isatty
}

fn is_unsupported_term() -> bool {
    let term = env::var("TERM").ok().unwrap();

    let mut unsupported = false;
    for iter in &UNSUPPORTED_TERM {
        unsupported = (term == *iter)
    }
    unsupported
}

fn enable_raw_mode(fd: i64) -> Result<(), errno::Errno> {
    Ok(())
}

fn readline_raw() -> Result<String, io::Error> {
    let mut buffer = Vec::new();
    let mut input: [u8; 1] = [0];

    if is_a_tty() {
        let numread = io::stdin().take(1).read(&mut input).unwrap();

        println!("Read #{:?} bytes with a value of{:?}",numread,input[0]);
        buffer.push(input[0]);

        Ok(String::from_utf8(buffer).unwrap())
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
    stdout.write(prompt.as_bytes());
    stdout.flush();

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
