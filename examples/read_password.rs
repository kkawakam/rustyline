use std::borrow::Cow::{self, Borrowed, Owned};

use rustyline::completion::Completer;
use rustyline::config::Configurer;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::{ColorMode, Editor, Helper};

struct MaskingHighlighter {
    masking: bool,
}

impl Highlighter for MaskingHighlighter {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        use unicode_width::UnicodeWidthStr;
        if self.masking {
            Owned("*".repeat(line.width()))
        } else {
            Borrowed(line)
        }
    }

    fn highlight_char(&self, _line: &str, _pos: usize) -> bool {
        self.masking
    }
}

impl Completer for MaskingHighlighter {
    type Candidate = String;
}
impl Hinter for MaskingHighlighter {}
impl Helper for MaskingHighlighter {}

fn main() -> rustyline::Result<()> {
    println!("This is just a hack. Reading passwords securely requires more than that.");
    let h = MaskingHighlighter { masking: false };
    let mut rl = Editor::new();
    rl.set_helper(Some(h));

    let username = rl.readline("Username:")?;
    println!("Username: {}", username);

    rl.helper_mut().expect("No helper").masking = true;
    rl.set_color_mode(ColorMode::Forced); // force masking
    rl.set_auto_add_history(false); // make sure password is not added to history
    let passwd = rl.readline("Password:")?;
    println!("Secret: {}", passwd);
    Ok(())
}
