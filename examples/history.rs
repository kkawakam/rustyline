extern crate nix;
extern crate rustyline;

use nix::Error;
use nix::errno::Errno;
use rustyline::error::ReadlineError;
use rustyline::history::History;

fn main() {
    let mut history = Some(History::new());
    loop {
        let readline = rustyline::readline(">> ", &mut history);
        match readline {
            Ok(line) => {
                history.as_mut().unwrap().add(&line);
                println!("Line: {}", line);
            },
            Err(ReadlineError::Errno(Error::Sys(Errno::EAGAIN))) => {
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
}
