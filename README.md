# RustyLine
[![Build Status](https://travis-ci.org/kkawakam/rustyline.svg?branch=master)](https://travis-ci.org/kkawakam/rustyline)

Readline implementation in Rust that is based on [Antirez' Linenoise](https://github.com/antirez/linenoise)

[Documentation](https://kkawakam.github.io/rustyline)

## Build
This project uses Cargo and Rust Stable
```bash
cargo build --release
```

## Example
``rust
extern crate rustyline;

use rustyline::error::ReadlineError;
use rustyline::Editor;

fn main() {
    let mut rl = Editor::new();
    if let Err(_) = rl.load_history("history.txt") {
        println!("No previous history.");
    }
    loop {
        let readline = rl.readline(">> ");
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
```
                          
## crates.io
You can use this package in your project by adding the following
to your `Cargo.toml`:

``toml
[dependencies]
rustyline = "0.1.0"
```
