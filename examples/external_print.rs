use env_logger;
use std::io::Write;
use std::thread;
use std::time::Duration;

use rand::{thread_rng, Rng};

use rustyline::error::ReadlineError;
use rustyline::Editor;

fn main() {
    env_logger::init();
    let mut rl = Editor::<()>::new();
    let mut printer = rl.create_external_printer().expect("No printer");
    thread::spawn(move || {
        let mut rng = thread_rng();
        let mut i = 0usize;
        loop {
            writeln!(printer, "External message #{}", i).expect("External print failure");
            let wait_ms = rng.gen_range(1, 500);
            thread::sleep(Duration::from_millis(wait_ms));
            i += 1;
        }
    });

    loop {
        let readline = rl.readline(">>");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());
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
}
