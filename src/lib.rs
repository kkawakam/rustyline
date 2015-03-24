#![feature(libc)]
extern crate libc;

use std::io::Write;
use std::io;


fn isatty() -> bool {
    let isatty = unsafe { libc::isatty(libc::STDIN_FILENO as i32) } != 0;
    isatty
}

pub fn readline(prompt: &'static str) -> Option<String> {
    // Write prompt and flush it to stdout
    let mut stdout = io::stdout();
    stdout.write(prompt.as_bytes());
    stdout.flush();


    let mut buffer = String::new();
    if isatty() {
        Some(buffer)
    } else {
        None
    }
}

fn read_handler(buffer: String) -> String {
   buffer
}
