use rustyline::validate::MatchingBracketValidator;
use rustyline::{Editor, Result};
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
    let mut rl = Editor::new();
    rl.set_helper(Some(h));

    let input = rl.readline("> ")?;
    println!("Input: {}", input);

    Ok(())
}
