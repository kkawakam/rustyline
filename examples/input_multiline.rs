use rustyline::validate::MatchingBracketValidator;
use rustyline::{Cmd, Editor, EventHandler, KeyCode, KeyEvent, Modifiers, Result};
use rustyline_derive::{Completer, Helper, Highlighter, Hinter, Validator};

#[derive(Completer, Helper, Highlighter, Hinter, Validator)]
struct InputValidator {
    #[rustyline(Validator)]
    brackets: MatchingBracketValidator,
}

fn main() -> Result<()> {
    let h = InputValidator {
        brackets: MatchingBracketValidator::new(),
    };
    let mut rl = Editor::new()?;
    rl.set_helper(Some(h));
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('s'), Modifiers::CTRL),
        EventHandler::Simple(Cmd::Newline),
    );

    let input = rl.readline("> ")?;
    println!("Input: {input}");

    Ok(())
}
