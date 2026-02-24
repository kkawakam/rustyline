## Related topics

- [Multiple commands for a keybinding](https://github.com/kkawakam/rustyline/issues/306) and
  [Conditional Bind Sequences](https://github.com/kkawakam/rustyline/issues/269) : original issues
- [Conditional Bind Sequences](https://github.com/kkawakam/rustyline/pull/293) : incomplete proposal
- [Add `Cmd::Yield` for complex custom bindings](https://github.com/kkawakam/rustyline/pull/342) : another proposal
- [Initial invoke trait and auto-indent example](https://github.com/kkawakam/rustyline/pull/466) : a validator is like a custom action triggered indirectly.

And other issues that should be solved if our design is good:

- [Extend Meta-F,Alt+Right feature for hint partial completion](https://github.com/kkawakam/rustyline/pull/430)
- [Use Arrow-Up to search history with prefix](https://github.com/kkawakam/rustyline/issues/423)
- [Execute Arbitrary Command Via Keybinding](https://github.com/kkawakam/rustyline/issues/418)
- [Use Ctrl-E for hint completion](https://github.com/kkawakam/rustyline/pull/407)
- [Add History Search Behaviour](https://github.com/kkawakam/rustyline/pull/424)
- ...

## Conditions / Filters

See https://python-prompt-toolkit.readthedocs.io/en/master/pages/advanced_topics/key_bindings.html?highlight=filter#attaching-a-filter-condition

Some keys/commands may behave differently depending on:

- edit mode (emacs vs vi)
- vi input mode (insert vs replace vs command modes)
- empty line
- cursor position
- repeat count
- original key pressed (when same command is bound to different key)
- hint
- ...

## More input

Some keys/commands may ask for more input.
I am not sure if this point should be tackled here.

## Multiple / complex actions

`EventHandler::Macro(Vec<Cmd>)` executes a sequence of commands for a single key binding.

- **Undo**: the sequence is wrapped in a single undo group (`Changeset::begin`/`end`), so one undo reverts all commands.
- **Kill ring**: the kill ring is not reset between commands, preserving consecutive-kill semantics within the macro.
- **Refresh**: a `refresh_line()` is called after the last command completes. Individual commands still refresh during execution; since they run back-to-back without I/O waits, there is no visible flicker.

## Misc

```rust
/// Command / action result
#[derive(Debug, Clone, PartialEq, Copy)]
#[non_exhaustive]
pub enum ActionResult {
    // Interrupt / reject user input
    // => Err should be fine
    //Bail,
    ///
    Continue,
    /// Accept user input (except if `Validator` disagrees)
    Return,
}
```

```rust
bitflags::bitflags! {
    #[doc = "Action invocation impacts"]
    pub struct Impacts: u8 {
        const PUSH_CHAR = 0b0000_0001;
        const BEEP = 0b0000_0010;
        const MOVE_CURSOR = 0b0000_0100; // State::move_cursor
        const REFRESH = 0b0000_1000; // State::refresh_line
        const CLEAR_SREEN = 0b0001_0000; // State::clear_screen
    }
}
```
