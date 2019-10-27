//! Windows specific definitions
use std::io::{self, ErrorKind, Write};
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;

use log::debug;
use unicode_width::UnicodeWidthChar;
use winapi::shared::minwindef::{DWORD, FALSE, TRUE, WORD};
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::synchapi::{CreateEventW, ResetEvent, SetEvent};
use winapi::um::wincon::{self, CONSOLE_SCREEN_BUFFER_INFO};
use winapi::um::winnt::{CHAR, HANDLE};
use winapi::um::{consoleapi, processenv, winbase, winuser};

use super::{Event, RawMode, RawReader, Renderer, Term};
use crate::config::{BellStyle, ColorMode, Config, OutputStreamType};
use crate::error;
use crate::highlight::Highlighter;
use crate::keys::{self, KeyPress};
use crate::layout::{Layout, Position};
use crate::line_buffer::LineBuffer;
use crate::Result;

const STDIN_FILENO: DWORD = winbase::STD_INPUT_HANDLE;
const STDOUT_FILENO: DWORD = winbase::STD_OUTPUT_HANDLE;
const STDERR_FILENO: DWORD = winbase::STD_ERROR_HANDLE;

fn get_std_handle(fd: DWORD) -> Result<HANDLE> {
    let handle = unsafe { processenv::GetStdHandle(fd) };
    check_handle(handle)
}

fn check_handle(handle: HANDLE) -> Result<HANDLE> {
    if handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())?;
    } else if handle.is_null() {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "no stdio handle available for this process",
        ))?;
    }
    Ok(handle)
}

macro_rules! check {
    ($funcall:expr) => {{
        let rc = unsafe { $funcall };
        if rc == FALSE {
            Err(io::Error::last_os_error())?;
        }
        rc
    }};
}

fn get_win_size(handle: HANDLE) -> (usize, usize) {
    let mut info = unsafe { mem::zeroed() };
    match unsafe { wincon::GetConsoleScreenBufferInfo(handle, &mut info) } {
        FALSE => (80, 24),
        _ => (
            info.dwSize.X as usize,
            (1 + info.srWindow.Bottom - info.srWindow.Top) as usize,
        ), // (info.srWindow.Right - info.srWindow.Left + 1)
    }
}

fn get_console_mode(handle: HANDLE) -> Result<DWORD> {
    let mut original_mode = 0;
    check!(consoleapi::GetConsoleMode(handle, &mut original_mode));
    Ok(original_mode)
}

#[cfg(not(test))]
pub type Mode = ConsoleMode;

#[derive(Clone, Debug)]
pub struct ConsoleMode {
    original_stdin_mode: DWORD,
    stdin_handle: HANDLE,
    original_stdstream_mode: Option<DWORD>,
    stdstream_handle: HANDLE,
    raw_mode: Arc<AtomicBool>,
}

impl RawMode for ConsoleMode {
    /// Disable RAW mode for the terminal.
    fn disable_raw_mode(&self) -> Result<()> {
        check!(consoleapi::SetConsoleMode(
            self.stdin_handle,
            self.original_stdin_mode,
        ));
        if let Some(original_stdstream_mode) = self.original_stdstream_mode {
            check!(consoleapi::SetConsoleMode(
                self.stdstream_handle,
                original_stdstream_mode,
            ));
        }
        self.raw_mode.store(false, Ordering::SeqCst);
        Ok(())
    }
}

/// Console input reader
pub struct ConsoleRawReader {
    handle: HANDLE,
    // external print reader
    pipe_reader: Option<Arc<AsyncPipe>>,
}

impl ConsoleRawReader {
    fn create(pipe_reader: Option<Arc<AsyncPipe>>) -> Result<ConsoleRawReader> {
        let handle = get_std_handle(STDIN_FILENO)?;
        Ok(ConsoleRawReader {
            handle,
            pipe_reader,
        })
    }

    fn select(&mut self) -> Result<Event> {
        use std::convert::TryInto;
        use winapi::um::synchapi::WaitForMultipleObjects;
        use winapi::um::winbase::{INFINITE, WAIT_OBJECT_0};

        let pipe_reader = self.pipe_reader.as_ref().unwrap();
        let handles = [self.handle, pipe_reader.event.0];
        let n = handles.len().try_into().unwrap();
        loop {
            let rc = unsafe { WaitForMultipleObjects(n, handles.as_ptr(), FALSE, INFINITE) };
            if rc == WAIT_OBJECT_0 + 0 {
                let mut count = 0;
                check!(consoleapi::GetNumberOfConsoleInputEvents(
                    self.handle,
                    &mut count
                ));
                match read_input(self.handle, count)? {
                    KeyPress::UnknownEscSeq => continue, // no relevant
                    key => return Ok(Event::KeyPress(key)),
                };
            } else if rc == WAIT_OBJECT_0 + 1 {
                debug!(target: "rustyline", "ExternalPrinter::receive");
                check!(ResetEvent(pipe_reader.event.0));
                match pipe_reader.receiver.recv() {
                    Ok(msg) => return Ok(Event::ExternalPrint(msg)),
                    Err(e) => Err(io::Error::new(io::ErrorKind::InvalidInput, e))?,
                }
            } else {
                Err(io::Error::last_os_error())?
            }
        }
    }
}

impl RawReader for ConsoleRawReader {
    fn wait_for_input(&mut self, single_esc_abort: bool) -> Result<Event> {
        match self.pipe_reader {
            Some(_) => self.select(),
            None => self.next_key(single_esc_abort).map(Event::KeyPress),
        }
    }

    fn next_key(&mut self, _: bool) -> Result<KeyPress> {
        read_input(self.handle, std::u32::MAX)
    }

    fn read_pasted_text(&mut self) -> Result<String> {
        unimplemented!()
    }
}

fn read_input(handle: HANDLE, max_count: u32) -> Result<KeyPress> {
    use std::char::decode_utf16;
    use winapi::um::wincon::{
        LEFT_ALT_PRESSED, LEFT_CTRL_PRESSED, RIGHT_ALT_PRESSED, RIGHT_CTRL_PRESSED, SHIFT_PRESSED,
    };

    let mut rec: wincon::INPUT_RECORD = unsafe { mem::zeroed() };
    let mut count = 0;
    let mut total = 0;
    let mut surrogate = 0;
    loop {
        if total >= max_count {
            return Ok(KeyPress::UnknownEscSeq);
        }
        // TODO GetNumberOfConsoleInputEvents
        check!(consoleapi::ReadConsoleInputW(
            handle, &mut rec, 1 as DWORD, &mut count,
        ));
        total += count;

        if rec.EventType == wincon::WINDOW_BUFFER_SIZE_EVENT {
            SIGWINCH.store(true, Ordering::SeqCst);
            debug!(target: "rustyline", "SIGWINCH");
            return Err(error::ReadlineError::WindowResize); // sigwinch +
                                                            // err => err
                                                            // ignored
        } else if rec.EventType != wincon::KEY_EVENT {
            continue;
        }
        let key_event = unsafe { rec.Event.KeyEvent() };
        // writeln!(io::stderr(), "key_event: {:?}", key_event).unwrap();
        if key_event.bKeyDown == 0 && key_event.wVirtualKeyCode != winuser::VK_MENU as WORD {
            continue;
        }
        // key_event.wRepeatCount seems to be always set to 1 (maybe because we only
        // read one character at a time)

        let alt_gr = key_event.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_ALT_PRESSED)
            == (LEFT_CTRL_PRESSED | RIGHT_ALT_PRESSED);
        let alt = key_event.dwControlKeyState & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED) != 0;
        let ctrl = key_event.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED) != 0;
        let meta = alt && !alt_gr;
        let shift = key_event.dwControlKeyState & SHIFT_PRESSED != 0;

        let utf16 = unsafe { *key_event.uChar.UnicodeChar() };
        if utf16 == 0 {
            match i32::from(key_event.wVirtualKeyCode) {
                winuser::VK_LEFT => {
                    return Ok(if ctrl {
                        KeyPress::ControlLeft
                    } else if shift {
                        KeyPress::ShiftLeft
                    } else {
                        KeyPress::Left
                    });
                }
                winuser::VK_RIGHT => {
                    return Ok(if ctrl {
                        KeyPress::ControlRight
                    } else if shift {
                        KeyPress::ShiftRight
                    } else {
                        KeyPress::Right
                    });
                }
                winuser::VK_UP => {
                    return Ok(if ctrl {
                        KeyPress::ControlUp
                    } else if shift {
                        KeyPress::ShiftUp
                    } else {
                        KeyPress::Up
                    });
                }
                winuser::VK_DOWN => {
                    return Ok(if ctrl {
                        KeyPress::ControlDown
                    } else if shift {
                        KeyPress::ShiftDown
                    } else {
                        KeyPress::Down
                    });
                }
                winuser::VK_DELETE => return Ok(KeyPress::Delete),
                winuser::VK_HOME => return Ok(KeyPress::Home),
                winuser::VK_END => return Ok(KeyPress::End),
                winuser::VK_PRIOR => return Ok(KeyPress::PageUp),
                winuser::VK_NEXT => return Ok(KeyPress::PageDown),
                winuser::VK_INSERT => return Ok(KeyPress::Insert),
                winuser::VK_F1 => return Ok(KeyPress::F(1)),
                winuser::VK_F2 => return Ok(KeyPress::F(2)),
                winuser::VK_F3 => return Ok(KeyPress::F(3)),
                winuser::VK_F4 => return Ok(KeyPress::F(4)),
                winuser::VK_F5 => return Ok(KeyPress::F(5)),
                winuser::VK_F6 => return Ok(KeyPress::F(6)),
                winuser::VK_F7 => return Ok(KeyPress::F(7)),
                winuser::VK_F8 => return Ok(KeyPress::F(8)),
                winuser::VK_F9 => return Ok(KeyPress::F(9)),
                winuser::VK_F10 => return Ok(KeyPress::F(10)),
                winuser::VK_F11 => return Ok(KeyPress::F(11)),
                winuser::VK_F12 => return Ok(KeyPress::F(12)),
                // winuser::VK_BACK is correctly handled because the key_event.UnicodeChar is
                // also set.
                _ => continue,
            };
        } else if utf16 == 27 {
            return Ok(KeyPress::Esc);
        } else {
            if utf16 >= 0xD800 && utf16 < 0xDC00 {
                surrogate = utf16;
                continue;
            }
            let orc = if surrogate == 0 {
                decode_utf16(Some(utf16)).next()
            } else {
                decode_utf16([surrogate, utf16].iter().cloned()).next()
            };
            let rc = if let Some(rc) = orc {
                rc
            } else {
                return Err(error::ReadlineError::Eof);
            };
            let c = rc?;
            if meta {
                return Ok(KeyPress::Meta(c));
            } else {
                let mut key = keys::char_to_key_press(c);
                if key == KeyPress::Tab && shift {
                    key = KeyPress::BackTab;
                } else if key == KeyPress::Char(' ') && ctrl {
                    key = KeyPress::Ctrl(' ');
                }
                return Ok(key);
            }
        }
    }
}

pub struct ConsoleRenderer {
    out: OutputStreamType,
    handle: HANDLE,
    cols: usize, // Number of columns in terminal
    buffer: String,
    colors_enabled: bool,
    bell_style: BellStyle,
}

impl ConsoleRenderer {
    fn new(
        handle: HANDLE,
        out: OutputStreamType,
        colors_enabled: bool,
        bell_style: BellStyle,
    ) -> ConsoleRenderer {
        // Multi line editing is enabled by ENABLE_WRAP_AT_EOL_OUTPUT mode
        let (cols, _) = get_win_size(handle);
        ConsoleRenderer {
            out,
            handle,
            cols,
            buffer: String::with_capacity(1024),
            colors_enabled,
            bell_style,
        }
    }

    fn get_console_screen_buffer_info(&self) -> Result<CONSOLE_SCREEN_BUFFER_INFO> {
        let mut info = unsafe { mem::zeroed() };
        check!(wincon::GetConsoleScreenBufferInfo(self.handle, &mut info));
        Ok(info)
    }

    fn set_console_cursor_position(&mut self, pos: wincon::COORD) -> Result<()> {
        check!(wincon::SetConsoleCursorPosition(self.handle, pos));
        Ok(())
    }

    fn clear(&mut self, length: DWORD, pos: wincon::COORD) -> Result<()> {
        let mut _count = 0;
        check!(wincon::FillConsoleOutputCharacterA(
            self.handle,
            ' ' as CHAR,
            length,
            pos,
            &mut _count,
        ));
        Ok(())
    }

    fn clear_old_rows(
        &mut self,
        info: &mut CONSOLE_SCREEN_BUFFER_INFO,
        layout: &Layout,
    ) -> Result<()> {
        let current_row = layout.cursor.row;
        let old_rows = layout.end.row;
        // position at the start of the prompt, clear to end of previous input
        info.dwCursorPosition.X = 0;
        info.dwCursorPosition.Y -= current_row as i16;
        self.set_console_cursor_position(info.dwCursorPosition)?;
        self.clear(
            (info.dwSize.X * (old_rows as i16 + 1)) as DWORD,
            info.dwCursorPosition,
        )
    }
}

impl Renderer for ConsoleRenderer {
    type Reader = ConsoleRawReader;

    fn move_cursor(&mut self, old: Position, new: Position) -> Result<()> {
        let mut cursor = self.get_console_screen_buffer_info()?.dwCursorPosition;
        if new.row > old.row {
            cursor.Y += (new.row - old.row) as i16;
        } else {
            cursor.Y -= (old.row - new.row) as i16;
        }
        if new.col > old.col {
            cursor.X += (new.col - old.col) as i16;
        } else {
            cursor.X -= (old.col - new.col) as i16;
        }
        self.set_console_cursor_position(cursor)
    }

    fn refresh_line(
        &mut self,
        prompt: &str,
        line: &LineBuffer,
        hint: Option<&str>,
        old_layout: &Layout,
        new_layout: &Layout,
        highlighter: Option<&dyn Highlighter>,
    ) -> Result<()> {
        let default_prompt = new_layout.default_prompt;
        let cursor = new_layout.cursor;
        let end_pos = new_layout.end;

        self.buffer.clear();
        if let Some(highlighter) = highlighter {
            // TODO handle ansi escape code (SetConsoleTextAttribute)
            // append the prompt
            self.buffer
                .push_str(&highlighter.highlight_prompt(prompt, default_prompt));
            // append the input line
            self.buffer
                .push_str(&highlighter.highlight(line, line.pos()));
        } else {
            // append the prompt
            self.buffer.push_str(prompt);
            // append the input line
            self.buffer.push_str(line);
        }
        // append hint
        if let Some(hint) = hint {
            if let Some(highlighter) = highlighter {
                self.buffer.push_str(&highlighter.highlight_hint(hint));
            } else {
                self.buffer.push_str(hint);
            }
        }
        let mut info = self.get_console_screen_buffer_info()?;
        // position at the start of the prompt, clear to end of previous input
        self.clear_old_rows(&mut info, old_layout)?;
        // display prompt, input line and hint
        self.write_and_flush(self.buffer.as_bytes())?;

        // position the cursor
        let mut coord = self.get_console_screen_buffer_info()?.dwCursorPosition;
        coord.X = cursor.col as i16;
        coord.Y -= (end_pos.row - cursor.row) as i16;
        self.set_console_cursor_position(coord)?;

        Ok(())
    }

    fn write_and_flush(&self, buf: &[u8]) -> Result<()> {
        match self.out {
            OutputStreamType::Stdout => {
                io::stdout().write_all(buf)?;
                io::stdout().flush()?;
            }
            OutputStreamType::Stderr => {
                io::stderr().write_all(buf)?;
                io::stderr().flush()?;
            }
        }
        Ok(())
    }

    /// Characters with 2 column width are correctly handled (not split).
    fn calculate_position(&self, s: &str, orig: Position) -> Position {
        let mut pos = orig;
        for c in s.chars() {
            let cw = if c == '\n' {
                pos.col = 0;
                pos.row += 1;
                None
            } else {
                c.width()
            };
            if let Some(cw) = cw {
                pos.col += cw;
                if pos.col > self.cols {
                    pos.row += 1;
                    pos.col = cw;
                }
            }
        }
        if pos.col == self.cols {
            pos.col = 0;
            pos.row += 1;
        }
        pos
    }

    fn beep(&mut self) -> Result<()> {
        match self.bell_style {
            BellStyle::Audible => {
                io::stderr().write_all(b"\x07")?;
                io::stderr().flush()?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Clear the screen. Used to handle ctrl+l
    fn clear_screen(&mut self) -> Result<()> {
        let info = self.get_console_screen_buffer_info()?;
        let coord = wincon::COORD { X: 0, Y: 0 };
        check!(wincon::SetConsoleCursorPosition(self.handle, coord));
        let n = info.dwSize.X as DWORD * info.dwSize.Y as DWORD;
        self.clear(n, coord)
    }

    fn clear_rows(&mut self, layout: &Layout) -> Result<()> {
        let mut info = self.get_console_screen_buffer_info()?;
        self.clear_old_rows(&mut info, layout)
    }

    fn sigwinch(&self) -> bool {
        SIGWINCH.compare_and_swap(true, false, Ordering::SeqCst)
    }

    /// Try to get the number of columns in the current terminal,
    /// or assume 80 if it fails.
    fn update_size(&mut self) {
        let (cols, _) = get_win_size(self.handle);
        self.cols = cols;
    }

    fn get_columns(&self) -> usize {
        self.cols
    }

    /// Try to get the number of rows in the current terminal,
    /// or assume 24 if it fails.
    fn get_rows(&self) -> usize {
        let (_, rows) = get_win_size(self.handle);
        rows
    }

    fn colors_enabled(&self) -> bool {
        self.colors_enabled
    }

    fn move_cursor_at_leftmost(&mut self, _: &mut ConsoleRawReader) -> Result<()> {
        self.write_and_flush(b"")?; // we must do this otherwise the cursor position is not reported correctly
        let mut info = self.get_console_screen_buffer_info()?;
        if info.dwCursorPosition.X == 0 {
            return Ok(());
        }
        debug!(target: "rustyline", "initial cursor location: {:?}, {:?}", info.dwCursorPosition.X, info.dwCursorPosition.Y);
        info.dwCursorPosition.X = 0;
        info.dwCursorPosition.Y += 1;
        self.set_console_cursor_position(info.dwCursorPosition)
    }
}

static SIGWINCH: AtomicBool = AtomicBool::new(false);

#[cfg(not(test))]
pub type Terminal = Console;

#[derive(Clone, Debug)]
pub struct Console {
    stdin_isatty: bool,
    stdin_handle: HANDLE,
    stdstream_isatty: bool,
    stdstream_handle: HANDLE,
    pub(crate) color_mode: ColorMode,
    ansi_colors_supported: bool,
    stream_type: OutputStreamType,
    bell_style: BellStyle,
    raw_mode: Arc<AtomicBool>,
    // external print reader
    pipe_reader: Option<Arc<AsyncPipe>>,
    // external print writer
    pipe_writer: Option<SyncSender<String>>,
}

impl Console {
    fn colors_enabled(&self) -> bool {
        // TODO ANSI Colors & Windows <10
        match self.color_mode {
            ColorMode::Enabled => self.stdstream_isatty && self.ansi_colors_supported,
            ColorMode::Forced => true,
            ColorMode::Disabled => false,
        }
    }
}

impl Term for Console {
    type ExternalPrinter = ExternalPrinter;
    type Mode = ConsoleMode;
    type Reader = ConsoleRawReader;
    type Writer = ConsoleRenderer;

    fn new(
        color_mode: ColorMode,
        stream_type: OutputStreamType,
        _tab_stop: usize,
        bell_style: BellStyle,
    ) -> Console {
        let stdin_handle = get_std_handle(STDIN_FILENO);
        let stdin_isatty = match stdin_handle {
            Ok(handle) => {
                // If this function doesn't fail then fd is a TTY
                get_console_mode(handle).is_ok()
            }
            Err(_) => false,
        };

        let stdstream_handle = get_std_handle(if stream_type == OutputStreamType::Stdout {
            STDOUT_FILENO
        } else {
            STDERR_FILENO
        });
        let stdstream_isatty = match stdstream_handle {
            Ok(handle) => {
                // If this function doesn't fail then fd is a TTY
                get_console_mode(handle).is_ok()
            }
            Err(_) => false,
        };

        Console {
            stdin_isatty,
            stdin_handle: stdin_handle.unwrap_or(ptr::null_mut()),
            stdstream_isatty,
            stdstream_handle: stdstream_handle.unwrap_or(ptr::null_mut()),
            color_mode,
            ansi_colors_supported: false,
            stream_type,
            bell_style,
            raw_mode: Arc::new(AtomicBool::new(false)),
            pipe_reader: None,
            pipe_writer: None,
        }
    }

    /// Checking for an unsupported TERM in windows is a no-op
    fn is_unsupported(&self) -> bool {
        false
    }

    fn is_stdin_tty(&self) -> bool {
        self.stdin_isatty
    }

    fn is_output_tty(&self) -> bool {
        self.stdstream_isatty
    }

    // pub fn install_sigwinch_handler(&mut self) {
    // See ReadConsoleInputW && WINDOW_BUFFER_SIZE_EVENT
    // }

    /// Enable RAW mode for the terminal.
    fn enable_raw_mode(&mut self) -> Result<Self::Mode> {
        if !self.stdin_isatty {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "no stdio handle available for this process",
            ))?;
        }
        let original_stdin_mode = get_console_mode(self.stdin_handle)?;
        // Disable these modes
        let mut raw = original_stdin_mode
            & !(wincon::ENABLE_LINE_INPUT
                | wincon::ENABLE_ECHO_INPUT
                | wincon::ENABLE_PROCESSED_INPUT);
        // Enable these modes
        raw |= wincon::ENABLE_EXTENDED_FLAGS;
        raw |= wincon::ENABLE_INSERT_MODE;
        raw |= wincon::ENABLE_QUICK_EDIT_MODE;
        raw |= wincon::ENABLE_WINDOW_INPUT;
        check!(consoleapi::SetConsoleMode(self.stdin_handle, raw));

        let original_stdstream_mode = if self.stdstream_isatty {
            let original_stdstream_mode = get_console_mode(self.stdstream_handle)?;
            // To enable ANSI colors (Windows 10 only):
            // https://docs.microsoft.com/en-us/windows/console/setconsolemode
            if original_stdstream_mode & wincon::ENABLE_VIRTUAL_TERMINAL_PROCESSING == 0 {
                let raw = original_stdstream_mode | wincon::ENABLE_VIRTUAL_TERMINAL_PROCESSING;
                self.ansi_colors_supported =
                    unsafe { consoleapi::SetConsoleMode(self.stdstream_handle, raw) != 0 };
                debug!(target: "rustyline", "ansi_colors_supported: {}", self.ansi_colors_supported);
            } else {
                debug!(target: "rustyline", "ANSI colors already enabled");
                self.ansi_colors_supported = true;
            }
            Some(original_stdstream_mode)
        } else {
            None
        };

        self.raw_mode.store(true, Ordering::SeqCst);
        // when all ExternalPrinter are dropped there is no need to use `pipe_reader`
        /*if let Some(ref arc) = self.pipe_writer { FIXME
            if Arc::strong_count(arc) == 1 {
                self.pipe_writer = None;
                self.pipe_reader = None;
            }
        }*/

        Ok(ConsoleMode {
            original_stdin_mode,
            stdin_handle: self.stdin_handle,
            original_stdstream_mode,
            stdstream_handle: self.stdstream_handle,
            raw_mode: self.raw_mode.clone(),
        })
    }

    fn create_reader(&self, _: &Config) -> Result<ConsoleRawReader> {
        ConsoleRawReader::create(self.pipe_reader.clone())
    }

    fn create_writer(&self) -> ConsoleRenderer {
        ConsoleRenderer::new(
            self.stdstream_handle,
            self.stream_type,
            self.colors_enabled(),
            self.bell_style,
        )
    }

    fn create_external_printer(&mut self) -> Result<ExternalPrinter> {
        if let Some(ref sender) = self.pipe_writer {
            return Ok(ExternalPrinter {
                event: INVALID_HANDLE_VALUE, // FIXME
                buf: String::new(),
                sender: sender.clone(),
                raw_mode: self.raw_mode.clone(),
                target: self.stream_type,
            });
        }
        if !self.is_stdin_tty() || !self.is_output_tty() {
            Err(io::Error::from(ErrorKind::Other))?; // FIXME
        }
        let event = unsafe { CreateEventW(ptr::null_mut(), TRUE, FALSE, ptr::null()) };
        if event.is_null() {
            Err(io::Error::last_os_error())?;
        }
        let (sender, receiver) = sync_channel(1);

        let reader = Arc::new(AsyncPipe {
            event: Handle(event),
            receiver,
        });
        self.pipe_reader.replace(reader.clone());
        self.pipe_writer.replace(sender.clone());
        Ok(ExternalPrinter {
            event: event,
            buf: String::new(),
            sender,
            raw_mode: self.raw_mode.clone(),
            target: self.stream_type,
        })
    }
}

unsafe impl Send for Console {}
unsafe impl Sync for Console {}

#[derive(Debug)]
struct AsyncPipe {
    event: Handle,
    receiver: Receiver<String>,
}

#[derive(Debug)]
pub struct ExternalPrinter {
    event: HANDLE,
    buf: String,
    sender: SyncSender<String>,
    raw_mode: Arc<AtomicBool>,
    target: OutputStreamType,
}

unsafe impl Send for ExternalPrinter {}
unsafe impl Sync for ExternalPrinter {}

impl Write for ExternalPrinter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // write directly to stdout/stderr while not in raw mode
        if self.raw_mode.load(Ordering::SeqCst) {
            match self.target {
                OutputStreamType::Stderr => io::stderr().write(buf),
                OutputStreamType::Stdout => io::stdout().write(buf),
            }
        } else {
            match std::str::from_utf8(buf) {
                Ok(s) => {
                    self.buf.push_str(s);
                    if s.contains('\n') {
                        self.flush()?
                    }
                }
                Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidInput, e)),
            };
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.raw_mode.load(Ordering::SeqCst) {
            match self.target {
                OutputStreamType::Stderr => io::stderr().flush(),
                OutputStreamType::Stdout => io::stdout().flush(),
            }
        } else {
            if let Err(err) = self.sender.send(self.buf.split_off(0)) {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, err));
            }
            check!(SetEvent(self.event));
            Ok(())
        }
    }
}

#[derive(Debug)]
struct Handle(HANDLE);

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

#[cfg(test)]
mod test {
    use super::Console;

    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Console>();
    }

    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Console>();
    }
}
