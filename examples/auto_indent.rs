use std::cmp::Ordering;
use std::io::Write;
use std::cell::RefCell;

use rustyline::error::ReadlineError;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::Editor;
use rustyline_derive::{Completer, Helper, Highlighter, Hinter};
use rustyline::{Cmd, Movement};

#[derive(Completer, Helper, Highlighter, Hinter)]
struct InputValidator {
    stack_protect: RefCell<()>,
}

const INDENT_SIZE: usize = 2;  // default

impl Validator for InputValidator {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<ValidationResult, ReadlineError> {
        let input = ctx.input();
        let _protect = if let Ok(cell) = self.stack_protect.try_borrow_mut() {
            cell
        } else {
            // This is reached when `NewLine` is called below
            return Ok(ValidationResult::Valid(None));
        };
        if let Some(line) = input.lines().last() {
            let indent_chars = line.len() - line.trim_start().len();
            writeln!(std::fs::File::create("./pipe").unwrap(),
                "Indent_chars {}", indent_chars).unwrap();
            let cur_indent = indent_chars / INDENT_SIZE;
            let open_braces = line.chars().filter(|c| *c == '{').count();
            let close_braces = line.chars().filter(|c| *c == '}').count();
            let indent = match open_braces.cmp(&close_braces) {
                Ordering::Greater => cur_indent + 1,
                Ordering::Equal => cur_indent,
                Ordering::Less => cur_indent.saturating_sub(1),
            };
            // dedent just edited line with single brace
            if line.trim() == "}" {
                ctx.invoke(Cmd::Dedent(Movement::WholeLine))?;
            }
            // indent new line as needed
            ctx.invoke(Cmd::Newline)?;
            writeln!(std::fs::File::create("./pipe").unwrap(),
                "INDENT {}", indent).unwrap();
            for _ in 0..indent {
                ctx.invoke(Cmd::Indent(Movement::WholeLine))?;
            }
            // Example always returns invalid, so you can type any text
            //
            // But Valid or Invalid result should contain empty string as
            // an error description to prevent newline being inserted, as we
            // already inserted newline above
            return Ok(ValidationResult::Invalid(Some(String::new())));
        }
        Ok(ValidationResult::Valid(None))
    }
}

fn main() -> rustyline::Result<()> {
    let h = InputValidator { stack_protect: RefCell::new(()) };
    let mut rl = Editor::new();
    rl.set_helper(Some(h));

    let input = rl.readline("> ")?;
    println!("Input: {}", input);
    Ok(())
}
