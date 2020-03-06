use std::sync::Arc;
use std::borrow::Cow::{self, Borrowed, Owned};
use std::mem;

use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::config::OutputStreamType;
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::highlight::{PromptInfo};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::{Cmd, CompletionType, Config, Context, EditMode, Editor, KeyPress};
use rustyline_derive::{Helper, Validator};

#[derive(Helper, Validator)]
struct MyHelper {
    completer: FilenameCompleter,
    highlighter: MatchingBracketHighlighter,
    hinter: HistoryHinter,
    colored_prompt: String,
    continuation_prompt: String,
}

#[derive(Debug)]
struct ViMode;

#[derive(Debug)]
struct EmacsMode;

impl Completer for MyHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        self.completer.complete(line, pos, ctx)
    }
}

impl Hinter for MyHelper {
    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for MyHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        info: PromptInfo<'_>,
    ) -> Cow<'b, str> {
        if info.default() {
            if info.line_no() > 0 {
                Borrowed(&self.continuation_prompt)
            } else {
                Borrowed(&self.colored_prompt)
            }
        } else {
            Borrowed(prompt)
        }
    }

    fn has_continuation_prompt(&self) -> bool {
        return true;
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned("\x1b[1m".to_owned() + hint + "\x1b[m")
    }

    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize) -> bool {
        self.highlighter.highlight_char(line, pos)
    }
}

// To debug rustyline:
// RUST_LOG=rustyline=debug cargo run --example example 2> debug.log
fn main() -> rustyline::Result<()> {
    env_logger::init();
    let mut config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .edit_mode(EditMode::Emacs)
        .output_stream(OutputStreamType::Stdout);
    let mut initial = String::new();
    let mut initial_cursor = 0;
    println!("Use F2 fo vi mode, F3 for emacs mode");
    'outer: loop {
        let h = MyHelper {
            completer: FilenameCompleter::new(),
            highlighter: MatchingBracketHighlighter::new(),
            hinter: HistoryHinter {},
            colored_prompt: "  0> ".to_owned(),
            continuation_prompt: "\x1b[1;32m...> \x1b[0m".to_owned(),
        };
        let mut rl = Editor::with_config(config.clone().build());
        rl.set_helper(Some(h));
        rl.bind_sequence(KeyPress::F(2), Cmd::Yield(Arc::new(ViMode)));
        rl.bind_sequence(KeyPress::F(3), Cmd::Yield(Arc::new(EmacsMode)));
        if rl.load_history("history.txt").is_err() {
            println!("No previous history.");
        }
        let mut count = 1;
        loop {
            let p = format!("{:>3}> ", count);
            rl.helper_mut().expect("No helper").colored_prompt = format!("\x1b[1;32m{}\x1b[0m", p);
            let cur_ini = mem::replace(&mut initial, String::new());
            let cur_cursor = mem::replace(&mut initial_cursor, 0);
            let readline = rl.readline_with_initial(&p,
                (&cur_ini[..cur_cursor], &cur_ini[cur_cursor..]));
            let mut stop = false;
            match readline {
                Ok(line) => {
                    rl.add_history_entry(line.as_str());
                    println!("Line: {}", line);
                    count += 1;
                    continue;
                }
                Err(ReadlineError::Interrupted) => {
                    println!("CTRL-C");
                    stop = true;
                }
                Err(ReadlineError::Yielded { input, cursor, value }) => {
                    if value.is::<ViMode>() {
                        println!("Switching to vi mode");
                        config = config.edit_mode(EditMode::Vi);
                    }
                    if value.is::<EmacsMode>() {
                        println!("Switching to emacs mode");
                        config = config.edit_mode(EditMode::Emacs);
                    }
                    initial = input;
                    initial_cursor = cursor;
                }
                Err(ReadlineError::Eof) => {
                    println!("CTRL-D");
                    stop = true;
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    stop = true;
                }
            }
            rl.save_history("history.txt")?;
            if stop {
                break 'outer;
            } else {
                break;
            }
        }
    }
    Ok(())
}
