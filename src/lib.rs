extern crate libc;

use std::env;
use std::io::{Write, Read};
use std::io;

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

pub fn readline(prompt: &'static str) -> Option<String> {
    // Write prompt and flush it to stdout
    let mut stdout = io::stdout();
    stdout.write(prompt.as_bytes());
    stdout.flush();

    if is_unsupported_term() {
        Some(read_handler())
    } else {
        None
    }
}

fn read_handler() -> String {
    let mut buffer = Vec::new();
    let mut input: [u8; 1] = [0];

    let numread = io::stdin().take(1).read(&mut input).unwrap();

    println!("Read #{:?} bytes with a value of{:?}",numread,input[0]);
    buffer.push(input[0]);

    String::from_utf8(buffer).unwrap()
}
