use std::borrow::Cow;

use rustyline::highlight::Highlighter;
use rustyline::parse::{InputEdit, Parser};
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{Completer, Hinter};
use rustyline::{Editor, Helper, Result};

/// [Yellow, Cyan, BrightMagenta]
const BRACKET_COLORS: [u8; 3] = [33, 36, 95];
/// Red
const INVALID_COLOR: u8 = 31;
#[derive(Completer, Hinter)]
struct BarcketHelper {
    bracket_level: i32,
    color_indexs: Vec<(usize, u8)>,
    /// re-render only when input just changed
    /// not render after cursor moving
    need_render: bool,
}

impl Helper for BarcketHelper {}

impl Parser for BarcketHelper {
    /// Set `need_render = true`
    ///
    /// Parse input once, update the parsed bracker results into `BarcketHelper` itself.
    fn parse(&mut self, line: &str, _change: InputEdit) {
        self.need_render = true;
        let btyes = line.as_bytes();
        self.bracket_level = (0..line.len()).fold(0, |level, pos| {
            let c = btyes[pos];
            if c == b'(' {
                level + 1
            } else if c == b')' {
                level - 1
            } else {
                level
            }
        });
        let mut level = 0;
        self.color_indexs = (0..line.len())
            .filter_map(|pos| {
                let c = btyes[pos];
                if c == b'(' {
                    let color = if self.bracket_level <= level {
                        TryInto::<usize>::try_into(level).map_or(INVALID_COLOR, |level| {
                            BRACKET_COLORS[level % BRACKET_COLORS.len()]
                        })
                    } else {
                        INVALID_COLOR
                    };
                    level += 1;
                    Some((pos, color))
                } else if c == b')' {
                    level -= 1;
                    let color = TryInto::<usize>::try_into(level).map_or(INVALID_COLOR, |level| {
                        BRACKET_COLORS[level % BRACKET_COLORS.len()]
                    });
                    Some((pos, color))
                } else {
                    None
                }
            })
            .collect();
    }
}

impl Validator for BarcketHelper {
    /// Use the parsed bracker results to validate
    fn validate(&mut self, _ctx: &mut ValidationContext) -> Result<ValidationResult> {
        if self.bracket_level > 0 {
            Ok(ValidationResult::Incomplete)
        } else if self.bracket_level < 0 {
            Ok(ValidationResult::Invalid(Some(format!(
                " - excess {} close bracket",
                -self.bracket_level
            ))))
        } else {
            Ok(ValidationResult::Valid(None))
        }
    }
}

impl Highlighter for BarcketHelper {
    /// use `need_render` to decide whether to highlight again
    fn highlight_char(&mut self, _line: &str, _pos: usize, _forced: bool) -> bool {
        self.need_render
    }
    /// Set `need_render = false`
    ///
    /// Use the parsed bracker results to highlight
    fn highlight<'l>(&mut self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        self.need_render = false;
        if self.color_indexs.is_empty() {
            Cow::Borrowed(&line)
        } else {
            let mut out = String::new();
            let mut last_idx = 0;
            for (pos, color) in &self.color_indexs {
                out += &format!(
                    "{}\x1b[1;{}m{}\x1b[m",
                    &line[last_idx..(*pos)],
                    color,
                    &line[(*pos)..=(*pos)],
                );
                last_idx = *pos + 1;
            }
            out += &line[last_idx..];
            Cow::Owned(out)
        }
    }
}

fn main() -> Result<()> {
    let h = BarcketHelper {
        bracket_level: 0,
        color_indexs: Vec::new(),
        need_render: true,
    };
    let mut rl = Editor::new()?;
    rl.set_helper(Some(h));
    let input = rl.readline(">> ")?;
    println!("Input: {input}");

    Ok(())
}
