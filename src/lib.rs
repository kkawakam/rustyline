#![feature(libc)]
extern crate libc;



fn isatty() -> bool {
    let isatty = unsafe { libc::isatty(libc::STDIN_FILENO as i32) } != 0;
    isatty
}

pub fn readline() -> Option<String> {
    if isatty() {
        Some(buffer)
    } else {
        None
    }
}



