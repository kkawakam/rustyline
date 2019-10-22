fn main() {
    use std::io::{stdin, stdout, Write};

    use rustyline::error::ReadlineError;
    use rustyline::Editor;

    loop {
        let mut editor = Editor::<()>::new();
        if editor.load_history("history.txt").is_err() {
        }
        let result = editor.readline("Enter expression (ctrl-C to quit): ");
        match result {
            Ok(mut line) => {
                editor.add_history_entry(line.as_str());
                line = line.trim().to_string();
                if line.len() > 0 {
                    println!("You entered: {}", line);
                }
            },
            Err(ReadlineError::Interrupted) => {
                break
            },
            Err(ReadlineError::Eof) => {
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break
            }
        }
        editor.save_history("history.txt").unwrap();
    }
}
