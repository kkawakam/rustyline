use std::collections::HashSet;

use rustyline::{highlight::Highlighter, hint::Hinter, Context};
use rustyline::Editor;
use rustyline_derive::{Completer, Helper, Validator};

#[derive(Completer, Helper, Validator)]
struct DIYHinter {
    hints: HashSet<String>,
}

impl Highlighter for DIYHinter {
    fn highlight_char(&self, _line: &str, _pos: usize) -> bool {
        false
    }
}

impl Hinter for DIYHinter {
    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<String> {
        if pos < line.len() {
            return None;
        }

        self.hints
            .iter()
            .filter_map(|hint| {
                // expect hint after word complete, like redis-cli, add this condition below: line.ends_with(" ") 
                if pos>0 && hint.starts_with(&line[..pos]) {
                    Some(hint[pos..].to_owned())
                } else {
                    None
                }
            })
            .nth(0)
    }
}

fn diy_hints() -> HashSet<String> {
    let mut set = HashSet::new();
    set.insert(String::from("help"));
    set.insert(String::from("get key"));
    set.insert(String::from("set key value"));
    set.insert(String::from("hget key field"));
    set.insert(String::from("hset key field value"));
    set
}

fn main() -> rustyline::Result<()> {
    println!("This is a DIY hint hack of rustyline");
    let h = DIYHinter {
        hints: HashSet::new(),
    };
    let mut rl = Editor::new();
    rl.set_helper(Some(h));
    rl.helper_mut().expect("No helper").hints = diy_hints();

    loop {
        let passwd = rl.readline("> ")?;
        println!("input: {}", passwd);
    }
}
