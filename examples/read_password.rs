use std::borrow::Cow::{self, Owned};

use rustyline::config::Configurer;
use rustyline::highlight::Highlighter;
use rustyline::{ColorMode, Editor, Result};
use rustyline::{Completer, Helper, Hinter, Validator};

#[derive(Completer, Helper, Hinter, Validator)]
struct MaskingHighlighter {
    masking: bool,
}

impl Highlighter for MaskingHighlighter {
    #[cfg(any(not(feature = "split-highlight"), feature = "ansi-str"))]
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        use unicode_width::UnicodeWidthStr;
        if self.masking {
            Owned(" ".repeat(line.width()))
        } else {
            Cow::Borrowed(line)
        }
    }

    #[cfg(all(feature = "split-highlight", not(feature = "ansi-str")))]
    fn highlight_line<'l>(
        &self,
        line: &'l str,
        _pos: usize,
    ) -> impl ExactSizeIterator<Item = (rustyline::highlight::AnsiStyle, Cow<'l, str>)> {
        use unicode_width::UnicodeWidthStr;
        if self.masking {
            vec![(
                rustyline::highlight::AnsiStyle::default(),
                Owned(" ".repeat(line.width())),
            )]
            .into_iter()
        } else {
            vec![].into_iter()
        }
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _forced: bool) -> bool {
        self.masking
    }
}

fn main() -> Result<()> {
    println!("This is just a hack. Reading passwords securely requires more than that.");
    let h = MaskingHighlighter { masking: false };
    let mut rl = Editor::new()?;
    rl.set_helper(Some(h));

    let username = rl.readline("Username:")?;
    println!("Username: {username}");

    rl.helper_mut().expect("No helper").masking = true;
    rl.set_color_mode(ColorMode::Forced); // force masking
    rl.set_auto_add_history(false); // make sure password is not added to history
    let mut guard = rl.set_cursor_visibility(false)?;
    let passwd = rl.readline("Password:")?;
    guard.take();
    println!("Secret: {passwd}");
    Ok(())
}
