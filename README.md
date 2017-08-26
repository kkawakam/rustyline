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
 - Multi line mode (line wrapping)
 - Word commands

## Actions

For all modes:

Keystroke    | Action
---------    | ------
Home         | Move cursor to the beginning of line
End          | Move cursor to end of line
Left         | Move cursor one character left
Right        | Move cursor one character right
Ctrl-C       | Interrupt/Cancel edition
Ctrl-D, Del  | (if line is *not* empty) Delete character under cursor
Ctrl-D       | (if line *is* empty) End of File
Ctrl-J, Ctrl-M, Enter | Finish the line entry
Ctrl-R       | Reverse Search history (Ctrl-S forward, Ctrl-G cancel)
Ctrl-T       | Transpose previous character with current character
Ctrl-U       | Delete from start of line to cursor
Ctrl-V       | Insert any special character without perfoming its associated action (#65)
Ctrl-W       | Delete word leading up to cursor (using white space as a word boundary)
Ctrl-X Ctrl-U | Undo
Ctrl-Y       | Paste from Yank buffer
Ctrl-Z       | Suspend (unix only)
Ctrl-_       | Undo

### Emacs mode (default mode)

Keystroke    | Action
---------    | ------
Ctrl-A, Home | Move cursor to the beginning of line
Ctrl-B, Left | Move cursor one character left
Ctrl-E, End  | Move cursor to end of line
Ctrl-F, Right| Move cursor one character right
Ctrl-H, BackSpace | Delete character before cursor
Ctrl-I, Tab  | Next completion
Ctrl-K       | Delete from cursor to end of line
Ctrl-L       | Clear screen
Ctrl-N, Down | Next match from history
Ctrl-P, Up   | Previous match from history
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
Meta-0, 1, ..., - | Specify the digit to the argument. `–` starts a negative argument.

[Readline Emacs Editing Mode Cheat Sheet](http://www.catonmat.net/download/readline-emacs-editing-mode-cheat-sheet.pdf)

### Vi command mode

Keystroke    | Action
---------    | ------
$, End       | Move cursor to end of line
.            | Redo the last text modification
;            | Redo the last character finding command
,            | Redo the last character finding command in opposite direction
0, Home      | Move cursor to the beginning of line
^            | Move to the first non-blank character of line
a            | Insert after cursor
A            | Insert at the end of line
b            | Move one word or token left
B            | Move one non-blank word left
c<movement>  | Change text of a movement command
C            | Change text to the end of line (equivalent to c$)
d<movement>  | Delete text of a movement command
D, Ctrl-K    | Delete to the end of the line
e            | Move to the end of the current word
E            | Move to the end of the current non-blank word
f<char>      | Move right to the next occurance of `char`
F<char>      | Move left to the previous occurance of `char`
h, Ctrl-H, BackSpace | Move one character left
l, Space     | Move one character right
Ctrl-L       | Clear screen
i            | Insert before cursor
I            | Insert at the beginning of line
+, j, Ctrl-N | Move forward one command in history
-, k, Ctrl-P | Move backward one command in history
p            | Insert the yanked text at the cursor (paste)
P            | Insert the yanked text before the cursor
r            | Replaces a single character under the cursor (without leaving command mode)
s            | Delete a single character under the cursor and enter input mode
S            | Change current line (equivalent to 0c$)
t<char>      | Move right to the next occurance of `char`, then one char backward
T<char>      | Move left to the previous occurance of `char`, then one char forward
u            | Undo
w            | Move one word or token right
W            | Move one non-blank word right
x            | Delete a single character under the cursor
X            | Delete a character before the cursor
y<movement>  | Yank a movement into buffer (copy)

### Vi insert mode

Keystroke    | Action
---------    | ------
Ctrl-H, BackSpace | Delete character before cursor
Ctrl-I, Tab  | Next completion
Esc          | Switch to command mode

[Readline VI Editing Mode Cheat Sheet](http://www.catonmat.net/download/bash-vi-editing-mode-cheat-sheet.pdf)

[Terminal codes (ANSI/VT100)](http://wiki.bash-hackers.org/scripting/terminalcodes)

## ToDo

 - expose an API callable from C

## Wine

```sh
$ cargo run --example example --target 'x86_64-pc-windows-gnu'
...
Error: Io(Error { repr: Os { code: 6, message: "Invalid handle." } })
$ wineconsole --backend=curses target/x86_64-pc-windows-gnu/debug/examples/example.exe
...
```

## Terminal checks

```sh
$ # current settings of all terminal attributes:
$ stty -a
$ # key bindings:
$ bind -p
```

## Similar projects

 - [copperline](https://github.com/srijs/rust-copperline) (Rust)
 - [linefeed](https://github.com/murarth/linefeed) (Rust)
 - [liner](https://github.com/MovingtoMars/liner) (Rust)
 - [linenoise-ng](https://github.com/arangodb/linenoise-ng) (C++)
 - [liner](https://github.com/peterh/liner) (Go)
 - [readline](https://github.com/chzyer/readline) (Go)
 - [haskeline](https://github.com/judah/haskeline) (Haskell)
 - [rb-readline](https://github.com/ConnorAtherton/rb-readline) (Ruby)
 - [python-prompt-toolkit](https://github.com/jonathanslenders/python-prompt-toolkit) (Python)
