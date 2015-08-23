extern crate nix;
extern crate rustyline;

use rustyline::error::ReadlineError;
use rustyline::history::History;

fn main() {
    let mut history = Some(History::new());
    history.as_mut().unwrap().load("history.txt");
    loop {
        let readline = rustyline::readline(">> ", &mut history);
        match readline {
            Ok(line) => {
                history.as_mut().unwrap().add(&line);
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
    history.as_mut().unwrap().save("history.txt");
}
