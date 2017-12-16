extern crate log;
extern crate rustyline;

use log::{LogLevel, LogLevelFilter, LogMetadata, LogRecord, SetLoggerError};

use rustyline::completion::FilenameCompleter;
use rustyline::hint::Hinter;
use rustyline::error::ReadlineError;
use rustyline::{Cmd, CompletionType, Config, EditMode, Editor, KeyPress};

// On unix platforms you can use ANSI escape sequences
#[cfg(unix)]
static PROMPT: &'static str = "\x1b[1;32m>>\x1b[0m ";

// Windows consoles typically don't support ANSI escape sequences out
// of the box
#[cfg(windows)]
static PROMPT: &'static str = ">> ";

struct Hints {}

impl Hinter for Hints {
    fn hint(&self, line: &str, _pos: usize) -> Option<String> {
        if line == "hello" {
            Some(" \x1b[1mWorld\x1b[m".to_owned())
        } else {
            None
        }
    }
}

fn main() {
    init_logger().is_ok();
    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .edit_mode(EditMode::Emacs)
        .build();
    let c = FilenameCompleter::new();
    let mut rl = Editor::with_config(config);
    rl.set_helper(Some((c, Hints {})));
    rl.bind_sequence(KeyPress::Meta('N'), Cmd::HistorySearchForward);
    rl.bind_sequence(KeyPress::Meta('P'), Cmd::HistorySearchBackward);
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }
    loop {
        let readline = rl.readline(PROMPT);
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_ref());
                println!("Line: {}", line);
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    rl.save_history("history.txt").unwrap();
}

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &LogMetadata) -> bool {
        metadata.level() <= LogLevel::Debug
    }

    fn log(&self, record: &LogRecord) {
        if self.enabled(record.metadata()) {
            eprintln!("{} - {}", record.level(), record.args());
        }
    }
}

fn init_logger() -> Result<(), SetLoggerError> {
    log::set_logger(|max_log_level| {
        max_log_level.set(LogLevelFilter::Info);
        Box::new(Logger)
    })
}
