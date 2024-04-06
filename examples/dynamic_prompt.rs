use std::thread;
use std::time::Duration;

use rustyline::{DefaultEditor, ExternalPrinter, Result};

fn main() -> Result<()> {
    let mut rl = DefaultEditor::new()?;
    let mut printer = rl.create_external_printer()?;
    thread::spawn(move || {
        let mut i = 0usize;
        loop {
            printer
                .set_prompt(format!("prompt {:02}>", i))
                .expect("set prompt successfully");
            thread::sleep(Duration::from_secs(1));
            i += 1;
        }
    });

    loop {
        let line = rl.readline("> ")?;
        rl.add_history_entry(line.as_str())?;
        println!("Line: {line}");
    }
}
