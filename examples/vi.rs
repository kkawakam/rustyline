//! This example shows how a highlighter can be used to show Vi mode indicators
//! while editing.

use std::borrow::Cow::{self, Borrowed, Owned};

use rustyline::highlight::{Highlighter, PromptState};
use rustyline::{Config, EditMode, Editor, InputMode};
use rustyline_derive::{Completer, Helper, Hinter, Validator};

#[derive(Completer, Helper, Hinter, Validator)]
struct ViHighlighter {
    command: &'static str,
    insert: &'static str,
    replace: &'static str,
}

impl Highlighter for ViHighlighter {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        prompt_state: PromptState,
    ) -> Cow<'b, str> {
        if prompt_state.default {
            let indicator = match prompt_state.mode {
                Some(InputMode::Command) => self.command,
                Some(InputMode::Insert) => self.insert,
                Some(InputMode::Replace) => self.replace,
                _ => " ",
            };
            Owned(format!("{}{}", indicator, &prompt[1..]))
        } else {
            Borrowed(prompt)
        }
    }
}

fn main() -> rustyline::Result<()> {
    let config = Config::builder().edit_mode(EditMode::Vi).build();
    let helper = ViHighlighter {
        command: ":",
        insert: "+",
        replace: "#",
    };
    let mut rl = Editor::with_config(config);
    rl.set_helper(Some(helper));

    let username = rl.readline(" > ")?;
    println!("Echo: {}", username);

    Ok(())
}
