//! Readline for Rust
//!
//! This implementation is based on [Antirez's
//! Linenoise](https://github.com/antirez/linenoise)
//!
//! # Example
//!
//! Usage
//!
//! ```
//! let mut rl = rustyline::Editor::<()>::new();
//! let readline = rl.readline(">> ");
//! match readline {
//!     Ok(line) => println!("Line: {:?}", line),
//!     Err(_) => println!("No input"),
//! }
//! ```
#![allow(unknown_lints)]
// #![feature(non_exhaustive)]
// #![feature(tool_lints)]

extern crate dirs;
extern crate libc;
#[macro_use]
extern crate log;
extern crate memchr;
#[cfg(unix)]
extern crate nix;
extern crate unicode_segmentation;
extern crate unicode_width;
#[cfg(unix)]
extern crate utf8parse;
#[cfg(windows)]
extern crate winapi;

pub mod completion;
pub mod config;
mod edit;
pub mod error;
pub mod highlight;
pub mod hint;
pub mod history;
mod keymap;
mod keys;
mod kill_ring;
pub mod line_buffer;
mod undo;

mod tty;

use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};
use std::path::Path;
use std::result;
use std::sync::{Arc, Mutex, RwLock};
use unicode_width::UnicodeWidthStr;

use tty::{RawMode, RawReader, Renderer, Term, Terminal};

use completion::{longest_common_prefix, Candidate, Completer};
pub use config::{ColorMode, CompletionType, Config, EditMode, HistoryDuplicates};
use edit::State;
use highlight::Highlighter;
use hint::Hinter;
use history::{Direction, History};
pub use keymap::{Anchor, At, CharSearch, Cmd, Movement, RepeatCount, Word};
use keymap::{InputState, Refresher};
pub use keys::KeyPress;
use kill_ring::{KillRing, Mode};
use line_buffer::WordAction;

/// The error type for I/O and Linux Syscalls (Errno)
pub type Result<T> = result::Result<T, error::ReadlineError>;

/// Completes the line/word
fn complete_line<R: RawReader, C: Completer>(
    rdr: &mut R,
    s: &mut State,
    input_state: &mut InputState,
    completer: &C,
    highlighter: Option<&Highlighter>,
    config: &Config,
) -> Result<Option<Cmd>> {
    // get a list of completions
    let (start, candidates) = try!(completer.complete(&s.line, s.line.pos()));
    // if no completions, we are done
    if candidates.is_empty() {
        try!(s.out.beep());
        Ok(None)
    } else if CompletionType::Circular == config.completion_type() {
        let mark = s.changes.borrow_mut().begin();
        // Save the current edited line before overwriting it
        let backup = s.line.as_str().to_owned();
        let backup_pos = s.line.pos();
        let mut cmd;
        let mut i = 0;
        loop {
            // Show completion or original buffer
            if i < candidates.len() {
                let candidate = candidates[i].replacement();
                // TODO we can't highlight the line buffer directly
                /*let candidate = if let Some(highlighter) = s.highlighter {
                    highlighter.highlight_candidate(candidate, CompletionType::Circular)
                } else {
                    Borrowed(candidate)
                };*/
                completer.update(&mut s.line, start, candidate);
                try!(s.refresh_line());
            } else {
                // Restore current edited line
                s.line.update(&backup, backup_pos);
                try!(s.refresh_line());
            }

            cmd = try!(s.next_cmd(input_state, rdr, true));
            match cmd {
                Cmd::Complete => {
                    i = (i + 1) % (candidates.len() + 1); // Circular
                    if i == candidates.len() {
                        try!(s.out.beep());
                    }
                }
                Cmd::Abort => {
                    // Re-show original buffer
                    if i < candidates.len() {
                        s.line.update(&backup, backup_pos);
                        try!(s.refresh_line());
                    }
                    s.changes.borrow_mut().truncate(mark);
                    return Ok(None);
                }
                _ => {
                    s.changes.borrow_mut().end();
                    break;
                }
            }
        }
        Ok(Some(cmd))
    } else if CompletionType::List == config.completion_type() {
        if let Some(lcp) = longest_common_prefix(&candidates) {
            // if we can extend the item, extend it
            if lcp.len() > s.line.pos() - start {
                completer.update(&mut s.line, start, lcp);
                try!(s.refresh_line());
            }
        }
        // beep if ambiguous
        if candidates.len() > 1 {
            try!(s.out.beep());
        } else {
            return Ok(None);
        }
        // we can't complete any further, wait for second tab
        let mut cmd = try!(s.next_cmd(input_state, rdr, true));
        // if any character other than tab, pass it to the main loop
        if cmd != Cmd::Complete {
            return Ok(Some(cmd));
        }
        // move cursor to EOL to avoid overwriting the command line
        let save_pos = s.line.pos();
        try!(s.edit_move_end());
        s.line.set_pos(save_pos);
        // we got a second tab, maybe show list of possible completions
        let show_completions = if candidates.len() > config.completion_prompt_limit() {
            let msg = format!("\nDisplay all {} possibilities? (y or n)", candidates.len());
            try!(s.out.write_and_flush(msg.as_bytes()));
            s.old_rows += 1;
            while cmd != Cmd::SelfInsert(1, 'y')
                && cmd != Cmd::SelfInsert(1, 'Y')
                && cmd != Cmd::SelfInsert(1, 'n')
                && cmd != Cmd::SelfInsert(1, 'N')
                && cmd != Cmd::Kill(Movement::BackwardChar(1))
            {
                cmd = try!(s.next_cmd(input_state, rdr, false));
            }
            match cmd {
                Cmd::SelfInsert(1, 'y') | Cmd::SelfInsert(1, 'Y') => true,
                _ => false,
            }
        } else {
            true
        };
        if show_completions {
            page_completions(rdr, s, input_state, highlighter, &candidates)
        } else {
            try!(s.refresh_line());
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn page_completions<R: RawReader, C: Candidate>(
    rdr: &mut R,
    s: &mut State,
    input_state: &mut InputState,
    highlighter: Option<&Highlighter>,
    candidates: &[C],
) -> Result<Option<Cmd>> {
    use std::cmp;

    let min_col_pad = 2;
    let cols = s.out.get_columns();
    let max_width = cmp::min(
        cols,
        candidates
            .into_iter()
            .map(|s| s.display().width())
            .max()
            .unwrap()
            + min_col_pad,
    );
    let num_cols = cols / max_width;

    let mut pause_row = s.out.get_rows() - 1;
    let num_rows = (candidates.len() + num_cols - 1) / num_cols;
    let mut ab = String::new();
    for row in 0..num_rows {
        if row == pause_row {
            try!(s.out.write_and_flush(b"\n--More--"));
            let mut cmd = Cmd::Noop;
            while cmd != Cmd::SelfInsert(1, 'y')
                && cmd != Cmd::SelfInsert(1, 'Y')
                && cmd != Cmd::SelfInsert(1, 'n')
                && cmd != Cmd::SelfInsert(1, 'N')
                && cmd != Cmd::SelfInsert(1, 'q')
                && cmd != Cmd::SelfInsert(1, 'Q')
                && cmd != Cmd::SelfInsert(1, ' ')
                && cmd != Cmd::Kill(Movement::BackwardChar(1))
                && cmd != Cmd::AcceptLine
            {
                cmd = try!(s.next_cmd(input_state, rdr, false));
            }
            match cmd {
                Cmd::SelfInsert(1, 'y') | Cmd::SelfInsert(1, 'Y') | Cmd::SelfInsert(1, ' ') => {
                    pause_row += s.out.get_rows() - 1;
                }
                Cmd::AcceptLine => {
                    pause_row += 1;
                }
                _ => break,
            }
            try!(s.out.write_and_flush(b"\n"));
        } else {
            try!(s.out.write_and_flush(b"\n"));
        }
        ab.clear();
        for col in 0..num_cols {
            let i = (col * num_rows) + row;
            if i < candidates.len() {
                let candidate = &candidates[i].display();
                let width = candidate.width();
                if let Some(highlighter) = highlighter {
                    ab.push_str(&highlighter.highlight_candidate(candidate, CompletionType::List));
                } else {
                    ab.push_str(candidate);
                }
                if ((col + 1) * num_rows) + row < candidates.len() {
                    for _ in width..max_width {
                        ab.push(' ');
                    }
                }
            }
        }
        try!(s.out.write_and_flush(ab.as_bytes()));
    }
    try!(s.out.write_and_flush(b"\n"));
    try!(s.refresh_line());
    Ok(None)
}

/// Incremental search
fn reverse_incremental_search<R: RawReader>(
    rdr: &mut R,
    s: &mut State,
    input_state: &mut InputState,
    history: &History,
) -> Result<Option<Cmd>> {
    if history.is_empty() {
        return Ok(None);
    }
    let mark = s.changes.borrow_mut().begin();
    // Save the current edited line (and cursor position) before overwriting it
    let backup = s.line.as_str().to_owned();
    let backup_pos = s.line.pos();

    let mut search_buf = String::new();
    let mut history_idx = history.len() - 1;
    let mut direction = Direction::Reverse;
    let mut success = true;

    let mut cmd;
    // Display the reverse-i-search prompt and process chars
    loop {
        let prompt = if success {
            format!("(reverse-i-search)`{}': ", search_buf)
        } else {
            format!("(failed reverse-i-search)`{}': ", search_buf)
        };
        try!(s.refresh_prompt_and_line(&prompt));

        cmd = try!(s.next_cmd(input_state, rdr, true));
        if let Cmd::SelfInsert(_, c) = cmd {
            search_buf.push(c);
        } else {
            match cmd {
                Cmd::Kill(Movement::BackwardChar(_)) => {
                    search_buf.pop();
                    continue;
                }
                Cmd::ReverseSearchHistory => {
                    direction = Direction::Reverse;
                    if history_idx > 0 {
                        history_idx -= 1;
                    } else {
                        success = false;
                        continue;
                    }
                }
                Cmd::ForwardSearchHistory => {
                    direction = Direction::Forward;
                    if history_idx < history.len() - 1 {
                        history_idx += 1;
                    } else {
                        success = false;
                        continue;
                    }
                }
                Cmd::Abort => {
                    // Restore current edited line (before search)
                    s.line.update(&backup, backup_pos);
                    try!(s.refresh_line());
                    s.changes.borrow_mut().truncate(mark);
                    return Ok(None);
                }
                Cmd::Move(_) => {
                    try!(s.refresh_line()); // restore prompt
                    break;
                }
                _ => break,
            }
        }
        success = match history.search(&search_buf, history_idx, direction) {
            Some(idx) => {
                history_idx = idx;
                let entry = history.get(idx).unwrap();
                let pos = entry.find(&search_buf).unwrap();
                s.line.update(entry, pos);
                true
            }
            _ => false,
        };
    }
    s.changes.borrow_mut().end();
    Ok(Some(cmd))
}

/// Handles reading and editting the readline buffer.
/// It will also handle special inputs in an appropriate fashion
/// (e.g., C-c will exit readline)
fn readline_edit<H: Helper>(
    prompt: &str,
    initial: Option<(&str, &str)>,
    editor: &mut Editor<H>,
    original_mode: &tty::Mode,
) -> Result<String> {
    let completer = editor.helper.as_ref();
    let hinter = editor.helper.as_ref().map(|h| h as &Hinter);
    let highlighter = if editor.term.colors_enabled() {
        editor.helper.as_ref().map(|h| h as &Highlighter)
    } else {
        None
    };

    let mut stdout = editor.term.create_writer();

    editor.reset_kill_ring(); // TODO recreate a new kill ring vs Arc<Mutex<KillRing>>
    let mut s = State::new(
        &mut stdout,
        prompt,
        editor.history.len(),
        hinter,
        highlighter,
    );
    let mut input_state = InputState::new(&editor.config, Arc::clone(&editor.custom_bindings));

    s.line.set_delete_listener(editor.kill_ring.clone());
    s.line.set_change_listener(s.changes.clone());

    if let Some((left, right)) = initial {
        s.line
            .update((left.to_owned() + right).as_ref(), left.len());
    }

    try!(s.refresh_line());

    let mut rdr = try!(editor.term.create_reader(&editor.config));

    loop {
        let rc = s.next_cmd(&mut input_state, &mut rdr, false);
        let mut cmd = try!(rc);

        if cmd.should_reset_kill_ring() {
            editor.reset_kill_ring();
        }

        // autocomplete
        if cmd == Cmd::Complete && completer.is_some() {
            let next = try!(complete_line(
                &mut rdr,
                &mut s,
                &mut input_state,
                completer.unwrap(),
                highlighter,
                &editor.config,
            ));
            if next.is_some() {
                cmd = next.unwrap();
            } else {
                continue;
            }
        }

        if let Cmd::SelfInsert(n, c) = cmd {
            try!(s.edit_insert(c, n));
            continue;
        } else if let Cmd::Insert(n, text) = cmd {
            try!(s.edit_yank(&input_state, &text, Anchor::Before, n));
            continue;
        }

        if cmd == Cmd::ReverseSearchHistory {
            // Search history backward
            let next = try!(reverse_incremental_search(
                &mut rdr,
                &mut s,
                &mut input_state,
                &editor.history,
            ));
            if next.is_some() {
                cmd = next.unwrap();
            } else {
                continue;
            }
        }

        match cmd {
            Cmd::Move(Movement::BeginningOfLine) => {
                // Move to the beginning of line.
                try!(s.edit_move_home())
            }
            Cmd::Move(Movement::ViFirstPrint) => {
                try!(s.edit_move_home());
                try!(s.edit_move_to_next_word(At::Start, Word::Big, 1))
            }
            Cmd::Move(Movement::BackwardChar(n)) => {
                // Move back a character.
                try!(s.edit_move_backward(n))
            }
            Cmd::ReplaceChar(n, c) => try!(s.edit_replace_char(c, n)),
            Cmd::Replace(mvt, text) => {
                try!(s.edit_kill(&mvt));
                if let Some(text) = text {
                    try!(s.edit_insert_text(&text))
                }
            }
            Cmd::Overwrite(c) => {
                try!(s.edit_overwrite_char(c));
            }
            Cmd::EndOfFile => if !input_state.is_emacs_mode() && !s.line.is_empty() {
                try!(s.edit_move_end());
                break;
            } else if s.line.is_empty() {
                return Err(error::ReadlineError::Eof);
            } else {
                try!(s.edit_delete(1))
            },
            Cmd::Move(Movement::EndOfLine) => {
                // Move to the end of line.
                try!(s.edit_move_end())
            }
            Cmd::Move(Movement::ForwardChar(n)) => {
                // Move forward a character.
                try!(s.edit_move_forward(n))
            }
            Cmd::ClearScreen => {
                // Clear the screen leaving the current line at the top of the screen.
                try!(s.out.clear_screen());
                try!(s.refresh_line())
            }
            Cmd::NextHistory => {
                // Fetch the next command from the history list.
                try!(s.edit_history_next(&editor.history, false))
            }
            Cmd::PreviousHistory => {
                // Fetch the previous command from the history list.
                try!(s.edit_history_next(&editor.history, true))
            }
            Cmd::HistorySearchBackward => {
                try!(s.edit_history_search(&editor.history, Direction::Reverse))
            }
            Cmd::HistorySearchForward => {
                try!(s.edit_history_search(&editor.history, Direction::Forward))
            }
            Cmd::TransposeChars => {
                // Exchange the char before cursor with the character at cursor.
                try!(s.edit_transpose_chars())
            }
            #[cfg(unix)]
            Cmd::QuotedInsert => {
                // Quoted insert
                let c = try!(rdr.next_char());
                try!(s.edit_insert(c, 1)) // FIXME
            }
            Cmd::Yank(n, anchor) => {
                // retrieve (yank) last item killed
                let mut kill_ring = editor.kill_ring.lock().unwrap();
                if let Some(text) = kill_ring.yank() {
                    try!(s.edit_yank(&input_state, text, anchor, n))
                }
            }
            Cmd::ViYankTo(ref mvt) => if let Some(text) = s.line.copy(mvt) {
                let mut kill_ring = editor.kill_ring.lock().unwrap();
                kill_ring.kill(&text, Mode::Append)
            },
            // TODO CTRL-_ // undo
            Cmd::AcceptLine => {
                #[cfg(test)]
                {
                    editor.term.cursor = s.cursor.col;
                }
                // Accept the line regardless of where the cursor is.
                try!(s.edit_move_end());
                if s.hinter.is_some() {
                    // Force a refresh without hints to leave the previous
                    // line as the user typed it after a newline.
                    s.hinter = None;
                    try!(s.refresh_line());
                }
                break;
            }
            Cmd::BeginningOfHistory => {
                // move to first entry in history
                try!(s.edit_history(&editor.history, true))
            }
            Cmd::EndOfHistory => {
                // move to last entry in history
                try!(s.edit_history(&editor.history, false))
            }
            Cmd::Move(Movement::BackwardWord(n, word_def)) => {
                // move backwards one word
                try!(s.edit_move_to_prev_word(word_def, n))
            }
            Cmd::CapitalizeWord => {
                // capitalize word after point
                try!(s.edit_word(WordAction::CAPITALIZE))
            }
            Cmd::Kill(ref mvt) => {
                try!(s.edit_kill(mvt));
            }
            Cmd::Move(Movement::ForwardWord(n, at, word_def)) => {
                // move forwards one word
                try!(s.edit_move_to_next_word(at, word_def, n))
            }
            Cmd::DowncaseWord => {
                // lowercase word after point
                try!(s.edit_word(WordAction::LOWERCASE))
            }
            Cmd::TransposeWords(n) => {
                // transpose words
                try!(s.edit_transpose_words(n))
            }
            Cmd::UpcaseWord => {
                // uppercase word after point
                try!(s.edit_word(WordAction::UPPERCASE))
            }
            Cmd::YankPop => {
                // yank-pop
                let mut kill_ring = editor.kill_ring.lock().unwrap();
                if let Some((yank_size, text)) = kill_ring.yank_pop() {
                    try!(s.edit_yank_pop(yank_size, text))
                }
            }
            Cmd::Move(Movement::ViCharSearch(n, cs)) => try!(s.edit_move_to(cs, n)),
            Cmd::Undo(n) => {
                s.line.remove_change_listener();
                if s.changes.borrow_mut().undo(&mut s.line, n) {
                    try!(s.refresh_line());
                }
                s.line.set_change_listener(s.changes.clone());
            }
            Cmd::Interrupt => {
                return Err(error::ReadlineError::Interrupted);
            }
            #[cfg(unix)]
            Cmd::Suspend => {
                try!(original_mode.disable_raw_mode());
                try!(tty::suspend());
                try!(editor.term.enable_raw_mode()); // TODO original_mode may have changed
                try!(s.refresh_line());
                continue;
            }
            Cmd::Noop => {}
            _ => {
                // Ignore the character typed.
            }
        }
    }
    if cfg!(windows) {
        let _ = original_mode; // silent warning
    }
    Ok(s.line.into_string())
}

struct Guard<'m>(&'m tty::Mode);

#[allow(unused_must_use)]
impl<'m> Drop for Guard<'m> {
    fn drop(&mut self) {
        let Guard(mode) = *self;
        mode.disable_raw_mode();
    }
}

/// Readline method that will enable RAW mode, call the `readline_edit()`
/// method and disable raw mode
fn readline_raw<H: Helper>(
    prompt: &str,
    initial: Option<(&str, &str)>,
    editor: &mut Editor<H>,
) -> Result<String> {
    let original_mode = try!(editor.term.enable_raw_mode());
    let guard = Guard(&original_mode);
    let user_input = readline_edit(prompt, initial, editor, &original_mode);
    if editor.config.auto_add_history() {
        if let Ok(ref line) = user_input {
            editor.add_history_entry(line.as_ref());
        }
    }
    drop(guard); // try!(disable_raw_mode(original_mode));
    println!();
    user_input
}

fn readline_direct() -> Result<String> {
    let mut line = String::new();
    if try!(io::stdin().read_line(&mut line)) > 0 {
        Ok(line)
    } else {
        Err(error::ReadlineError::Eof)
    }
}

/// Syntax specific helper.
///
/// TODO Tokenizer/parser used for both completion, suggestion, highlighting.
/// (parse current line once)
pub trait Helper
where
    Self: Completer,
    Self: Hinter,
    Self: Highlighter,
{
}

impl Helper for () {}

/// Line editor
pub struct Editor<H: Helper> {
    term: Terminal,
    history: History,
    helper: Option<H>,
    kill_ring: Arc<Mutex<KillRing>>,
    config: Config,
    custom_bindings: Arc<RwLock<HashMap<KeyPress, Cmd>>>,
}

//#[allow(clippy::new_without_default)]
impl<H: Helper> Editor<H> {
    /// Create an editor with the default configuration
    pub fn new() -> Editor<H> {
        Self::with_config(Config::default())
    }

    /// Create an editor with a specific configuration.
    pub fn with_config(config: Config) -> Editor<H> {
        let term = Terminal::new(config.color_mode());
        Editor {
            term,
            history: History::with_config(config),
            helper: None,
            kill_ring: Arc::new(Mutex::new(KillRing::new(60))),
            config,
            custom_bindings: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// This method will read a line from STDIN and will display a `prompt`.
    ///
    /// It uses terminal-style interaction if `stdin` is connected to a
    /// terminal.
    /// Otherwise (e.g., if `stdin` is a pipe or the terminal is not supported),
    /// it uses file-style interaction.
    pub fn readline(&mut self, prompt: &str) -> Result<String> {
        self.readline_with(prompt, None)
    }

    /// This function behaves in the exact same manner as `readline`, except
    /// that it pre-populates the input area.
    ///
    /// The text that resides in the input area is given as a 2-tuple.
    /// The string on the left of the tuple is what will appear to the left of
    /// the cursor and the string on the right is what will appear to the
    /// right of the cursor.
    pub fn readline_with_initial(&mut self, prompt: &str, initial: (&str, &str)) -> Result<String> {
        self.readline_with(prompt, Some(initial))
    }

    fn readline_with(&mut self, prompt: &str, initial: Option<(&str, &str)>) -> Result<String> {
        if self.term.is_unsupported() {
            debug!(target: "rustyline", "unsupported terminal");
            // Write prompt and flush it to stdout
            let mut stdout = io::stdout();
            try!(stdout.write_all(prompt.as_bytes()));
            try!(stdout.flush());

            readline_direct()
        } else if !self.term.is_stdin_tty() {
            debug!(target: "rustyline", "stdin is not a tty");
            // Not a tty: read from file / pipe.
            readline_direct()
        } else {
            readline_raw(prompt, initial, self)
        }
    }

    /// Load the history from the specified file.
    pub fn load_history<P: AsRef<Path> + ?Sized>(&mut self, path: &P) -> Result<()> {
        self.history.load(path)
    }

    /// Save the history in the specified file.
    pub fn save_history<P: AsRef<Path> + ?Sized>(&self, path: &P) -> Result<()> {
        self.history.save(path)
    }

    /// Add a new entry in the history.
    pub fn add_history_entry<S: AsRef<str> + Into<String>>(&mut self, line: S) -> bool {
        self.history.add(line)
    }

    /// Clear history.
    pub fn clear_history(&mut self) {
        self.history.clear()
    }

    /// Return a mutable reference to the history object.
    pub fn history_mut(&mut self) -> &mut History {
        &mut self.history
    }

    /// Return an immutable reference to the history object.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Register a callback function to be called for tab-completion
    /// or to show hints to the user at the right of the prompt.
    pub fn set_helper(&mut self, helper: Option<H>) {
        self.helper = helper;
    }

    /// Return a mutable reference to the helper.
    pub fn helper_mut(&mut self) -> Option<&mut H> {
        self.helper.as_mut()
    }

    /// Return an immutable reference to the helper.
    pub fn helper(&self) -> Option<&H> {
        self.helper.as_ref()
    }

    /// Bind a sequence to a command.
    pub fn bind_sequence(&mut self, key_seq: KeyPress, cmd: Cmd) -> Option<Cmd> {
        let mut bindings = self.custom_bindings.write().unwrap();
        bindings.insert(key_seq, cmd)
    }

    /// Remove a binding for the given sequence.
    pub fn unbind_sequence(&mut self, key_seq: KeyPress) -> Option<Cmd> {
        let mut bindings = self.custom_bindings.write().unwrap();
        bindings.remove(&key_seq)
    }

    /// ```
    /// let mut rl = rustyline::Editor::<()>::new();
    /// for readline in rl.iter("> ") {
    ///     match readline {
    ///         Ok(line) => {
    ///             println!("Line: {}", line);
    ///         }
    ///         Err(err) => {
    ///             println!("Error: {:?}", err);
    ///             break;
    ///         }
    ///     }
    /// }
    /// ```
    pub fn iter<'a>(&'a mut self, prompt: &'a str) -> Iter<H> {
        Iter {
            editor: self,
            prompt,
        }
    }

    fn reset_kill_ring(&self) {
        let mut kill_ring = self.kill_ring.lock().unwrap();
        kill_ring.reset();
    }
}

impl<H: Helper> config::Configurer for Editor<H> {
    fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    fn set_max_history_size(&mut self, max_size: usize) {
        self.config_mut().set_max_history_size(max_size);
        self.history.set_max_len(max_size);
    }

    fn set_history_ignore_dups(&mut self, yes: bool) {
        self.config_mut().set_history_ignore_dups(yes);
        self.history.ignore_dups = yes;
    }

    fn set_history_ignore_space(&mut self, yes: bool) {
        self.config_mut().set_history_ignore_space(yes);
        self.history.ignore_space = yes;
    }

    fn set_color_mode(&mut self, color_mode: ColorMode) {
        self.config_mut().set_color_mode(color_mode);
        self.term.color_mode = color_mode;
    }
}

impl<H: Helper> fmt::Debug for Editor<H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Editor")
            .field("term", &self.term)
            .field("config", &self.config)
            .finish()
    }
}

/// Edited lines iterator
pub struct Iter<'a, H: Helper>
where
    H: 'a,
{
    editor: &'a mut Editor<H>,
    prompt: &'a str,
}

impl<'a, H: Helper> Iterator for Iter<'a, H> {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Result<String>> {
        let readline = self.editor.readline(self.prompt);
        match readline {
            Ok(l) => Some(Ok(l)),
            Err(error::ReadlineError::Eof) => None,
            e @ Err(_) => Some(e),
        }
    }
}

#[cfg(test)]
#[macro_use]
extern crate assert_matches;
#[cfg(test)]
mod test;
