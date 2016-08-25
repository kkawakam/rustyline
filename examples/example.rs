extern crate rustyline;

use rustyline::completion::FilenameCompleter;
use rustyline::error::ReadlineError;
use rustyline::{Config, Editor};

// On unix platforms you can use ANSI escape sequences
#[cfg(unix)]
static PROMPT: &'static str = "\x1b[1;32m>>\x1b[0m ";

// Windows consoles typically don't support ANSI escape sequences out
// of the box
#[cfg(windows)]
static PROMPT: &'static str = ">> ";

fn main() {
    let config = Config::builder()
        .history_ignore_space(true)
        .build();
    let c = FilenameCompleter::new();
    let mut rl = Editor::new(config);
    rl.set_completer(Some(c));
    if let Err(_) = rl.load_history("history.txt") {
        println!("No previous history.");
    }
    loop {
        let readline = rl.readline(PROMPT);
        match readline {
            Ok(line) => {
                rl.add_history_entry(&line);
                println!("Line: {}", line);
            },
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break
            },
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break
            }
        }
    }
    rl.save_history("history.txt").unwrap();
}
