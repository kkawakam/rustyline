use rustyline::completion::{extract_word, Completer, FilenameCompleter, Pair};
use rustyline::error::ReadlineError;
use rustyline::{CompletionType, Config, Context, EditMode, Editor};
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};
use std::collections::HashSet;

const DEFAULT_BREAK_CHARS: [u8; 3] = [b' ', b'\t', b'\n'];

#[derive(Hash, Debug, PartialEq, Eq)]
struct Command {
    cmd: String,
    pre_cmd: String,
}

impl Command {
    fn new(cmd: &str, pre_cmd: &str) -> Self {
        Self {
            cmd: cmd.into(),
            pre_cmd: pre_cmd.into(),
        }
    }
}
struct CommandCompleter {
    cmds: HashSet<Command>,
}

impl CommandCompleter {
    pub fn find_matches(&self, line: &str, pos: usize) -> rustyline::Result<(usize, Vec<Pair>)> {
        let (start, word) = extract_word(line, pos, None, &DEFAULT_BREAK_CHARS);
        let pre_cmd = line[..start].trim();

        let matches = self
            .cmds
            .iter()
            .filter_map(|hint| {
                if hint.cmd.starts_with(word) && pre_cmd == &hint.pre_cmd {
                    let mut replacement = hint.cmd.clone();
                    replacement += " ";
                    Some(Pair {
                        display: hint.cmd.to_string(),
                        replacement: replacement.to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();
        Ok((start, matches))
    }
}

impl Completer for CommandCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        self.find_matches(line, pos)
    }
}

#[derive(Helper, Hinter, Validator, Highlighter)]
struct MyHelper {
    file_completer: FilenameCompleter,
    cmd_completer: CommandCompleter,
}

impl Completer for MyHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        match self.cmd_completer.find_matches(line, pos) {
            Ok((start, matches)) => {
                if matches.is_empty() {
                    self.file_completer.complete(line, pos, ctx)
                } else {
                    Ok((start, matches))
                }
            }
            Err(e) => Err(e),
        }
    }
}

fn cmd_sets() -> HashSet<Command> {
    let mut set = HashSet::new();
    set.insert(Command::new("helper", "about"));
    set.insert(Command::new("hinter", "about"));
    set.insert(Command::new("highlighter", "about"));
    set.insert(Command::new("validator", "about"));
    set.insert(Command::new("completer", "about"));

    set.insert(Command::new("release", "dev"));
    set.insert(Command::new("deploy", "dev"));
    set.insert(Command::new("compile", "dev"));
    set.insert(Command::new("test", "dev"));

    set.insert(Command::new("history", ""));
    set.insert(Command::new("about", ""));
    set.insert(Command::new("help", ""));
    set.insert(Command::new("dev", ""));
    set
}

// To debug rustyline:
// RUST_LOG=rustyline=debug cargo run --example example 2> debug.log
fn main() -> rustyline::Result<()> {
    env_logger::init();
    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .edit_mode(EditMode::Emacs)
        .build();
    let h = MyHelper {
        file_completer: FilenameCompleter::new(),
        cmd_completer: CommandCompleter { cmds: cmd_sets() },
    };
    let mut rl: Editor<MyHelper> = Editor::with_config(config)?;
    rl.set_helper(Some(h));

    let mut count = 1;
    loop {
        let p = format!("{}> ", count);
        let readline = rl.readline(&p)?;
        println!("Line: {}", readline);
        count += 1;
    }
}
