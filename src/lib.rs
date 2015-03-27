extern crate libc;

use std::io::{Write, Read};
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

    if isatty() {
        Some(read_handler())
    } else {
        None
    }
}

fn read_handler() -> String {
    let mut buffer = Vec::new();
    let mut input: [u8; 1] = [0];

    // Create handle to stdin 
    let mut stdin = io::stdin();
    let numread = stdin.take(1).read(&mut input).unwrap();

    println!("Read #{:?} bytes with a value of{:?}",numread,input[0]);
    buffer.push(input[0]);

    String::from_utf8(buffer).unwrap()
}
