//! Hints (suggestions at the right of the prompt as you type).

use crate::history::Direction;
use crate::Context;

/// Hints provider
pub trait Hinter {
    /// Takes the currently edited `line` with the cursor `pos`ition and
    /// returns the string that should be displayed or `None`
    /// if no hint is available for the text the user currently typed.
    // TODO Validate: called while editing line but not while moving cursor.
    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        let _ = (line, pos, ctx);
        None
    }
}

impl Hinter for () {}

impl<'r, H: ?Sized + Hinter> Hinter for &'r H {
    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        (**self).hint(line, pos, ctx)
    }
}

/// Add suggestion based on previous history entries matching current user
/// input.
pub struct HistoryHinter {}

impl Hinter for HistoryHinter {
    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        if pos < line.len() {
            return None;
        }
        let start = if ctx.history_index() == ctx.history().len() {
            ctx.history_index().saturating_sub(1)
        } else {
            ctx.history_index()
        };
        if let Some(history_index) =
            ctx.history
                .starts_with(&line[..pos], start, Direction::Reverse)
        {
            let entry = ctx.history.get(history_index);
            if let Some(entry) = entry {
                if entry == line || entry == &line[..pos] {
                    return None;
                }
            }
            return entry.map(|s| s[pos..].to_owned());
        }
        None
    }
}

#[cfg(test)]
mod test {
    use super::{Hinter, HistoryHinter};
    use crate::history::History;
    use crate::Context;

    #[test]
    pub fn empty_history() {
        let history = History::new();
        let ctx = Context::new(&history);
        let hinter = HistoryHinter {};
        let hint = hinter.hint("test", 4, &ctx);
        assert_eq!(None, hint);
    }
}
