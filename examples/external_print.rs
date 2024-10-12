extern crate env_logger;
extern crate log;

use std::io::Write;
use std::thread;
use std::time::Duration;
use env_logger::Target;
use log::LevelFilter;

use rand::{thread_rng, Rng};

use rustyline::{DefaultEditor, ExternalPrinter, Result};

fn main() -> Result<()> {
    let mut rl = DefaultEditor::new()?;
    let mut printer = rl.create_external_printer()?;

    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .target(Target::Pipe(rl.create_external_writer()?))
        .init();
    thread::spawn(move || {
        loop {
            log::info!("Log Message");
            thread::sleep(Duration::from_secs(3));
        }
    });

    let mut writer = rl.create_external_writer().unwrap();
    thread::spawn(move || {
        let mut rng = thread_rng();
        let mut i = 0usize;
        loop {
            writer.write("writing without newline".as_bytes());
            printer
                .print(format!("External message #{i}"))
                .expect("External print failure");
            let wait_ms = rng.gen_range(1000..10000);
            thread::sleep(Duration::from_millis(wait_ms));
            i += 1;
        }
    });

    loop {
        let line = rl.readline("> ")?;
        println!("Read Line: {line}");
    }
}