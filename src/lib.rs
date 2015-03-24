#![feature(libc)]
extern crate libc;

pub fn readline() -> Option<String> {
    // Buffer to hold readline input
    let buffer = String::new();

    let isatty = unsafe { libc::isatty(libc::STDIN_FILENO as i32) } != 0; 
    if isatty  {
        println!("stdin is a tty");
    } else {
        println!("stdin is not a tty");
    }

    buffer
}
