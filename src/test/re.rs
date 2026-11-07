use rustyline::{DefaultEditor, Result};

fn main() -> Result<()> {
    let args = std::env::args();
    let mut rl = DefaultEditor::default()?;
    if args.skip(1).find(|arg| arg.eq("-s")).is_some() {
        rl.readline(&("> ", "\x1b[1;32m> \x1b[0m"))
    } else {
        rl.readline("> ")
    }?;
    Ok(())
}
