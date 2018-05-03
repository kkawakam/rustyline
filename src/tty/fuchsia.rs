//! Fuchsia specific definitions
use std::io::{self, Read, Stdout, Write};
use std::mem;
use std::sync::atomic;
use libc;
use char_iter;

use config::Config;
use consts::{self, KeyPress};
use error;
use Result;
use super::{RawMode, RawReader, Term};
use fuchsia_device::pty;

const STDIN_FILENO: usize = 0;
const STDOUT_FILENO: usize = 1;

fn get_win_size() -> (usize, usize) {
    match pty::get_window_size() {
        Ok(size) => {
            (size.width as usize, size.height as usize)
        }
        _ => (80, 24),
    }
}

struct StdinRaw {}

impl Read for StdinRaw {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let res = unsafe {
                libc::read(
                    STDIN_FILENO as i32,
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len() as libc::size_t,
                )
            };
            if res == -1 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error);
                }
            } else {
                return Ok(res as usize);
            }
        }
    }
}

pub type Mode = ConsoleMode;

#[derive(Clone, Copy, Debug)]
pub struct ConsoleMode {}

impl RawMode for Mode {
    /// RAW mode is never on w/ Fuchsia
    fn disable_raw_mode(&self) -> Result<()> {
        Ok(())
    }
}

pub type Terminal = Console;

#[derive(Clone, Debug)]
pub struct Console {
    stdin_isatty: bool,
}

pub struct FuchsiaRawReader {
    chars: char_iter::Chars<StdinRaw>,
}

impl FuchsiaRawReader {
    pub fn new() -> Result<FuchsiaRawReader> {
        let stdin = StdinRaw {};
        Ok(FuchsiaRawReader {
            chars: char_iter::chars(stdin),
        })
    }
    fn escape_sequence(&mut self) -> Result<KeyPress> {
        // Read the next two bytes representing the escape sequence.
        let seq1 = try!(self.next_char());
        if seq1 == '[' {
            // ESC [ sequences.
            let seq2 = try!(self.next_char());
            if seq2.is_digit(10) {
                // Extended escape, read additional byte.
                let seq3 = try!(self.next_char());
                if seq3 == '~' {
                    Ok(match seq2 {
                        '1' | '7' => KeyPress::Home, // '1': xterm
                        '3' => KeyPress::Delete,
                        '4' | '8' => KeyPress::End, // '4': xterm
                        '5' => KeyPress::PageUp,
                        '6' => KeyPress::PageDown,
                        _ => {
                            debug!(target: "rustyline", "unsupported esc sequence: ESC{:?}{:?}{:?}", seq1, seq2, seq3);
                            KeyPress::UnknownEscSeq
                        }
                    })
                } else {
                    debug!(target: "rustyline", "unsupported esc sequence: ESC{:?}{:?}{:?}", seq1, seq2, seq3);
                    Ok(KeyPress::UnknownEscSeq)
                }
            } else {
                Ok(match seq2 {
                    'A' => KeyPress::Up, // ANSI
                    'B' => KeyPress::Down,
                    'C' => KeyPress::Right,
                    'D' => KeyPress::Left,
                    'F' => KeyPress::End,
                    'H' => KeyPress::Home,
                    _ => {
                        debug!(target: "rustyline", "unsupported esc sequence: ESC{:?}{:?}", seq1, seq2);
                        KeyPress::UnknownEscSeq
                    }
                })
            }
        } else if seq1 == 'O' {
            // ESC O sequences.
            let seq2 = try!(self.next_char());
            Ok(match seq2 {
                'A' => KeyPress::Up,
                'B' => KeyPress::Down,
                'C' => KeyPress::Right,
                'D' => KeyPress::Left,
                'F' => KeyPress::End,
                'H' => KeyPress::Home,
                _ => {
                    debug!(target: "rustyline", "unsupported esc sequence: ESC{:?}{:?}", seq1, seq2);
                    KeyPress::UnknownEscSeq
                }
            })
        } else {
            // TODO ESC-R (r): Undo all changes made to this line.
            Ok(match seq1 {
                '\x08' => KeyPress::Meta('\x08'), // Backspace
                '-' => KeyPress::Meta('-'),
                '0'...'9' => KeyPress::Meta(seq1),
                '<' => KeyPress::Meta('<'),
                '>' => KeyPress::Meta('>'),
                'b' | 'B' => KeyPress::Meta('B'),
                'c' | 'C' => KeyPress::Meta('C'),
                'd' | 'D' => KeyPress::Meta('D'),
                'f' | 'F' => KeyPress::Meta('F'),
                'l' | 'L' => KeyPress::Meta('L'),
                'n' | 'N' => KeyPress::Meta('N'),
                'p' | 'P' => KeyPress::Meta('P'),
                't' | 'T' => KeyPress::Meta('T'),
                'u' | 'U' => KeyPress::Meta('U'),
                'y' | 'Y' => KeyPress::Meta('Y'),
                '\x7f' => KeyPress::Meta('\x7f'), // Delete
                _ => {
                    debug!(target: "rustyline", "unsupported esc sequence: M-{:?}", seq1);
                    KeyPress::UnknownEscSeq
                }
            })
        }
    }
}

// TODO properly set raw mode, process escape keys

impl RawReader for FuchsiaRawReader {
    fn next_key(&mut self) -> Result<KeyPress> {
        let c = try!(self.next_char());

        let mut key = consts::char_to_key_press(c);
        if key == KeyPress::Esc {
            // TODO
            debug!(target: "rustyline", "ESC + {:?} currently unsupported", key);
        }

        Ok(key)
    }

    fn next_char(&mut self) -> Result<char> {
        match self.chars.next() {
            Some(ch) => Ok(ch?),
            None => Err(error::ReadlineError::Eof),
        }
    }
}

impl Term for Console {
    type Reader = FuchsiaRawReader;
    type Writer = Stdout;
    type Mode = Mode;

    fn new() -> Console {
        let stdin_isatty = true;
        Console {
            stdin_isatty: stdin_isatty,
        }
    }

    /// Checking for an unsupported TERM in fuchsia is a no-op
    fn is_unsupported(&self) -> bool {
        false
    }

    fn is_stdin_tty(&self) -> bool {
        self.stdin_isatty
    }

    /// Try to get the number of columns in the current terminal,
    /// or assume 80 if it fails.
    fn get_columns(&self) -> usize {
        let (cols, _) = get_win_size();
        cols
    }

    /// Try to get the number of rows in the current terminal,
    /// or assume 24 if it fails.
    fn get_rows(&self) -> usize {
        let (_, rows) = get_win_size();
        rows
    }

    /// Enable RAW mode for the terminal. No termios support, so this fakes it
    fn enable_raw_mode(&self) -> Result<Mode> {
        Ok(Mode {})
    }

    fn create_reader(&self, _: &Config) -> Result<FuchsiaRawReader> {
        FuchsiaRawReader::new()
    }

    fn create_writer(&self) -> Stdout {
        io::stdout()
    }

    fn sigwinch(&self) -> bool {
        false
    }

    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self, w: &mut Write) -> Result<()> {
        try!(w.write_all(b"\x1b[H\x1b[2J"));
        try!(w.flush());
        Ok(())
    }
}
