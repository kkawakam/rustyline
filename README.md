# RustyLine
[![Build Status](https://travis-ci.org/kkawakam/rustyline.svg?branch=master)](https://travis-ci.org/kkawakam/rustyline)
[![Build status](https://ci.appveyor.com/api/projects/status/ls7sty8nt25rdfkq/branch/master?svg=true)](https://ci.appveyor.com/project/kkawakam/rustyline/branch/master)
[![Clippy Linting Result](https://clippy.bashy.io/github/kkawakam/rustyline/master/badge.svg)](https://clippy.bashy.io/github/kkawakam/rustyline/master/log)
[![](http://meritbadge.herokuapp.com/rustyline)](https://crates.io/crates/rustyline)

Readline implementation in Rust that is based on [Antirez' Linenoise](https://github.com/antirez/linenoise)

[Documentation (Releases)](https://docs.rs/rustyline)

[Documentation (Master)](https://kkawakam.github.io/rustyline/rustyline/)

**Supported Platforms**
* Linux
* Windows
   * cmd.exe
   * Powershell

**Note**: Powershell ISE is not supported, check [issue #56](https://github.com/kkawakam/rustyline/issues/56)

## Build
This project uses Cargo and Rust stable
```bash
cargo build --release
```

## Example
```rust
extern crate rustyline;

use rustyline::error::ReadlineError;
use rustyline::Editor;

fn main() {
    // `()` can be used when no completer is required
    let mut rl = Editor::<()>::new();
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
rustyline = "1.0.0"
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
Ctrl-I, Tab  | Next completion
Ctrl-J, Ctrl-M, Enter | Finish the line entry
Ctrl-K       | Delete from cursor to end of line
Ctrl-L       | Clear screen
Ctrl-N, Down | Next match from history
Ctrl-P, Up   | Previous match from history
Ctrl-R       | Reverse Search history (Ctrl-S forward, Ctrl-G cancel)
Ctrl-T       | Transpose previous character with current character
Ctrl-U       | Delete from start of line to cursor
Ctrl-V       | Insert any special character without perfoming its associated action
Ctrl-W       | Delete word leading up to cursor (using white space as a word boundary)
Ctrl-Y       | Paste from Yank buffer (Meta-Y to paste next yank instead)
Meta-<       | Move to first entry in history
Meta->       | Move to last entry in history
Meta-B, Alt-Left | Move cursor to previous word
Meta-C       | Capitalize the current word
Meta-D       | Delete forwards one word
Meta-F, Alt-Right | Move cursor to next word
Meta-L       | Lower-case the next word
Meta-T       | Transpose words
Meta-U       | Upper-case the next word
Meta-Y       | See Ctrl-Y
Meta-BackSpace | Kill from the start of the current word, or, if between words, to the start of the previous word

## ToDo

 - Show completion list
 - Undos
 - Read input with timeout to properly handle single ESC key
 - expose an API callable from C

## Wine

```sh
$ cargo run --example example --target 'x86_64-pc-windows-gnu'
...
Error: Io(Error { repr: Os { code: 6, message: "Invalid handle." } })
$ wineconsole --backend=curses target/x86_64-pc-windows-gnu/debug/examples/example.exe
...
```

## Similar projects

 - [copperline](https://github.com/srijs/rust-copperline) (Rust)
 - [liner](https://github.com/MovingtoMars/liner) (Rust)
 - [linenoise-ng](https://github.com/arangodb/linenoise-ng) (C++)
 - [liner](https://github.com/peterh/liner) (Go)
 - [readline](https://github.com/chzyer/readline) (Go)
 - [haskeline](https://github.com/judah/haskeline) (Haskell)
