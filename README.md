# RustyLine
[![Build Status](https://travis-ci.org/kkawakam/rustyline.svg?branch=master)](https://travis-ci.org/kkawakam/rustyline)

Readline implementation in Rust that is based on [Antirez' Linenoise](https://github.com/antirez/linenoise)

[Documentation](https://kkawakam.github.io/rustyline)

## Build
This project uses Cargo and Rust Nightly
```bash
cargo build --release
```

## Example
```rust
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

```toml
[dependencies]
rustyline = "0.2.3"
```

## Features

 - Unicode (UTF-8) (linenoise supports only ASCII)
 - Word completion (linenoise supports only line completion)
 - Filename completion
 - History search ([Searching for Commands in the History](http://cnswww.cns.cwru.edu/php/chet/readline/readline.html#SEC8))
 - Kill ring ([Killing Commands](http://cnswww.cns.cwru.edu/php/chet/readline/readline.html#IDX3))
 - Multi line mode
 - Word commands

## Actions

Keystroke    | Action
---------    | ------
Ctrl-A, Home | Move cursor to the beginning of line
Ctrl-B, Left | Move cursor one character left
Ctrl-C       | Interrupt/Cancel edition
Ctrl-D, Del  | (if line is *not* empty) Delete character under cursor
Ctrl-D       | (if line *is* empty) End of File
Ctrl-E, End  | Move cursor to end of line
Ctrl-F, Right| Move cursor one character right
Ctrl-H, BackSpace | Delete character before cursor
Ctrl-J, Return | Finish the line entry
Ctrl-K       | Delete from cursor to end of line
Ctrl-L       | Clear screen
Ctrl-N, Down | Next match from history
Ctrl-P, Up   | Previous match from history
Ctrl-R       | Reverse Search history (Ctrl-S forward, Ctrl-G cancel)
Ctrl-T       | Transpose previous character with current character
Ctrl-U       | Delete from start of line to cursor
Ctrl-V       | Insert any special character without perfoming its associated action
Ctrl-W       | Delete word leading up to cursor (using white space as a word boundary)
Ctrl-Y       | Paste from Yank buffer (Alt-Y to paste next yank instead)
Tab          | Next completion
Alt-B, Alt-Left | Move cursor to previous word
Alt-C        | Capitalize the current word
Alt-D        | Delete forwards one word
Alt-F, Alt-Right | Move cursor to next word
Alt-L        | Lower-case the next word
Alt-T        | Transpose words
Alt-U        | Upper-case the next word
Alt-Y        | See Ctrl-Y
Alt-BackSpace | Kill from the start of the current word, or, if between words, to the start of the previous word

## ToDo

 - Show completion list
 - expose an API callable from C
