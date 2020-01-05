use rustyline::error::ReadlineError;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::Editor;
use rustyline_derive::{Completer, Helper, Highlighter, Hinter};

#[derive(Completer, Helper, Highlighter, Hinter)]
struct InputValidator {}

impl Validator for InputValidator {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<ValidationResult, ReadlineError> {
        let input = ctx.input();
        if !input.starts_with("SELECT") {
            return Ok(ValidationResult::Invalid(Some(
                " --< Expect: SELECT stmt".to_owned(),
            )));
        } else if !input.ends_with(';') {
            return Ok(ValidationResult::Incomplete);
        }
        Ok(ValidationResult::Valid(None))
    }
}

fn main() -> rustyline::Result<()> {
    let h = InputValidator {};
    let mut rl = Editor::new();
    rl.set_helper(Some(h));

    let input = rl.readline("> ")?;
    println!("Input: {}", input);
    Ok(())
}
