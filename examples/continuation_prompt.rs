use rustyline::validate::MatchingBracketValidator;
use rustyline::{Cmd, Editor, EventHandler, KeyCode, KeyEvent, Modifiers, Result, highlight::Highlighter};
use rustyline::{Completer, Helper, Hinter, Validator};
use std::borrow::Cow::{self, Borrowed};
#[derive(Completer, Helper, Hinter, Validator)]
struct InputValidator {
    #[rustyline(Validator)]
    brackets: MatchingBracketValidator,
}

impl Highlighter for InputValidator {
    fn continuation_prompt<'p, 'b>(
        &self,
        prompt: &'p str,
        default: bool,
    ) -> Option<Cow<'b, str>> {
        Some(Borrowed(".... "))
    }
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
    let input = rl.readline(">>>> ")?;
    println!("Input:\n{input}");

    Ok(())
}
