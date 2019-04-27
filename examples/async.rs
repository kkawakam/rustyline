use std::sync::mpsc as sync_mpsc;
use std::thread;
use std::time::Duration;

use rustyline::error::ReadlineError;
use rustyline::Editor;

use futures::future::Future;
use futures::stream::Stream;

use tokio::sync::mpsc as async_mpsc;

fn main() {
    let timeout = Duration::from_millis(10);

    let (mut stdin_tx, stdin_rx) = async_mpsc::unbounded_channel();
    let (stdout_tx, stdout_rx) = sync_mpsc::channel();

    let mut editor = Editor::<()>::new();

    let _thread = thread::spawn(move || loop {
        // Standard usage of the Rustyline editor
        let line = editor.readline("> ");

        // Decide whether we should quit, otherwise we loop forever.
        let quit = match &line {
            Err(ReadlineError::Interrupted) => true,
            Err(ReadlineError::Eof) => true,
            _ => false,
        };

        // Send read lines into the async system
        stdin_tx.try_send(line).expect("failed to send");

        // Block for a moment to give the async code a chance to run
        match stdout_rx.recv_timeout(timeout) {
            Ok(msg) => println!("{}", msg),
            Err(sync_mpsc::RecvTimeoutError::Timeout) => (),
            Err(sync_mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if quit {
            break;
        }
    });

    let read_all = stdin_rx.for_each(move |line| {
        // Handle the data sent by the Editor thread
        let message = match line {
            Ok(line) => format!("Line: {}", line),
            Err(ReadlineError::Interrupted) => format!("CTRL-C"),
            Err(ReadlineError::Eof) => format!("CTRL-D"),
            Err(err) => format!("Error: {:?}", err),
        };

        // Communicate back data to print to the user.
        stdout_tx.send(message).ok();

        Ok(())
    });

    tokio::run(read_all.map_err(|_| ()));
}
