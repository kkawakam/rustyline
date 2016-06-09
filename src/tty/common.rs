extern crate libc;

/// Check to see if `fd` is a TTY
pub fn is_a_tty(fd: libc::c_int) -> bool {
    unsafe { libc::isatty(fd) != 0 }
}

