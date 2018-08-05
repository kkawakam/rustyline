API
- [ ] expose an API callable from C

Async (#126)

Bell
- [ ] bell-style

Color
- [x] ANSI Colors & Windows 10+
- [ ] ANSI Colors & Windows <10 (https://docs.rs/console/0.6.1/console/fn.strip_ansi_codes.html ? https://github.com/mattn/go-colorable/blob/master/colorable_windows.go)
- [ ] Syntax highlighting
- [ ] clicolors spec (https://docs.rs/console/0.6.1/console/fn.colors_enabled.html)

Completion
- [X] Quoted path
- [ ] Windows escape/unescape space in path
- [ ] file completion & escape/unescape (#106)
- [ ] file completion & tilde (#62)
- [X] display versus replacement
- [ ] composite/alternate completer (if the current completer returns nothing, try the next one)

Config
- [ ] Maximum buffer size for the line read

Cursor
- [ ] insert versus overwrite versus command mode
- [ ] In Vi command mode, prevent user from going to end of line. (#94)

Grapheme
- [ ] grapheme & input auto-wrap are buggy

Hints Callback
- [x] Not implemented on windows

History
- [ ] Move to the history line n
- [ ] historyFile: Where to read/write the history at the start and end of
each line input session.
- [ ] append_history
- [ ] history_truncate_file

Input
- [ ] Password input (#58)
- [ ] quoted insert (#65)
- [ ] Overwrite mode (em-toggle-overwrite, vi-replace-mode, rl_insert_mode)
- [ ] Encoding

Mouse
- [ ] Mouse support

Movement
- [ ] Move to the corresponding opening/closing bracket

Redo
- [X] redo substitue

Repeat
- [x] dynamic prompt (arg: ?)
- [ ] transpose chars

Syntax
- [ ] syntax specific tokenizer/parser
- [ ] highlighting

Undo
- [ ] Merge consecutive Replace
- [X] Undo group
- [ ] Undo all changes made to this line.
- [X] Kill+Insert (substitute/replace)
- [X] Repeated undo `Undo(RepeatCount)`

Unix
- [ ] Terminfo (https://github.com/Stebalien/term)

Windows
- [ ] is_atty is not working with cygwin/msys (https://github.com/softprops/atty works but then how to make `enable_raw_mode` works ?)
- [X] UTF-16 surrogate pair
- [ ] handle ansi escape code (https://docs.rs/console/0.6.1/console/fn.strip_ansi_codes.html ? https://github.com/mattn/go-colorable/blob/master/colorable_windows.go)
