API
- [ ] expose an API callable from C

Async (#126)

Bell
- [X] bell-style

Color
- [X] ANSI Colors & Windows 10+
- [ ] ANSI Colors & Windows <10 (https://docs.rs/console/0.6.1/console/fn.strip_ansi_codes.html ?
  https://github.com/mattn/go-colorable/blob/master/colorable_windows.go,
  https://github.com/mattn/ansicolor-w32.c)
- [ ] Syntax highlighting (https://github.com/trishume/syntect/)
- [ ] clicolors spec (https://docs.rs/console/0.6.1/console/fn.colors_enabled.html)

Completion
- [X] Quoted path
- [X] Windows escape/unescape space in path
- [ ] file completion & escape/unescape (#106)
- [ ] file completion & tilde (#62)
- [X] display versus replacement
- [ ] composite/alternate completer (if the current completer returns nothing, try the next one)

Config
- [ ] Maximum buffer size for the line read

Cursor
- [ ] insert versus overwrite versus command mode
- [ ] In vi command mode, prevent user from going to end of line. (#94)

Grapheme
- [ ] grapheme & input auto-wrap are buggy

Hints Callback
- [X] Not implemented on windows
- [X] Do an implementation based on previous history

History
- [ ] Move to the history line n
- [ ] historyFile: Where to read/write the history at the start and end of
each line input session.
- [ ] append_history
- [ ] history_truncate_file

Input
- [ ] Password input (#58) (https://github.com/conradkdotcom/rpassword) (https://github.com/antirez/linenoise/issues/125)
- [X] quoted insert (#65)
- [ ] Overwrite mode (em-toggle-overwrite, vi-replace-mode, rl_insert_mode)
- [ ] Encoding
- [ ] [Ctrl-][Alt-][Shift-]<Key> (#121)

Layout
- [ ] Redraw perf (https://crates.io/crates/cassowary)

Misc
- [ ] fallible iterator (https://docs.rs/fallible-iterator/0.2.1/fallible_iterator/)

Mouse
- [ ] Mouse support

Movement
- [ ] Move to the corresponding opening/closing bracket

Redo
- [X] redo substitute

Repeat
- [X] dynamic prompt (arg: ?)
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
- [ ] [ncurses](https://crates.io/crates/ncurses) alternative backend ?
- [X] [bracketed paste mode](https://cirw.in/blog/bracketed-paste)
- [ ] async stdin (https://github.com/Rufflewind/tokio-file-unix)

Windows
- [ ] is_atty is not working with Cygwin/MSYS (https://github.com/softprops/atty works but then how to make `enable_raw_mode` works ?)
  (https://github.com/mitsuhiko/console/blob/master/src/windows_term.rs#L285)
  (https://github.com/mattn/go-isatty/blob/master/isatty_windows.go, https://github.com/mattn/go-tty/blob/master/tty_windows.go#L143)
- [X] UTF-16 surrogate pair
- [ ] handle ANSI escape code (#61) (https://github.com/DanielKeep/rust-ansi-interpreter)
