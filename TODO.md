API
- [ ] expose an API callable from C

Async (#126)

Bell
- [ ] bell-style

Color
- [ ] ANSI Colors & Windows 10+
- [ ] ANSI Colors & Windows <10
- [ ] Syntax highlighting

Completion
- [ ] Quoted path
- [ ] Windows escape/unescape space in path
- [ ] file completion & escape/unescape (#106)
- [ ] file completion & tilde (#62)
- [ ] display versus replacement

Config
- [ ] Maximum buffer size for the line read

Cursor
- [ ] insert versus overwrite versus command mode
- [ ] In Vi command mode, prevent user from going to end of line. (#94)

Grapheme
- [ ] grapheme & input auto-wrap are buggy

Hints Callback
- [ ] Not implemented on windows

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

Repeat
- [ ] dynamic prompt (arg: ?)
- [ ] transpose chars

Undo
- [ ] Merge consecutive Replace
- [ ] Undo group
- [ ] Undo all changes made to this line.
- [ ] Kill+Insert (substitute/replace)
- [ ] Repeated undo `Undo(RepeatCount)`

Unix
- [ ] Terminfo (https://github.com/Stebalien/term)

Windows
- [ ] is_atty is not working with cygwin/msys
- [ ] UTF-16 surrogate pair
- [ ] handle ansi escape code
