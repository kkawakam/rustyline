use std::collections::HashSet;

use rustyline::Editor;
use rustyline::{hint::Hinter, Context};
use rustyline_derive::{Completer, Helper, Highlighter, Validator};

#[derive(Completer, Helper, Validator, Highlighter)]
struct DIYHinter {
    // It's simple example of rustyline, for more effecient, please use ** radix trie **
    hints: HashSet<String>,
}

impl Hinter for DIYHinter {
    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<String> {
        if pos < line.len() {
            return None;
        }

        self.hints
            .iter()
            .filter_map(|hint| {
                // expect hint after word complete, like redis cli, add condition:
                // line.ends_with(" ")
                if pos > 0 && hint.starts_with(&line[..pos]) {
                    Some(hint[pos..].to_owned())
                } else {
                    None
                }
            })
            .next()
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
    let h = DIYHinter { hints: diy_hints() };

    let mut rl: Editor<DIYHinter> = Editor::new();
    rl.set_helper(Some(h));

    loop {
        let input = rl.readline("> ")?;
        println!("input: {}", input);
    }
}
